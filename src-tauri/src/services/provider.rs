use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fs,
    io::{Read, Write},
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::PermissionsExt,
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::{
    adapters::provider::{adapters, ProviderAdapter},
    domain::{
        agent::{AgentLaunchSnapshot, AgentSessionId},
        provider::{
            PiProjectTrust, PiProjectTrustState, PiRpcProbeResult, ProviderAvailability,
            ProviderDiagnostic, ProviderId, ProviderInfo,
        },
        workspace::WorkspaceId,
    },
};

use super::{
    terminal::LaunchSpec,
    workspace::{WorkspaceError, WorkspaceService},
};

const ENV_MARKER: &[u8] = b"\0BAIBO_LOGIN_ENV_V1\0";
const ENV_COMMAND: &str = "printf '\\0BAIBO_LOGIN_ENV_V1\\0'; /usr/bin/env -0";
const MAX_ENV_BYTES: usize = 1024 * 1024;
const MAX_COMMAND_BYTES: usize = 1024 * 1024;
const MAX_RPC_LINE_BYTES: usize = 256 * 1024;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
struct LoginEnvironment {
    values: BTreeMap<OsString, OsString>,
}

impl LoginEnvironment {
    fn get(&self, name: &str) -> Option<&OsStr> {
        self.values.get(OsStr::new(name)).map(OsString::as_os_str)
    }
}

#[derive(Clone)]
struct DetectedProvider {
    info: ProviderInfo,
    executable: Option<PathBuf>,
}

#[derive(Clone)]
struct DetectionCache {
    environment: LoginEnvironment,
    providers: BTreeMap<ProviderId, DetectedProvider>,
}

type ProviderDetector = dyn Fn() -> Result<DetectionCache, ProviderError> + Send + Sync + 'static;

#[derive(Clone)]
pub struct ProviderService {
    workspace_service: WorkspaceService,
    cache: Arc<Mutex<Option<Arc<DetectionCache>>>>,
    detector: Arc<ProviderDetector>,
    #[cfg(test)]
    detection_count: Arc<AtomicUsize>,
}

pub(crate) struct PreparedAgentLaunch {
    pub launch: LaunchSpec,
    pub snapshot: AgentLaunchSnapshot,
}

