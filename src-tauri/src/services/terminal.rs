use std::{
    collections::{BTreeMap, HashMap},
    ffi::OsString,
    fs,
    io::{Read, Write},
    os::unix::ffi::OsStrExt,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, TrySendError},
        Arc, Mutex,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use thiserror::Error;

use crate::{
    domain::{
        agent::NewAgentSession,
        terminal::{
            NewTerminalSession, SessionKind, TerminalAttachment, TerminalEvent, TerminalId,
            TerminalSession, TerminalStatus, TerminalSubscriptionId,
        },
        workspace::{WorkspaceId, WorkspaceRegistrySnapshot},
    },
    persistence::TerminalRepository,
};

use super::workspace::{WorkspaceError, WorkspaceService};

const MAX_INPUT_BYTES: usize = 64 * 1024;
const OUTPUT_CHUNK_BYTES: usize = 16 * 1024;
const PERSISTENCE_QUEUE_CHUNKS: usize = 128;
const SUBSCRIBER_QUEUE_CHUNKS: usize = 64;
const OUTPUT_TRUNCATED_MARKER: &[u8] = b"\r\n[baibo: terminal output truncated]\r\n";

enum PersistMessage {
    Data(Vec<u8>),
    Barrier(mpsc::Sender<()>),
}

pub trait TerminalSubscriber: Send + Sync {
    fn send_output(&self, data: Vec<u8>) -> Result<(), ()>;
    fn send_event(&self, event: TerminalEvent) -> Result<(), ()>;
}

struct SpawnedPty {
    master: Box<dyn MasterPty + Send>,
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn portable_pty::ChildKiller + Send + Sync>,
    child: Box<dyn Child + Send + Sync>,
    process_group: Option<libc::pid_t>,
}

trait PtyBackend: Send + Sync {
    fn spawn(
        &self,
        session: &TerminalSession,
        launch: &LaunchSpec,
    ) -> Result<SpawnedPty, TerminalError>;
}

struct PortablePtyBackend;

impl PtyBackend for PortablePtyBackend {
    fn spawn(
        &self,
        session: &TerminalSession,
        launch: &LaunchSpec,
    ) -> Result<SpawnedPty, TerminalError> {
        let pair = native_pty_system()
            .openpty(pty_size(session.cols, session.rows))
            .map_err(|_| TerminalError::SpawnFailed)?;
        let mut command = CommandBuilder::new(&launch.executable);
        command.args(launch.argv.iter());
        command.cwd(&launch.cwd);
        if let Some(environment) = &launch.environment {
            command.env_clear();
            for (key, value) in environment {
                command.env(key, value);
            }
        }
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|_| TerminalError::SpawnFailed)?;
        drop(pair.slave);

