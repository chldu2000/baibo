use std::fmt;

use serde::{Deserialize, Serialize};

use super::{provider::ProviderId, terminal::TerminalSession, workspace::WorkspaceId};

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct AgentSessionId(String);

impl AgentSessionId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentSessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<String> for AgentSessionId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSession {
    pub id: AgentSessionId,
    pub workspace_id: WorkspaceId,
    pub provider_id: ProviderId,
    pub provider_session_id: Option<String>,
    pub terminal: TerminalSession,
    pub launch_mode: AgentLaunchMode,
    pub isolation_mode: AgentIsolationMode,
    pub restarted_from_session_id: Option<AgentSessionId>,
    pub created_at: i64,
    pub launch_snapshot: AgentLaunchSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentLaunchSnapshot {
    pub executable_path: Option<String>,
    pub argv: Vec<String>,
    pub provider_version: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NewAgentSession {
    pub id: AgentSessionId,
    pub workspace_id: WorkspaceId,
    pub provider_id: ProviderId,
    pub provider_session_id: Option<String>,
    pub launch_mode: AgentLaunchMode,
    pub isolation_mode: AgentIsolationMode,
    pub restarted_from_session_id: Option<AgentSessionId>,
    pub created_at: i64,
    pub launch_snapshot: AgentLaunchSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentLaunchMode {
    InteractivePty,
}

impl AgentLaunchMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InteractivePty => "interactive_pty",
        }
    }
}

impl TryFrom<&str> for AgentLaunchMode {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "interactive_pty" => Ok(Self::InteractivePty),
            other => Err(format!("unknown agent launch mode: {other}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentIsolationMode {
    Workspace,
}

impl AgentIsolationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
        }
    }
}

impl TryFrom<&str> for AgentIsolationMode {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "workspace" => Ok(Self::Workspace),
            other => Err(format!("unknown agent isolation mode: {other}")),
        }
    }
}