impl ProviderService {
    pub fn new(workspace_service: WorkspaceService) -> Self {
        Self {
            workspace_service,
            cache: Arc::new(Mutex::new(None)),
            detector: Arc::new(detect_providers),
            #[cfg(test)]
            detection_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(
        workspace_service: WorkspaceService,
        provider_id: ProviderId,
        executable: PathBuf,
        environment: BTreeMap<OsString, OsString>,
    ) -> Self {
        let adapter = adapter(provider_id);
        let info = ProviderInfo {
            id: provider_id,
            display_name: adapter.display_name().into(),
            availability: ProviderAvailability::Available,
            executable_path: executable.to_str().map(str::to_owned),
            version: Some("test".into()),
            launch_modes: adapter.supported_launch_modes(),
            capabilities: adapter.capabilities(),
            diagnostic: None,
        };
        let detection = DetectionCache {
            environment: LoginEnvironment {
                values: environment,
            },
            providers: BTreeMap::from([(
                provider_id,
                DetectedProvider {
                    info,
                    executable: Some(executable),
                },
            )]),
        };
        let refreshed = detection.clone();
        let detection_count = Arc::new(AtomicUsize::new(0));
        let detector_count = detection_count.clone();
        Self {
            workspace_service,
            cache: Arc::new(Mutex::new(Some(Arc::new(detection)))),
            detector: Arc::new(move || {
                detector_count.fetch_add(1, Ordering::SeqCst);
                Ok(refreshed.clone())
            }),
            detection_count,
        }
    }

    #[cfg(test)]
    pub(crate) fn detection_count(&self) -> usize {
        self.detection_count.load(Ordering::SeqCst)
    }

    pub fn list(&self) -> Result<Vec<ProviderInfo>, ProviderError> {
        let cache = self.cache()?;
        Ok(cache
            .providers
            .values()
            .map(|provider| provider.info.clone())
            .collect())
    }

    pub fn refresh(&self) -> Result<Vec<ProviderInfo>, ProviderError> {
        let detected = Arc::new((self.detector)()?);
        let result = detected
            .providers
            .values()
            .map(|provider| provider.info.clone())
            .collect();
        *self.cache.lock().map_err(|_| ProviderError::Internal)? = Some(detected);
        Ok(result)
    }

    pub(crate) fn build_launch_spec(
        &self,
        provider_id: ProviderId,
        workspace_id: &WorkspaceId,
        agent_session_id: &AgentSessionId,
    ) -> Result<PreparedAgentLaunch, ProviderError> {
        let workspace = self
            .workspace_service
            .resolve_for_terminal(workspace_id)
            .map_err(ProviderError::from_workspace)?;
        let cache = self.cache()?;
        let detected = cache
            .providers
            .get(&provider_id)
            .ok_or(ProviderError::ProviderUnavailable(provider_id))?;
        if detected.info.availability != ProviderAvailability::Available {
            return Err(match detected.info.availability {
                ProviderAvailability::Unsupported => {
                    ProviderError::ProviderUnsupported(provider_id)
                }
                _ => ProviderError::ProviderUnavailable(provider_id),
            });
        }
        let executable = detected
            .executable
            .as_deref()
            .ok_or(ProviderError::ProviderUnavailable(provider_id))?;
        let adapter = adapter(provider_id);
        let launch = adapter.build_launch_spec(
            executable,
            &cache.environment.values,
            Path::new(&workspace.canonical_path),
            workspace_id,
            agent_session_id,
        );
        let executable_path = launch
            .executable
            .to_str()
            .map(str::to_owned)
            .ok_or(ProviderError::LaunchSnapshotInvalid)?;
        let argv = launch
            .argv
            .iter()
            .map(|argument| {
                argument
                    .to_str()
                    .map(str::to_owned)
                    .ok_or(ProviderError::LaunchSnapshotInvalid)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PreparedAgentLaunch {
            launch,
            snapshot: AgentLaunchSnapshot {
                executable_path: Some(executable_path),
                argv,
                provider_version: detected.info.version.clone(),
            },
        })
    }

    pub fn pi_project_trust(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<PiProjectTrust, ProviderError> {
        let workspace = self
            .workspace_service
            .resolve_for_terminal(workspace_id)
            .map_err(ProviderError::from_workspace)?;
        let cache = self.cache()?;
        match resolve_pi_trust(
            workspace_id.clone(),
            Path::new(&workspace.canonical_path),
            &cache.environment,
        ) {
            Err(ProviderError::PiTrustUnknown) => Ok(PiProjectTrust {
                workspace_id: workspace_id.clone(),
                state: PiProjectTrustState::Unknown,
                message: "无法可靠读取 Pi trust/settings；请在 Pi TUI 中确认。".into(),
            }),
            result => result,
        }
    }

    pub fn run_pi_rpc_probe(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<PiRpcProbeResult, ProviderError> {
        let workspace = self
            .workspace_service
            .resolve_for_terminal(workspace_id)
            .map_err(ProviderError::from_workspace)?;
        let cache = self.cache()?;
        let provider = cache
            .providers
            .get(&ProviderId::Pi)
            .ok_or(ProviderError::ProviderUnavailable(ProviderId::Pi))?;
        if provider.info.availability != ProviderAvailability::Available {
            return Err(ProviderError::ProviderUnavailable(ProviderId::Pi));
        }
        let executable = provider
            .executable
            .as_deref()
            .ok_or(ProviderError::ProviderUnavailable(ProviderId::Pi))?;
        rpc_probe(
            executable,
            &cache.environment,
            Path::new(&workspace.canonical_path),
        )
    }

    fn cache(&self) -> Result<Arc<DetectionCache>, ProviderError> {
        if let Some(cache) = self
            .cache
            .lock()
            .map_err(|_| ProviderError::Internal)?
            .clone()
        {
            return Ok(cache);
        }
        let detected = Arc::new((self.detector)()?);
        let mut guard = self.cache.lock().map_err(|_| ProviderError::Internal)?;
        Ok(guard.get_or_insert_with(|| detected.clone()).clone())
    }
}

fn adapter(provider_id: ProviderId) -> Box<dyn ProviderAdapter> {
    adapters()
        .into_iter()
        .find(|adapter| adapter.id() == provider_id)
        .expect("built-in provider adapter")
}

fn detect_providers() -> Result<DetectionCache, ProviderError> {
    let environment = resolve_login_environment()?;
    let mut providers = BTreeMap::new();
    for adapter in adapters() {
        let detected = detect_provider(adapter.as_ref(), &environment);
        providers.insert(adapter.id(), detected);
    }
    Ok(DetectionCache {
        environment,
        providers,
    })
}

fn detect_provider(
    adapter: &dyn ProviderAdapter,
    environment: &LoginEnvironment,
) -> DetectedProvider {
    let executable = resolve_path_executable(environment, adapter.executable_name());
    let Some(executable) = executable else {
        let recovery = find_inactive_nvm_install(environment, adapter.executable_name()).map(
            |_| {
                format!(
                    "该命令安装在非活动 Node 环境中。请让登录 Shell 的 PATH 包含 {}，再刷新 provider。",
                    adapter.executable_name()
                )
            },
        );
        return DetectedProvider {
            info: ProviderInfo {
                id: adapter.id(),
                display_name: adapter.display_name().into(),
                availability: ProviderAvailability::Unavailable,
                executable_path: None,
                version: None,
                launch_modes: adapter.supported_launch_modes(),
                capabilities: adapter.capabilities(),
                diagnostic: Some(ProviderDiagnostic {
                    code: "provider_executable_not_found".into(),
                    message: format!("登录环境的 PATH 中找不到 {}。", adapter.executable_name()),
                    recovery: recovery.or_else(|| {
                        Some(format!(
                            "请在登录 Shell 中安装 {}，或修正 PATH 后刷新。",
                            adapter.display_name()
                        ))
                    }),
                }),
            },
            executable: None,
        };
    };

    let version = run_command(
        &executable,
        adapter.version_args(),
        environment,
        None,
        None,
        COMMAND_TIMEOUT,
    );
    let version = match version {
        Ok(output) if output.success => first_nonempty_line(&output.stdout, &output.stderr),
        Ok(_) | Err(_) => {
            return detection_failure(
                adapter,
                executable,
                ProviderAvailability::Error,
                "provider_version_failed",
                "可执行文件存在，但版本检测失败。",
            );
        }
    };
    let Some(version) = version else {
        return detection_failure(
            adapter,
            executable,
            ProviderAvailability::Error,
            "provider_version_invalid",
            "版本命令没有返回可识别的版本。",
        );
    };

    if let Some(args) = adapter.help_args() {
        match run_command(&executable, args, environment, None, None, COMMAND_TIMEOUT) {
            Ok(output)
                if output.success
                    && adapter.validate_help(&String::from_utf8_lossy(&output.stdout)) => {}
            _ => {
                return detection_failure(
                    adapter,
                    executable,
                    ProviderAvailability::Unsupported,
                    "provider_version_unsupported",
                    "当前版本缺少 CP3 所需的 RPC 或 trust 参数。",
                );
            }
        }
    }

    DetectedProvider {
        info: ProviderInfo {
            id: adapter.id(),
            display_name: adapter.display_name().into(),
            availability: ProviderAvailability::Available,
            executable_path: executable.to_str().map(str::to_owned),
            version: Some(version),
            launch_modes: adapter.supported_launch_modes(),
            capabilities: adapter.capabilities(),
            diagnostic: None,
        },
        executable: Some(executable),
    }
}

fn detection_failure(
    adapter: &dyn ProviderAdapter,
    executable: PathBuf,
    availability: ProviderAvailability,
    code: &str,
    message: &str,
) -> DetectedProvider {
    DetectedProvider {
        info: ProviderInfo {
            id: adapter.id(),
            display_name: adapter.display_name().into(),
            availability,
            executable_path: executable.to_str().map(str::to_owned),
            version: None,
            launch_modes: adapter.supported_launch_modes(),
            capabilities: adapter.capabilities(),
            diagnostic: Some(ProviderDiagnostic {
                code: code.into(),
                message: message.into(),
                recovery: Some("请更新该 CLI，并在刷新 provider 后重试。".into()),
            }),
        },
        executable: Some(executable),
    }
}

fn resolve_login_environment() -> Result<LoginEnvironment, ProviderError> {
    let shell = login_shell().ok_or(ProviderError::LoginShellUnavailable)?;
    let mut command = Command::new(&shell);
    command
        .args(["-lic", ENV_COMMAND])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_child(command, None, COMMAND_TIMEOUT, MAX_ENV_BYTES)?;
    if !output.success {
        return Err(ProviderError::LoginEnvironmentFailed);
    }
    parse_environment(&output.stdout)
}

fn parse_environment(output: &[u8]) -> Result<LoginEnvironment, ProviderError> {
    let marker = output
        .windows(ENV_MARKER.len())
        .rposition(|window| window == ENV_MARKER)
        .ok_or(ProviderError::LoginEnvironmentInvalid)?;
    let bytes = &output[marker + ENV_MARKER.len()..];
    let mut values = BTreeMap::new();
    for entry in bytes
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let Some(separator) = entry.iter().position(|byte| *byte == b'=') else {
            continue;
        };
        if separator == 0 {
            continue;
        }
        values.insert(
            OsString::from_vec(entry[..separator].to_vec()),
            OsString::from_vec(entry[separator + 1..].to_vec()),
        );
    }
    if !values.contains_key(OsStr::new("PATH")) {
        return Err(ProviderError::LoginEnvironmentInvalid);
    }
    Ok(LoginEnvironment { values })
}

fn resolve_path_executable(environment: &LoginEnvironment, name: &str) -> Option<PathBuf> {
    let path = environment.get("PATH")?;
    for directory in path.as_bytes().split(|byte| *byte == b':') {
        if directory.is_empty() {
            continue;
        }
        let candidate = PathBuf::from(OsString::from_vec(directory.to_vec())).join(name);
        let Ok(canonical) = fs::canonicalize(candidate) else {
            continue;
        };
        if is_executable(&canonical) {
            return Some(canonical);
        }
    }
    None
}

fn find_inactive_nvm_install(environment: &LoginEnvironment, name: &str) -> Option<PathBuf> {
    let home = PathBuf::from(environment.get("HOME")?);
    let versions = fs::read_dir(home.join(".nvm/versions/node")).ok()?;
    for version in versions.flatten() {
        let candidate = version.path().join("bin").join(name);
        if is_executable(&candidate) {
            return fs::canonicalize(candidate).ok();
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    path.is_absolute()
        && fs::metadata(path)
            .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

fn first_nonempty_line(stdout: &[u8], stderr: &[u8]) -> Option<String> {
    stdout
        .split(|byte| *byte == b'\n')
        .chain(stderr.split(|byte| *byte == b'\n'))
        .map(|line| String::from_utf8_lossy(line).trim().to_owned())
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(256).collect())
}

fn run_command(
    executable: &Path,
    args: &[&str],
    environment: &LoginEnvironment,
    cwd: Option<&Path>,
    input: Option<&[u8]>,
    timeout: Duration,
) -> Result<CommandOutput, ProviderError> {
    let mut command = Command::new(executable);
    command
        .args(args)
        .env_clear()
        .envs(&environment.values)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    run_child(command, input, timeout, MAX_COMMAND_BYTES)
}

struct CommandOutput {
    success: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run_child(
    mut command: Command,
    input: Option<&[u8]>,
    timeout: Duration,
    limit: usize,
) -> Result<CommandOutput, ProviderError> {
    let mut child = command
        .spawn()
        .map_err(|_| ProviderError::CommandSpawnFailed)?;
    let stdout = child
        .stdout
        .take()
        .ok_or(ProviderError::CommandSpawnFailed)?;
    let stderr = child
        .stderr
        .take()
        .ok_or(ProviderError::CommandSpawnFailed)?;
    let stdout_worker = thread::spawn(move || read_limited(stdout, limit));
    let stderr_worker = thread::spawn(move || read_limited(stderr, limit));
    if let Some(input) = input {
        child
            .stdin
            .take()
            .ok_or(ProviderError::CommandSpawnFailed)?
            .write_all(input)
            .map_err(|_| ProviderError::CommandIo)?;
    }
    drop(child.stdin.take());
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|_| ProviderError::CommandIo)? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ProviderError::CommandTimeout);
        }
        thread::sleep(Duration::from_millis(10));
    };
    let stdout = stdout_worker
        .join()
        .map_err(|_| ProviderError::CommandIo)??;
    let stderr = stderr_worker
        .join()
        .map_err(|_| ProviderError::CommandIo)??;
    Ok(CommandOutput {
        success: status.success(),
        stdout,
        stderr,
    })
}

fn read_limited(mut reader: impl Read, limit: usize) -> Result<Vec<u8>, ProviderError> {
    let mut result = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let length = reader
            .read(&mut buffer)
            .map_err(|_| ProviderError::CommandIo)?;
        if length == 0 {
            return Ok(result);
        }
        let remaining = limit.saturating_sub(result.len());
        result.extend_from_slice(&buffer[..length.min(remaining)]);
        if length > remaining {
            return Err(ProviderError::CommandOutputTooLarge);
        }
    }
}

fn login_shell() -> Option<PathBuf> {
    unsafe {
        let buffer_size = libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX);
        let buffer_size = if buffer_size > 0 {
            usize::try_from(buffer_size).ok()?
        } else {
            16 * 1024
        };
        let mut entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut result = std::ptr::null_mut();
        let mut buffer = vec![0_u8; buffer_size];
        if libc::getpwuid_r(
            libc::geteuid(),
            entry.as_mut_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        ) != 0
            || result.is_null()
        {
            return None;
        }
        let entry = entry.assume_init();
        if entry.pw_shell.is_null() {
            return None;
        }
        let shell = std::ffi::CStr::from_ptr(entry.pw_shell);
        let path = PathBuf::from(OsStr::from_bytes(shell.to_bytes()));
        is_executable(&path).then_some(path)
    }
}

fn resolve_pi_trust(
    workspace_id: WorkspaceId,
    cwd: &Path,
    environment: &LoginEnvironment,
) -> Result<PiProjectTrust, ProviderError> {
    let home = environment
        .get("HOME")
        .map(PathBuf::from)
        .ok_or(ProviderError::PiTrustUnknown)?;
    if !has_pi_project_resources(cwd, &home) {
        return Ok(PiProjectTrust {
            workspace_id,
            state: PiProjectTrustState::NotRequired,
            message: "未发现需要 Pi 项目信任的本地资源。".into(),
        });
    }
    let agent_dir = environment
        .get("PI_CODING_AGENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".pi/agent"));
    let trust = read_trust_decisions(&agent_dir.join("trust.json"))?;
    if let Some(decision) = nearest_trust_decision(&trust, cwd) {
        return Ok(PiProjectTrust {
            workspace_id,
            state: if decision {
                PiProjectTrustState::Trusted
            } else {
                PiProjectTrustState::Denied
            },
            message: if decision {
                "Pi 已保存对此目录或其父目录的信任决定。".into()
            } else {
                "Pi 已保存对此目录或其父目录的不信任决定。".into()
            },
        });
    }
    let settings = read_json_object(&agent_dir.join("settings.json"))?;
    let policy = match settings.get("defaultProjectTrust") {
        None => "ask",
        Some(Value::String(policy)) => policy.as_str(),
        Some(_) => return Err(ProviderError::PiTrustUnknown),
    };
    let (state, message) = match policy {
        "always" => (
            PiProjectTrustState::Trusted,
            "Pi 的默认项目策略为始终信任。".into(),
        ),
        "never" => (
            PiProjectTrustState::Denied,
            "Pi 的默认项目策略为不信任。".into(),
        ),
        "ask" => (
            PiProjectTrustState::PromptRequired,
            "Pi 将在原生 TUI 中请求项目信任决定；Baibo 不会代为修改配置。".into(),
        ),
        _ => return Err(ProviderError::PiTrustUnknown),
    };
    Ok(PiProjectTrust {
        workspace_id,
        state,
        message,
    })
}

fn has_pi_project_resources(cwd: &Path, home: &Path) -> bool {
    const PI_RESOURCES: &[&str] = &[
        "settings.json",
        "extensions",
        "skills",
        "prompts",
        "themes",
        "SYSTEM.md",
        "APPEND_SYSTEM.md",
    ];
    if PI_RESOURCES
        .iter()
        .any(|name| cwd.join(".pi").join(name).exists())
    {
        return true;
    }
    let user_skills = home.join(".agents/skills");
    cwd.ancestors()
        .map(|ancestor| ancestor.join(".agents/skills"))
        .any(|skills| skills != user_skills && skills.exists())
}

fn read_json_object(path: &Path) -> Result<serde_json::Map<String, Value>, ProviderError> {
    if !path.exists() {
        return Ok(serde_json::Map::new());
    }
    let metadata = fs::metadata(path).map_err(|_| ProviderError::PiTrustUnknown)?;
    if metadata.len() > MAX_COMMAND_BYTES as u64 {
        return Err(ProviderError::PiTrustUnknown);
    }
    let bytes = fs::read(path).map_err(|_| ProviderError::PiTrustUnknown)?;
    serde_json::from_slice::<Value>(&bytes)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .ok_or(ProviderError::PiTrustUnknown)
}

fn read_trust_decisions(path: &Path) -> Result<serde_json::Map<String, Value>, ProviderError> {
    let trust = read_json_object(path)?;
    if trust
        .values()
        .all(|decision| decision.is_boolean() || decision.is_null())
    {
        Ok(trust)
    } else {
        Err(ProviderError::PiTrustUnknown)
    }
}

fn nearest_trust_decision(trust: &serde_json::Map<String, Value>, cwd: &Path) -> Option<bool> {
    cwd.ancestors().find_map(|path| {
        let key = path.to_str()?;
        trust.get(key).and_then(Value::as_bool)
    })
}

fn rpc_probe(
    executable: &Path,
    environment: &LoginEnvironment,
    cwd: &Path,
) -> Result<PiRpcProbeResult, ProviderError> {
    let started = Instant::now();
    let temp_dir = std::env::temp_dir().join(format!("baibo-pi-rpc-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&temp_dir).map_err(|_| ProviderError::RpcProbeFailed)?;
    let mut isolated = environment.clone();
    isolated.values.insert(
        OsString::from("PI_CODING_AGENT_DIR"),
        temp_dir.as_os_str().to_owned(),
    );
    let id = uuid::Uuid::new_v4().to_string();
    let request = format!("{{\"type\":\"get_state\",\"id\":\"{id}\"}}\n");
    let args = [
        "--mode",
        "rpc",
        "--no-session",
        "--no-approve",
        "--offline",
        "--no-extensions",
        "--no-skills",
        "--no-prompt-templates",
        "--no-themes",
        "--no-context-files",
    ];
    let result = run_rpc_command(executable, &args, &isolated, cwd, request.as_bytes(), &id);
    let _ = fs::remove_dir_all(&temp_dir);
    result?;
    Ok(PiRpcProbeResult {
        provider_id: ProviderId::Pi,
        ok: true,
        message: "Pi RPC get_state 响应有效，JSONL 未连接到终端视图。".into(),
        elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

fn run_rpc_command(
    executable: &Path,
    args: &[&str],
    environment: &LoginEnvironment,
    cwd: &Path,
    request: &[u8],
    id: &str,
) -> Result<(), ProviderError> {
    let mut command = Command::new(executable);
    command
        .args(args)
        .env_clear()
        .envs(&environment.values)
        .current_dir(cwd)
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|_| ProviderError::CommandSpawnFailed)?;
    let process_group = i32::try_from(child.id()).ok();
    let Some(mut stdin) = child.stdin.take() else {
        terminate_process_group(&mut child, process_group);
        return Err(ProviderError::CommandSpawnFailed);
    };
    if stdin.write_all(request).is_err() {
        terminate_process_group(&mut child, process_group);
        return Err(ProviderError::CommandIo);
    }
    drop(stdin);
    let Some(stdout) = child.stdout.take() else {
        terminate_process_group(&mut child, process_group);
        return Err(ProviderError::CommandSpawnFailed);
    };
    let Some(stderr) = child.stderr.take() else {
        terminate_process_group(&mut child, process_group);
        return Err(ProviderError::CommandSpawnFailed);
    };
    let (output_tx, output_rx) = mpsc::sync_channel::<Result<Vec<u8>, ProviderError>>(16);
    let stdout_worker = thread::spawn(move || {
        let mut reader = stdout;
        let mut buffer = [0_u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(length) => {
                    if output_tx.send(Ok(buffer[..length].to_vec())).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    let _ = output_tx.send(Err(ProviderError::CommandIo));
                    break;
                }
            }
        }
    });
    let stderr_worker = thread::spawn(move || read_limited(stderr, MAX_COMMAND_BYTES));
    let deadline = Instant::now() + COMMAND_TIMEOUT;
    let mut output = Vec::new();
    let mut parsed = 0;
    let result = loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break Err(ProviderError::CommandTimeout);
        }
        match output_rx.recv_timeout(remaining.min(Duration::from_millis(50))) {
            Ok(Ok(chunk)) => {
                if output.len().saturating_add(chunk.len()) > MAX_COMMAND_BYTES {
                    break Err(ProviderError::RpcResponseTooLarge);
                }
                output.extend_from_slice(&chunk);
                let mut protocol_error = None;
                while let Some(relative) = output[parsed..].iter().position(|byte| *byte == b'\n') {
                    let end = parsed + relative;
                    let line = &output[parsed..end];
                    parsed = end + 1;
                    if line.len() > MAX_RPC_LINE_BYTES {
                        protocol_error = Some(ProviderError::RpcResponseTooLarge);
                        break;
                    }
                    match rpc_response_matches(line, id) {
                        Ok(true) => break,
                        Ok(false) => {}
                        Err(error) => {
                            protocol_error = Some(error);
                            break;
                        }
                    }
                }
                if let Some(error) = protocol_error {
                    break Err(error);
                }
                if validate_rpc_response(&output[..parsed], id).is_ok() {
                    break Ok(());
                }
            }
            Ok(Err(error)) => break Err(error),
            Err(mpsc::RecvTimeoutError::Timeout) => match child.try_wait() {
                Ok(Some(_)) => break validate_rpc_response(&output, id),
                Ok(None) => {}
                Err(_) => break Err(ProviderError::CommandIo),
            },
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break validate_rpc_response(&output, id);
            }
        }
    };
    terminate_process_group(&mut child, process_group);
    let _ = stdout_worker.join();
    let _ = stderr_worker.join();
    result
}

fn terminate_process_group(child: &mut std::process::Child, process_group: Option<i32>) {
    let killed = process_group
        .map(|process_group| unsafe { libc::kill(-process_group, libc::SIGKILL) == 0 })
        .unwrap_or(false);
    if !killed {
        let _ = child.kill();
    }
    let _ = child.wait();
}

fn validate_rpc_response(bytes: &[u8], id: &str) -> Result<(), ProviderError> {
    if bytes.len() > MAX_COMMAND_BYTES {
        return Err(ProviderError::RpcResponseTooLarge);
    }
    for line in bytes.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        if line.len() > MAX_RPC_LINE_BYTES {
            return Err(ProviderError::RpcResponseTooLarge);
        }
        if rpc_response_matches(line, id)? {
            return Ok(());
        }
    }
    Err(ProviderError::RpcProtocolInvalid)
}

fn rpc_response_matches(line: &[u8], id: &str) -> Result<bool, ProviderError> {
    let value: Value =
        serde_json::from_slice(line).map_err(|_| ProviderError::RpcProtocolInvalid)?;
    Ok(value.get("id").and_then(Value::as_str) == Some(id)
        && value.get("type").and_then(Value::as_str) == Some("response")
        && value.get("command").and_then(Value::as_str) == Some("get_state")
        && value.get("success").and_then(Value::as_bool) == Some(true))
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider 命令输出异常")]
    CommandIo,
    #[error("provider 命令输出超过安全上限")]
    CommandOutputTooLarge,
    #[error("无法启动 provider 命令")]
    CommandSpawnFailed,
    #[error("provider 命令执行超时")]
    CommandTimeout,
    #[error("provider 内部状态不可用")]
    Internal,
    #[error("无法解析登录 Shell 环境")]
    LoginEnvironmentFailed,
    #[error("登录 Shell 环境格式无效")]
    LoginEnvironmentInvalid,
    #[error("provider 启动快照包含无法持久化的路径或参数")]
    LaunchSnapshotInvalid,
    #[error("无法找到可用的登录 Shell")]
    LoginShellUnavailable,
    #[error("无法读取 Pi 项目信任状态；请直接在 Pi TUI 中确认")]
    PiTrustUnknown,
    #[error("provider {0} 在当前登录环境中不可用")]
    ProviderUnavailable(ProviderId),
    #[error("provider {0} 的当前版本不受支持")]
    ProviderUnsupported(ProviderId),
    #[error("Pi RPC 响应不符合协议")]
    RpcProtocolInvalid,
    #[error("Pi RPC 诊断失败")]
    RpcProbeFailed,
    #[error("Pi RPC 响应超过安全上限")]
    RpcResponseTooLarge,
    #[error("{0}")]
    Workspace(WorkspaceError),
}

impl ProviderError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::CommandIo => "provider_command_io",
            Self::CommandOutputTooLarge => "provider_command_output_too_large",
            Self::CommandSpawnFailed => "provider_command_spawn_failed",
            Self::CommandTimeout => "provider_command_timeout",
            Self::Internal => "provider_internal",
            Self::LoginEnvironmentFailed => "login_environment_failed",
            Self::LoginEnvironmentInvalid => "login_environment_invalid",
            Self::LaunchSnapshotInvalid => "provider_launch_snapshot_invalid",
            Self::LoginShellUnavailable => "login_shell_unavailable",
            Self::PiTrustUnknown => "pi_trust_unknown",
            Self::ProviderUnavailable(_) => "provider_unavailable",
            Self::ProviderUnsupported(_) => "provider_unsupported",
            Self::RpcProtocolInvalid => "pi_rpc_protocol_invalid",
            Self::RpcProbeFailed => "pi_rpc_probe_failed",
            Self::RpcResponseTooLarge => "pi_rpc_response_too_large",
            Self::Workspace(error) => error.code(),
        }
    }

    fn from_workspace(error: WorkspaceError) -> Self {
        Self::Workspace(error)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCommandError {
    pub code: &'static str,
    pub message: String,
}

impl From<ProviderError> for ProviderCommandError {
    fn from(error: ProviderError) -> Self {
        log::error!(
            target: "baibo::provider",
            "provider operation failed: {}",
            error.code()
        );
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        ffi::{OsStr, OsString},
        fs,
        os::unix::{ffi::OsStrExt, fs::PermissionsExt},
        path::PathBuf,
    };

    use serde_json::json;
    use tempfile::TempDir;

    use crate::domain::{provider::PiProjectTrustState, workspace::WorkspaceId};

    use super::{
        detect_providers, nearest_trust_decision, parse_environment, resolve_pi_trust,
        run_rpc_command, validate_rpc_response, LoginEnvironment, ProviderError, ENV_MARKER,
        MAX_COMMAND_BYTES,
    };

    #[test]
    fn parses_marker_and_preserves_non_utf8_environment_values() {
        let mut output = b"shell startup noise\n".to_vec();
        output.extend_from_slice(ENV_MARKER);
        output.extend_from_slice(b"PATH=/usr/bin\0TOKEN=\xffsecret\0");
        let environment = parse_environment(&output).expect("environment");

        assert_eq!(environment.get("PATH"), Some(OsStr::new("/usr/bin")));
        assert_eq!(
            environment.get("TOKEN").expect("token").as_bytes(),
            b"\xffsecret"
        );
    }

    #[test]
    fn rejects_environment_without_marker_or_path() {
        assert!(parse_environment(b"PATH=/usr/bin\0").is_err());
        let mut output = ENV_MARKER.to_vec();
        output.extend_from_slice(b"HOME=/tmp\0");
        assert!(parse_environment(&output).is_err());
    }

    #[test]
    fn nearest_parent_trust_decision_wins() {
        let trust = json!({
            "/workspace": true,
            "/workspace/project": false,
            "/workspace/project/child": null
        })
        .as_object()
        .expect("object")
        .clone();

        assert_eq!(
            nearest_trust_decision(&trust, std::path::Path::new("/workspace/project/child")),
            Some(false)
        );
    }

    #[test]
    fn validates_fragment_joined_jsonl_without_splitting_unicode_separators() {
        let line = b"{\"id\":\"probe\",\"type\":\"response\",\"command\":\"get_state\",\"success\":true,\"data\":{\"note\":\"a\\u2028b\\u2029c\"}}\n";
        assert!(validate_rpc_response(line, "probe").is_ok());
        let crlf = b"{\"id\":\"probe\",\"type\":\"response\",\"command\":\"get_state\",\"success\":true}\r\n";
        assert!(validate_rpc_response(crlf, "probe").is_ok());
        assert!(validate_rpc_response(b"{\"id\":\"wrong\"}\n", "probe").is_err());
    }

    #[test]
    fn pi_trust_is_read_only_and_honors_parent_and_default_decisions() {
        let temp = TempDir::new().expect("temp");
        let home = temp.path().join("home");
        let workspace = temp.path().join("projects/workspace");
        let agent_dir = home.join(".pi/agent");
        fs::create_dir_all(workspace.join(".pi")).expect("project config");
        fs::create_dir_all(&agent_dir).expect("agent config");
        fs::write(workspace.join(".pi/settings.json"), "{}").expect("project setting");
        let trust_path = agent_dir.join("trust.json");
        let parent = workspace.parent().expect("parent").to_string_lossy();
        fs::write(&trust_path, format!("{{\"{parent}\":false}}\n")).expect("trust");
        fs::write(
            agent_dir.join("settings.json"),
            "{\"defaultProjectTrust\":\"always\"}\n",
        )
        .expect("settings");
        let before = fs::read(&trust_path).expect("before");
        let environment = LoginEnvironment {
            values: BTreeMap::from([
                (OsString::from("HOME"), home.as_os_str().to_owned()),
                (
                    OsString::from("PI_CODING_AGENT_DIR"),
                    agent_dir.as_os_str().to_owned(),
                ),
            ]),
        };

        let trust = resolve_pi_trust(
            WorkspaceId::from("workspace-a".to_owned()),
            &workspace,
            &environment,
        )
        .expect("trust");

        assert_eq!(trust.state, PiProjectTrustState::Denied);
        assert_eq!(fs::read(&trust_path).expect("after"), before);
    }

    #[test]
    fn pi_trust_reports_not_required_without_project_resources() {
        let temp = TempDir::new().expect("temp");
        let home = temp.path().join("home");
        let workspace = temp.path().join("workspace");
        fs::create_dir_all(&workspace).expect("workspace");
        let environment = LoginEnvironment {
            values: BTreeMap::from([(OsString::from("HOME"), home.into_os_string())]),
        };

        let trust = resolve_pi_trust(
            WorkspaceId::from("workspace-a".to_owned()),
            &workspace,
            &environment,
        )
        .expect("trust");

        assert_eq!(trust.state, PiProjectTrustState::NotRequired);
    }

    #[test]
    fn pi_trust_rejects_invalid_trust_and_settings_schemas() {
        let temp = TempDir::new().expect("temp");
        let home = temp.path().join("home");
        let workspace = temp.path().join("workspace");
        let agent_dir = home.join(".pi/agent");
        fs::create_dir_all(workspace.join(".pi")).expect("project config");
        fs::create_dir_all(&agent_dir).expect("agent config");
        fs::write(workspace.join(".pi/settings.json"), "{}").expect("project setting");
        let environment = LoginEnvironment {
            values: BTreeMap::from([
                (OsString::from("HOME"), home.as_os_str().to_owned()),
                (
                    OsString::from("PI_CODING_AGENT_DIR"),
                    agent_dir.as_os_str().to_owned(),
                ),
            ]),
        };
        let valid_parent = workspace.parent().expect("parent").to_string_lossy();
        fs::write(
            agent_dir.join("trust.json"),
            serde_json::to_vec(&json!({
                valid_parent.as_ref(): true,
                "/invalid": "yes"
            }))
            .expect("trust json"),
        )
        .expect("trust");

        assert!(matches!(
            resolve_pi_trust(
                WorkspaceId::from("workspace-a".to_owned()),
                &workspace,
                &environment,
            ),
            Err(ProviderError::PiTrustUnknown)
        ));

        fs::write(agent_dir.join("trust.json"), "{}").expect("empty trust");
        fs::write(
            agent_dir.join("settings.json"),
            "{\"defaultProjectTrust\":42}",
        )
        .expect("settings");
        assert!(matches!(
            resolve_pi_trust(
                WorkspaceId::from("workspace-a".to_owned()),
                &workspace,
                &environment,
            ),
            Err(ProviderError::PiTrustUnknown)
        ));
    }

    #[test]
    fn rpc_stdin_failure_terminates_the_spawned_process_group() {
        let temp = TempDir::new().expect("temp");
        let script = temp.path().join("close-stdin");
        let pid_file = temp.path().join("pid");
        fs::write(
            &script,
            "#!/bin/sh\nprintf '%s' \"$$\" > \"$PID_FILE\"\nexec 0<&-\n/bin/sleep 30\n",
        )
        .expect("script");
        let mut permissions = fs::metadata(&script).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&script, permissions).expect("permissions");
        let environment = LoginEnvironment {
            values: BTreeMap::from([
                (OsString::from("PATH"), OsString::from("/usr/bin:/bin")),
                (OsString::from("PID_FILE"), pid_file.as_os_str().to_owned()),
            ]),
        };
        let request = vec![b'x'; MAX_COMMAND_BYTES];

        let error = run_rpc_command(&script, &[], &environment, temp.path(), &request, "probe")
            .expect_err("closed stdin must fail");

        assert!(matches!(error, ProviderError::CommandIo));
        let pid: i32 = fs::read_to_string(pid_file)
            .expect("pid file")
            .parse()
            .expect("pid");
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn current_login_environment_returns_both_provider_diagnostics() {
        let cache = detect_providers().expect("login environment detection");

        assert_eq!(cache.providers.len(), 2);
        assert!(cache
            .providers
            .contains_key(&crate::domain::provider::ProviderId::Codex));
        assert!(cache
            .providers
            .contains_key(&crate::domain::provider::ProviderId::Pi));
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires Pi in the current login environment"]
    fn real_pi_rpc_get_state_smoke_test() {
        let cache = detect_providers().expect("login environment detection");
        let mut environment = cache.environment.clone();
        let pi = cache
            .providers
            .get(&crate::domain::provider::ProviderId::Pi)
            .and_then(|provider| provider.executable.as_deref())
            .map(PathBuf::from)
            .or_else(|| super::find_inactive_nvm_install(&environment, "pi"))
            .expect("Pi executable");
        if let Some(bin) = pi.parent() {
            let mut path = bin.as_os_str().to_owned();
            path.push(":");
            path.push(environment.get("PATH").expect("PATH"));
            environment.values.insert(OsString::from("PATH"), path);
        }
        let result = super::rpc_probe(&pi, &environment, std::path::Path::new("/tmp"))
            .expect("Pi RPC probe");

        assert!(result.ok);
    }
}