        let process_group = pair.master.process_group_leader();
        let reader = match pair.master.try_clone_reader() {
            Ok(reader) => reader,
            Err(_) => {
                terminate_spawned_child(&mut *child, process_group);
                return Err(TerminalError::SpawnFailed);
            }
        };
        let writer = match pair.master.take_writer() {
            Ok(writer) => writer,
            Err(_) => {
                terminate_spawned_child(&mut *child, process_group);
                return Err(TerminalError::SpawnFailed);
            }
        };
        let killer = child.clone_killer();
        Ok(SpawnedPty {
            master: pair.master,
            reader,
            writer,
            killer,
            child,
            process_group,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LaunchSpec {
    pub executable: PathBuf,
    pub argv: Vec<OsString>,
    pub cwd: PathBuf,
    pub environment: Option<BTreeMap<OsString, OsString>>,
}

struct SubscriberHandle {
    output: mpsc::SyncSender<Vec<u8>>,
    subscriber: Arc<dyn TerminalSubscriber>,
    lagged: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
}

struct LiveTerminal {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn portable_pty::ChildKiller + Send + Sync>>,
    process_group: Option<libc::pid_t>,
    stop_requested: AtomicBool,
    finished: AtomicBool,
    stream_gate: Mutex<()>,
    persistence: mpsc::SyncSender<PersistMessage>,
    subscribers: Mutex<HashMap<String, SubscriberHandle>>,
}

#[derive(Clone)]
pub struct TerminalManager {
    repository: TerminalRepository,
    workspace_service: WorkspaceService,
    backend: Arc<dyn PtyBackend>,
    live: Arc<Mutex<HashMap<TerminalId, Arc<LiveTerminal>>>>,
    operation_lock: Arc<Mutex<()>>,
}

impl TerminalManager {
    pub fn new(repository: TerminalRepository, workspace_service: WorkspaceService) -> Self {
        Self::new_with_backend(repository, workspace_service, Arc::new(PortablePtyBackend))
    }

    fn new_with_backend(
        repository: TerminalRepository,
        workspace_service: WorkspaceService,
        backend: Arc<dyn PtyBackend>,
    ) -> Self {
        Self {
            repository,
            workspace_service,
            backend,
            live: Arc::new(Mutex::new(HashMap::new())),
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn recover(&self) -> Result<usize, TerminalError> {
        self.repository.recover_interrupted(now_millis()?)
    }

    pub fn list(&self, workspace_id: &WorkspaceId) -> Result<Vec<TerminalSession>, TerminalError> {
        self.workspace_service
            .get_registered(workspace_id)
            .map_err(TerminalError::from_workspace)?;
        self.repository.list(workspace_id)
    }

    pub(crate) fn get(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
    ) -> Result<TerminalSession, TerminalError> {
        self.repository.get_scoped(workspace_id, terminal_id)
    }

    pub fn create(
        &self,
        workspace_id: WorkspaceId,
        cols: u16,
        rows: u16,
    ) -> Result<TerminalSession, TerminalError> {
        validate_size(cols, rows)?;
        let _operation = self
            .operation_lock
            .lock()
            .map_err(|_| TerminalError::Internal)?;
        let workspace = self
            .workspace_service
            .resolve_for_terminal(&workspace_id)
            .map_err(TerminalError::from_workspace)?;
        let shell = login_shell()?;
        let launch = LaunchSpec {
            executable: shell.clone(),
            argv: vec![OsString::from("-l")],
            cwd: PathBuf::from(&workspace.canonical_path),
            environment: None,
        };
        let terminal_id = TerminalId::new();
        let starting = self.repository.create(NewTerminalSession {
            id: terminal_id.clone(),
            workspace_id: workspace_id.clone(),
            title: String::new(),
            auto_title: true,
            shell: path_to_string(&shell)?,
            cwd: workspace.canonical_path.clone(),
            cols,
            rows,
            now: now_millis()?,
            session_kind: SessionKind::Shell,
        })?;

        match self.spawn(starting.clone(), launch) {
            Ok(session) => Ok(session),
            Err(error) => {
                let _ = self.repository.finish(
                    &workspace_id,
                    &terminal_id,
                    TerminalStatus::Failed,
                    None,
                    "spawn_failed",
                    now_millis().unwrap_or(starting.created_at),
                );
                Err(error)
            }
        }
    }

    fn spawn(
        &self,
        session: TerminalSession,
        launch: LaunchSpec,
    ) -> Result<TerminalSession, TerminalError> {
        let SpawnedPty {
            master,
            mut reader,
            writer,
            killer,
            mut child,
            process_group,
        } = self.backend.spawn(&session, &launch)?;
        let (persist_tx, persist_rx) =
            mpsc::sync_channel::<PersistMessage>(PERSISTENCE_QUEUE_CHUNKS);
        let live = Arc::new(LiveTerminal {
            master: Mutex::new(master),
            writer: Mutex::new(writer),
            killer: Mutex::new(killer),
            process_group,
            stop_requested: AtomicBool::new(false),
            finished: AtomicBool::new(false),
            stream_gate: Mutex::new(()),
            persistence: persist_tx.clone(),
            subscribers: Mutex::new(HashMap::new()),
        });
        self.live
            .lock()
            .map_err(|_| TerminalError::Internal)?
            .insert(session.id.clone(), live.clone());

        let running =
            match self
                .repository
                .mark_running(&session.workspace_id, &session.id, now_millis()?)
            {
                Ok(running) => running,
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    if let Ok(mut live_map) = self.live.lock() {
                        live_map.remove(&session.id);
                    }
                    return Err(error);
                }
            };

        let persist_repository = self.repository.clone();
        let persist_id = session.id.clone();
        let lag_marker_pending = Arc::new(AtomicBool::new(false));
        let persist_lag_marker = lag_marker_pending.clone();
        thread::spawn(move || {
            while let Ok(message) = persist_rx.recv() {
                match message {
                    PersistMessage::Data(data) => {
                        persist_chunk(&persist_repository, &persist_id, &data);
                        if persist_lag_marker.swap(false, Ordering::AcqRel) {
                            persist_truncation_marker(
                                &persist_repository,
                                &persist_id,
                                OUTPUT_TRUNCATED_MARKER,
                            );
                        }
                    }
                    PersistMessage::Barrier(acknowledge) => {
                        let _ = acknowledge.send(());
                    }
                }
            }
        });

        let reader_live = live.clone();
        let reader_id = session.id.clone();
        thread::spawn(move || {
            let mut buffer = [0u8; OUTPUT_CHUNK_BYTES];
            let mut lagged = false;
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(length) => {
                        let Ok(_stream) = reader_live.stream_gate.lock() else {
                            break;
                        };
                        let data = buffer[..length].to_vec();
                        broadcast_output(&reader_live, data.clone());
                        match persist_tx.try_send(PersistMessage::Data(data)) {
                            Ok(()) => lagged = false,
                            Err(TrySendError::Full(_)) if !lagged => {
                                lagged = true;
                                lag_marker_pending.store(true, Ordering::Release);
                                broadcast_output(&reader_live, OUTPUT_TRUNCATED_MARKER.to_vec());
                                broadcast_event(
                                    &reader_live,
                                    TerminalEvent::OutputLagged {
                                        terminal_id: reader_id.clone(),
                                    },
                                );
                            }
                            Err(TrySendError::Full(_)) => {}
                            Err(TrySendError::Disconnected(_)) => break,
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let waiter_manager = self.clone();
        let waiter_live = live;
        let waiter_workspace_id = session.workspace_id.clone();
        let waiter_terminal_id = session.id.clone();
        thread::spawn(move || {
            let waited = child.wait();
            waiter_live.finished.store(true, Ordering::Release);
            let stopped = waiter_live.stop_requested.load(Ordering::Acquire);
            let (status, exit_code, reason) = match waited {
                Ok(exit) if stopped => (
                    TerminalStatus::Stopped,
                    Some(exit.exit_code() as i32),
                    "user_stop",
                ),
                Ok(exit) if exit.signal().is_some() => (
                    TerminalStatus::Failed,
                    Some(exit.exit_code() as i32),
                    "signal_exit",
                ),
                Ok(exit) => (
                    TerminalStatus::Exited,
                    Some(exit.exit_code() as i32),
                    "process_exit",
                ),
                Err(_) => (TerminalStatus::Failed, None, "wait_failed"),
            };
            if let Ok(updated) = waiter_manager.repository.finish(
                &waiter_workspace_id,
                &waiter_terminal_id,
                status,
                exit_code,
                reason,
                now_millis().unwrap_or(0),
            ) {
                broadcast_event(
                    &waiter_live,
                    TerminalEvent::SessionUpdated { session: updated },
                );
            }
            if let Ok(mut live_map) = waiter_manager.live.lock() {
                live_map.remove(&waiter_terminal_id);
            }
        });

        Ok(running)
    }

    pub(crate) fn create_agent_with_launch_spec(
        &self,
        workspace_id: WorkspaceId,
        cols: u16,
        rows: u16,
        mut agent: NewAgentSession,
        launch: LaunchSpec,
    ) -> Result<TerminalSession, TerminalError> {
        validate_size(cols, rows)?;
        let _operation = self
            .operation_lock
            .lock()
            .map_err(|_| TerminalError::Internal)?;
        let workspace = self
            .workspace_service
            .resolve_for_terminal(&workspace_id)
            .map_err(TerminalError::from_workspace)?;
        if launch.cwd != Path::new(&workspace.canonical_path) {
            return Err(TerminalError::InvalidLaunchSpec);
        }
        let terminal_id = TerminalId::new();
        let now = now_millis()?;
        agent.created_at = now;
        let starting = self.repository.create_agent(
            NewTerminalSession {
                id: terminal_id.clone(),
                workspace_id: workspace_id.clone(),
                title: String::new(),
                auto_title: false,
                shell: path_to_string(&launch.executable)?,
                cwd: workspace.canonical_path,
                cols,
                rows,
                now,
                session_kind: SessionKind::Agent,
            },
            &agent,
        )?;
        if !is_executable(&launch.executable) {
            self.repository.finish(
                &workspace_id,
                &terminal_id,
                TerminalStatus::Failed,
                None,
                "executable_unavailable",
                now_millis().unwrap_or(starting.created_at),
            )?;
            return Err(TerminalError::SpawnFailed);
        }
        match self.spawn(starting.clone(), launch) {
            Ok(session) => Ok(session),
            Err(error) => {
                self.repository.finish(
                    &workspace_id,
                    &terminal_id,
                    TerminalStatus::Failed,
                    None,
                    "spawn_failed",
                    now_millis().unwrap_or(starting.created_at),
                )?;
                Err(error)
            }
        }
    }

    pub fn attach(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
        subscriber: Arc<dyn TerminalSubscriber>,
    ) -> Result<TerminalAttachment, TerminalError> {
        let mut session = self.repository.get_scoped(workspace_id, terminal_id)?;
        let subscription_id = TerminalSubscriptionId::new();
        let live = self
            .live
            .lock()
            .map_err(|_| TerminalError::Internal)?
            .get(terminal_id)
            .cloned();
        if let Some(live) = live {
            let _stream = live
                .stream_gate
                .lock()
                .map_err(|_| TerminalError::Internal)?;
            let (acknowledge, persisted) = mpsc::channel();
            live.persistence
                .send(PersistMessage::Barrier(acknowledge))
                .map_err(|_| TerminalError::Internal)?;
            persisted.recv().map_err(|_| TerminalError::Internal)?;
            let mut subscribers = live
                .subscribers
                .lock()
                .map_err(|_| TerminalError::Internal)?;
            let (output, output_rx) = mpsc::sync_channel::<Vec<u8>>(SUBSCRIBER_QUEUE_CHUNKS);
            let lagged = Arc::new(AtomicBool::new(false));
            let cancelled = Arc::new(AtomicBool::new(false));
            let worker_lagged = lagged.clone();
            let worker_cancelled = cancelled.clone();
            let worker_subscriber = subscriber.clone();
            let worker_terminal_id = terminal_id.clone();
            thread::spawn(move || {
                while let Ok(data) = output_rx.recv() {
                    if worker_cancelled.load(Ordering::Acquire) {
                        break;
                    }
                    if worker_subscriber.send_output(data).is_err() {
                        break;
                    }
                    if worker_lagged.swap(false, Ordering::AcqRel)
                        && worker_subscriber
                            .send_event(TerminalEvent::OutputLagged {
                                terminal_id: worker_terminal_id.clone(),
                            })
                            .is_err()
                    {
                        break;
                    }
                }
            });
            for chunk in self.repository.read_log(workspace_id, terminal_id)? {
                output
                    .send(chunk)
                    .map_err(|_| TerminalError::ChannelClosed)?;
            }
            subscribers.insert(
                subscription_id.as_str().to_owned(),
                SubscriberHandle {
                    output,
                    subscriber,
                    lagged,
                    cancelled,
                },
            );
            session = self.repository.get_scoped(workspace_id, terminal_id)?;
        } else {
            for chunk in self.repository.read_log(workspace_id, terminal_id)? {
                subscriber
                    .send_output(chunk)
                    .map_err(|_| TerminalError::ChannelClosed)?;
            }
        }
        Ok(TerminalAttachment {
            subscription_id,
            session,
        })
    }

    pub fn detach(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
        subscription_id: &TerminalSubscriptionId,
    ) -> Result<(), TerminalError> {
        self.repository.get_scoped(workspace_id, terminal_id)?;
        if let Some(live) = self
            .live
            .lock()
            .map_err(|_| TerminalError::Internal)?
            .get(terminal_id)
            .cloned()
        {
            if let Some(handle) = live
                .subscribers
                .lock()
                .map_err(|_| TerminalError::Internal)?
                .remove(subscription_id.as_str())
            {
                handle.cancelled.store(true, Ordering::Release);
            }
        }
        Ok(())
    }

    pub fn input(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
        data: &[u8],
    ) -> Result<(), TerminalError> {
        if data.is_empty() || data.len() > MAX_INPUT_BYTES {
            return Err(TerminalError::InvalidInput);
        }
        self.repository.get_scoped(workspace_id, terminal_id)?;
        let live = self.live_terminal(terminal_id)?;
        let mut writer = live.writer.lock().map_err(|_| TerminalError::Internal)?;
        writer
            .write_all(data)
            .and_then(|_| writer.flush())
            .map_err(|_| TerminalError::WriteFailed)
    }

    pub fn resize(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
        cols: u16,
        rows: u16,
    ) -> Result<TerminalSession, TerminalError> {
        validate_size(cols, rows)?;
        self.repository.get_scoped(workspace_id, terminal_id)?;
        let live = self.live_terminal(terminal_id)?;
        let master = live.master.lock().map_err(|_| TerminalError::Internal)?;
        master
            .resize(pty_size(cols, rows))
            .map_err(|_| TerminalError::ResizeFailed)?;
        let session = self
            .repository
            .resize(workspace_id, terminal_id, cols, rows)?;
        drop(master);
        Ok(session)
    }

    pub fn stop(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
    ) -> Result<TerminalSession, TerminalError> {
        let session = self.repository.get_scoped(workspace_id, terminal_id)?;
        let live = self.live_terminal(terminal_id)?;
        if live.stop_requested.swap(true, Ordering::AcqRel) {
            return Ok(session);
        }
        if let Some(process_group) = live.process_group {
            let result = unsafe { libc::kill(-process_group, libc::SIGHUP) };
            if result != 0 {
                live.stop_requested.store(false, Ordering::Release);
                return Err(TerminalError::StopFailed);
            }
        } else {
            let kill_result = match live.killer.lock() {
                Ok(mut killer) => killer.kill(),
                Err(_) => {
                    live.stop_requested.store(false, Ordering::Release);
                    return Err(TerminalError::Internal);
                }
            };
            if kill_result.is_err() {
                live.stop_requested.store(false, Ordering::Release);
                return Err(TerminalError::StopFailed);
            }
        }
        let delayed = live.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(2));
            if !delayed.finished.load(Ordering::Acquire) {
                if let Some(process_group) = delayed.process_group {
                    unsafe {
                        libc::kill(-process_group, libc::SIGKILL);
                    }
                } else if let Ok(mut killer) = delayed.killer.lock() {
                    let _ = killer.kill();
                }
            }
        });
        Ok(session)
    }

    pub fn delete(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
    ) -> Result<(), TerminalError> {
        self.repository.delete(workspace_id, terminal_id)
    }

    pub fn remove_workspace(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceRegistrySnapshot, TerminalError> {
        let _operation = self
            .operation_lock
            .lock()
            .map_err(|_| TerminalError::Internal)?;
        if self.repository.has_live(&workspace_id)? {
            return Err(TerminalError::WorkspaceHasRunningTerminals);
        }
        self.workspace_service
            .remove(workspace_id)
            .map_err(TerminalError::from_workspace)
    }

    fn live_terminal(&self, terminal_id: &TerminalId) -> Result<Arc<LiveTerminal>, TerminalError> {
        self.live
            .lock()
            .map_err(|_| TerminalError::Internal)?
            .get(terminal_id)
            .cloned()
            .ok_or(TerminalError::NotRunning)
    }
}

fn persist_chunk(repository: &TerminalRepository, terminal_id: &TerminalId, data: &[u8]) {
    if let Err(error) = repository.append_log(terminal_id, data, now_millis().unwrap_or(0)) {
        log::error!(
            target: "baibo::terminal",
            "terminal log persistence failed: {}",
            error.code()
        );
    }
}

fn persist_truncation_marker(
    repository: &TerminalRepository,
    terminal_id: &TerminalId,
    data: &[u8],
) {
    if let Err(error) =
        repository.append_truncation_marker(terminal_id, data, now_millis().unwrap_or(0))
    {
        log::error!(
            target: "baibo::terminal",
            "terminal truncation marker persistence failed: {}",
            error.code()
        );
    }
}

fn terminate_spawned_child(child: &mut dyn Child, process_group: Option<libc::pid_t>) {
    let group_killed = process_group
        .map(|process_group| unsafe { libc::kill(-process_group, libc::SIGKILL) == 0 })
        .unwrap_or(false);
    if !group_killed {
        let _ = child.kill();
    }
    let _ = child.wait();
}

fn broadcast_output(live: &LiveTerminal, data: Vec<u8>) {
    if let Ok(mut subscribers) = live.subscribers.lock() {
        subscribers.retain(|_, handle| match handle.output.try_send(data.clone()) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                handle.lagged.store(true, Ordering::Release);
                true
            }
            Err(TrySendError::Disconnected(_)) => false,
        });
    }
}

fn broadcast_event(live: &LiveTerminal, event: TerminalEvent) {
    if let Ok(mut subscribers) = live.subscribers.lock() {
        subscribers.retain(|_, handle| handle.subscriber.send_event(event.clone()).is_ok());
    }
}

fn validate_size(cols: u16, rows: u16) -> Result<(), TerminalError> {
    if !(2..=500).contains(&cols) || !(1..=200).contains(&rows) {
        return Err(TerminalError::InvalidSize);
    }
    Ok(())
}

fn pty_size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn login_shell() -> Result<PathBuf, TerminalError> {
    let shell = system_login_shell().unwrap_or_else(|| PathBuf::from("/bin/zsh"));
    if is_executable(&shell) {
        Ok(shell)
    } else {
        let fallback = PathBuf::from("/bin/zsh");
        if is_executable(&fallback) {
            Ok(fallback)
        } else {
            Err(TerminalError::ShellUnavailable)
        }
    }
}

fn system_login_shell() -> Option<PathBuf> {
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
        let value = std::ffi::CStr::from_ptr(entry.pw_shell);
        Some(PathBuf::from(std::ffi::OsStr::from_bytes(value.to_bytes())))
    }
}

fn is_executable(path: &Path) -> bool {
    path.is_absolute()
        && fs::metadata(path)
            .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

fn path_to_string(path: &Path) -> Result<String, TerminalError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or(TerminalError::ShellUnavailable)
}

fn now_millis() -> Result<i64, TerminalError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| TerminalError::Clock)?
        .as_millis();
    i64::try_from(millis).map_err(|_| TerminalError::Clock)
}

#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("终端输出通道已关闭")]
    ChannelClosed,
    #[error("终端数据库不可用")]
    Database,
    #[error("终端输入无效")]
    InvalidInput,
    #[error("终端启动规格无效")]
    InvalidLaunchSpec,
    #[error("终端状态转换无效")]
    InvalidTransition,
    #[error("终端尺寸必须为 2–500 列、1–200 行")]
    InvalidSize,
    #[error("终端内部状态不可用")]
    Internal,
    #[error("未找到终端 {0}")]
    NotFound(TerminalId),
    #[error("终端进程未在运行")]
    NotRunning,
    #[error("无法调整终端尺寸")]
    ResizeFailed,
    #[error("无法找到可用的登录 Shell")]
    ShellUnavailable,
    #[error("无法启动终端进程")]
    SpawnFailed,
    #[error("无法停止终端进程")]
    StopFailed,
    #[error("运行中的终端不能删除")]
    StillRunning,
    #[error("工作空间仍有运行中的终端，请先停止它们")]
    WorkspaceHasRunningTerminals,
    #[error("无法写入终端")]
    WriteFailed,
    #[error("系统时间不可用")]
    Clock,
    #[error("{0}")]
    Workspace(WorkspaceError),
}

