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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentLaunchMode {
    InteractivePty,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentIsolationMode {
    Workspace,
}
