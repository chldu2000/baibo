use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

use crate::{
    domain::{
        agent::AgentSessionId,
        provider::{CapabilitySupport, ProviderCapabilities, ProviderId, ProviderLaunchMode},
        workspace::WorkspaceId,
    },
    services::terminal::LaunchSpec,
};

pub(crate) trait ProviderAdapter: Send + Sync {
    fn id(&self) -> ProviderId;
    fn display_name(&self) -> &'static str;
    fn executable_name(&self) -> &'static str;
    fn version_args(&self) -> &'static [&'static str];
    fn help_args(&self) -> Option<&'static [&'static str]>;
    fn validate_help(&self, _help: &str) -> bool {
        true
    }
    fn capabilities(&self) -> ProviderCapabilities;
    fn supported_launch_modes(&self) -> Vec<ProviderLaunchMode>;

    fn build_launch_spec(
        &self,
        executable: &Path,
        environment: &BTreeMap<OsString, OsString>,
        cwd: &Path,
        workspace_id: &WorkspaceId,
        agent_session_id: &AgentSessionId,
    ) -> LaunchSpec {
        let mut environment = environment.clone();
        environment.insert(
            OsString::from("BAIBO_WORKSPACE_ID"),
            OsString::from(workspace_id.as_str()),
        );
        environment.insert(
            OsString::from("BAIBO_AGENT_SESSION_ID"),
            OsString::from(agent_session_id.as_str()),
        );
        environment.insert(
            OsString::from("BAIBO_PROVIDER_ID"),
            OsString::from(self.id().as_str()),
        );
        LaunchSpec {
            executable: PathBuf::from(executable),
            argv: Vec::new(),
            cwd: PathBuf::from(cwd),
            environment: Some(environment),
        }
    }
}

pub(crate) struct CodexAdapter;

impl ProviderAdapter for CodexAdapter {
    fn id(&self) -> ProviderId {
        ProviderId::Codex
    }

    fn display_name(&self) -> &'static str {
        "Codex"
    }

    fn executable_name(&self) -> &'static str {
        "codex"
    }

    fn version_args(&self) -> &'static [&'static str] {
        &["--version"]
    }

    fn help_args(&self) -> Option<&'static [&'static str]> {
        None
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            interactive_pty: CapabilitySupport::Supported,
            native_resume: CapabilitySupport::Supported,
            structured_events: CapabilitySupport::Experimental,
            approvals: CapabilitySupport::Supported,
            mcp: CapabilitySupport::Supported,
            rpc: CapabilitySupport::Unsupported,
            extensions: CapabilitySupport::Unsupported,
            skills: CapabilitySupport::Supported,
            project_trust: CapabilitySupport::Unsupported,
        }
    }

    fn supported_launch_modes(&self) -> Vec<ProviderLaunchMode> {
        vec![ProviderLaunchMode::InteractivePty]
    }
}

pub(crate) struct PiAdapter;

impl ProviderAdapter for PiAdapter {
    fn id(&self) -> ProviderId {
        ProviderId::Pi
    }

    fn display_name(&self) -> &'static str {
        "Pi"
    }

    fn executable_name(&self) -> &'static str {
        "pi"
    }

    fn version_args(&self) -> &'static [&'static str] {
        &["--version"]
    }

    fn help_args(&self) -> Option<&'static [&'static str]> {
        Some(&["--help"])
    }

    fn validate_help(&self, help: &str) -> bool {
        ["--mode", "rpc", "--approve", "--no-approve", "--offline"]
            .iter()
            .all(|flag| help.contains(flag))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            interactive_pty: CapabilitySupport::Supported,
            native_resume: CapabilitySupport::Supported,
            structured_events: CapabilitySupport::Experimental,
            approvals: CapabilitySupport::Supported,
            mcp: CapabilitySupport::Unsupported,
            rpc: CapabilitySupport::Experimental,
            extensions: CapabilitySupport::Supported,
            skills: CapabilitySupport::Supported,
            project_trust: CapabilitySupport::Supported,
        }
    }

    fn supported_launch_modes(&self) -> Vec<ProviderLaunchMode> {
        vec![ProviderLaunchMode::InteractivePty, ProviderLaunchMode::Rpc]
    }
}

pub(crate) fn adapters() -> Vec<Box<dyn ProviderAdapter>> {
    vec![Box::new(CodexAdapter), Box::new(PiAdapter)]
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, ffi::OsString, path::Path};

    use crate::domain::{agent::AgentSessionId, provider::ProviderId, workspace::WorkspaceId};

    use super::{CodexAdapter, PiAdapter, ProviderAdapter};

    #[test]
    fn adapters_build_direct_launch_specs_without_shell_arguments() {
        let mut environment = BTreeMap::new();
        environment.insert(OsString::from("HOME"), OsString::from("/private/home"));
        environment.insert(
            OsString::from("PROVIDER_SECRET"),
            OsString::from("memory-only"),
        );
        let workspace_id = WorkspaceId::from("workspace-a".to_owned());
        let agent_id = AgentSessionId::new();

        for adapter in [
            &CodexAdapter as &dyn ProviderAdapter,
            &PiAdapter as &dyn ProviderAdapter,
        ] {
            let spec = adapter.build_launch_spec(
                Path::new("/usr/local/bin/provider"),
                &environment,
                Path::new("/workspace"),
                &workspace_id,
                &agent_id,
            );

            assert!(spec.argv.is_empty());
            assert_eq!(spec.cwd, Path::new("/workspace"));
            assert_eq!(
                spec.environment
                    .as_ref()
                    .and_then(|values| values.get(&OsString::from("PROVIDER_SECRET"))),
                Some(&OsString::from("memory-only"))
            );
            assert_eq!(
                spec.environment
                    .as_ref()
                    .and_then(|values| values.get(&OsString::from("BAIBO_PROVIDER_ID"))),
                Some(&OsString::from(adapter.id().as_str()))
            );
        }
        assert_eq!(CodexAdapter.id(), ProviderId::Codex);
        assert_eq!(PiAdapter.id(), ProviderId::Pi);
    }
}
