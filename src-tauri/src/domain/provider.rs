use std::fmt;

use serde::{Deserialize, Serialize};

use super::workspace::WorkspaceId;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderId {
    Codex,
    Pi,
}

impl ProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Pi => "pi",
        }
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderAvailability {
    #[allow(dead_code)]
    Checking,
    Available,
    Unavailable,
    Unsupported,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CapabilitySupport {
    Supported,
    Unsupported,
    Experimental,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub interactive_pty: CapabilitySupport,
    pub native_resume: CapabilitySupport,
    pub structured_events: CapabilitySupport,
    pub approvals: CapabilitySupport,
    pub mcp: CapabilitySupport,
    pub rpc: CapabilitySupport,
    pub extensions: CapabilitySupport,
    pub skills: CapabilitySupport,
    pub project_trust: CapabilitySupport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderLaunchMode {
    InteractivePty,
    Rpc,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDiagnostic {
    pub code: String,
    pub message: String,
    pub recovery: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: ProviderId,
    pub display_name: String,
    pub availability: ProviderAvailability,
    pub executable_path: Option<String>,
    pub version: Option<String>,
    pub launch_modes: Vec<ProviderLaunchMode>,
    pub capabilities: ProviderCapabilities,
    pub diagnostic: Option<ProviderDiagnostic>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PiProjectTrustState {
    NotRequired,
    Trusted,
    Denied,
    PromptRequired,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiProjectTrust {
    pub workspace_id: WorkspaceId,
    pub state: PiProjectTrustState,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiRpcProbeResult {
    pub provider_id: ProviderId,
    pub ok: bool,
    pub message: String,
    pub elapsed_ms: u64,
}