impl TerminalError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ChannelClosed => "terminal_channel_closed",
            Self::Database => "terminal_database_unavailable",
            Self::InvalidInput => "invalid_terminal_input",
            Self::InvalidLaunchSpec => "invalid_terminal_launch_spec",
            Self::InvalidTransition => "invalid_terminal_transition",
            Self::InvalidSize => "invalid_terminal_size",
            Self::Internal => "terminal_internal",
            Self::NotFound(_) => "terminal_not_found",
            Self::NotRunning => "terminal_not_running",
            Self::ResizeFailed => "terminal_resize_failed",
            Self::ShellUnavailable => "shell_unavailable",
            Self::SpawnFailed => "terminal_spawn_failed",
            Self::StopFailed => "terminal_stop_failed",
            Self::StillRunning => "terminal_still_running",
            Self::WorkspaceHasRunningTerminals => "workspace_has_running_terminals",
            Self::WriteFailed => "terminal_write_failed",
            Self::Clock => "clock_unavailable",
            Self::Workspace(error) => error.code(),
        }
    }

    pub fn database(_error: rusqlite::Error) -> Self {
        Self::Database
    }

    pub fn from_workspace(error: WorkspaceError) -> Self {
        Self::Workspace(error)
    }

    pub fn not_found(id: &TerminalId) -> Self {
        Self::NotFound(id.clone())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalCommandError {
    pub code: &'static str,
    pub message: String,
}

impl From<TerminalError> for TerminalCommandError {
    fn from(error: TerminalError) -> Self {
        log::error!(
            target: "baibo::terminal",
            "terminal operation failed: {}",
            error.code()
        );
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use std::{
        fs,
        sync::{Arc, Mutex},
        thread,
        time::{Duration, Instant},
    };

    use tempfile::TempDir;

    use crate::{
        domain::{
            terminal::{TerminalEvent, TerminalStatus},
            workspace::WorkspaceId,
        },
        persistence::{Database, TerminalRepository, WorkspaceRepository},
        services::workspace::WorkspaceService,
    };

    use super::{PtyBackend, SpawnedPty, TerminalError, TerminalManager, TerminalSubscriber};

    #[derive(Default)]
    struct RecordingSubscriber {
        output: Mutex<Vec<u8>>,
        events: Mutex<Vec<TerminalEvent>>,
    }

    impl TerminalSubscriber for RecordingSubscriber {
        fn send_output(&self, data: Vec<u8>) -> Result<(), ()> {
            self.output.lock().map_err(|_| ())?.extend(data);
            Ok(())
        }

        fn send_event(&self, event: TerminalEvent) -> Result<(), ()> {
            self.events.lock().map_err(|_| ())?.push(event);
            Ok(())
        }
    }

    struct Context {
        _temp: TempDir,
        manager: TerminalManager,
        workspace_id: WorkspaceId,
    }

    struct FailingBackend;

    impl PtyBackend for FailingBackend {
        fn spawn(
            &self,
            _session: &crate::domain::terminal::TerminalSession,
            _launch: &super::LaunchSpec,
        ) -> Result<SpawnedPty, TerminalError> {
            Err(TerminalError::SpawnFailed)
        }
    }

    impl Context {
        fn new() -> Self {
            let temp = TempDir::new().expect("temp directory");
            let app_data = temp.path().join("app-data").join("baibo");
            let database = Database::open(&app_data.join("baibo.sqlite3")).expect("open database");
            let workspace_path = temp.path().join("workspace");
            fs::create_dir(&workspace_path).expect("create workspace");
            let workspace_service =
                WorkspaceService::new(WorkspaceRepository::new(database.clone()), app_data);
            let snapshot = workspace_service
                .register_path(&workspace_path)
                .expect("register workspace");
            Self {
                _temp: temp,
                manager: TerminalManager::new(TerminalRepository::new(database), workspace_service),
                workspace_id: snapshot.active_workspace_id.expect("active workspace"),
            }
        }

        fn wait_for_status(
            &self,
            terminal_id: &crate::domain::terminal::TerminalId,
            expected: TerminalStatus,
        ) -> crate::domain::terminal::TerminalSession {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                let session = self
                    .manager
                    .list(&self.workspace_id)
                    .expect("list terminal")
                    .into_iter()
                    .find(|session| &session.id == terminal_id)
                    .expect("terminal record");
                if session.status == expected {
                    return session;
                }
                assert!(Instant::now() < deadline, "PTY status timed out");
                thread::sleep(Duration::from_millis(20));
            }
        }
    }

    #[test]
    fn injectable_backend_failure_is_persisted_without_panicking() {
        let mut context = Context::new();
        context.manager = TerminalManager::new_with_backend(
            context.manager.repository.clone(),
            context.manager.workspace_service.clone(),
            Arc::new(FailingBackend),
        );

        assert_eq!(
            context
                .manager
                .create(context.workspace_id.clone(), 80, 24)
                .expect_err("spawn must fail")
                .code(),
            "terminal_spawn_failed"
        );
        let sessions = context
            .manager
            .list(&context.workspace_id)
            .expect("list failed terminal");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].status, TerminalStatus::Failed);
        assert_eq!(
            sessions[0].termination_reason.as_deref(),
            Some("spawn_failed")
        );
    }

    #[test]
    fn real_pty_handles_bytes_resize_exit_and_process_group_stop() {
        let context = Context::new();
        let session = context
            .manager
            .create(context.workspace_id.clone(), 80, 24)
            .expect("create PTY");
        let subscriber = Arc::new(RecordingSubscriber::default());
        context
            .manager
            .attach(&context.workspace_id, &session.id, subscriber.clone())
            .expect("attach");
        context
            .manager
            .resize(&context.workspace_id, &session.id, 100, 32)
            .expect("resize");
        context
            .manager
            .input(
                &context.workspace_id,
                &session.id,
                b"printf '\\033[31mCP2-UTF8-\\344\\270\\255\\346\\226\\207\\033[0m\\n'; exit 7\n",
            )
            .expect("write command");
        let exited = context.wait_for_status(&session.id, TerminalStatus::Exited);
        thread::sleep(Duration::from_millis(50));
        let output = subscriber.output.lock().expect("output");

        assert_eq!(exited.exit_code, Some(7));
        assert_eq!((exited.cols, exited.rows), (100, 32));
        assert!(output.windows(5).any(|window| window == b"\x1b[31m"));
        assert!(String::from_utf8_lossy(&output).contains("CP2-UTF8-中文"));
        drop(output);

        let running = context
            .manager
            .create(context.workspace_id.clone(), 80, 24)
            .expect("create second PTY");
        assert_eq!(
            context
                .manager
                .remove_workspace(context.workspace_id.clone())
                .expect_err("running terminal blocks workspace removal")
                .code(),
            "workspace_has_running_terminals"
        );
        context
            .manager
            .stop(&context.workspace_id, &running.id)
            .expect("stop PTY");
        context.wait_for_status(&running.id, TerminalStatus::Stopped);
    }
}
