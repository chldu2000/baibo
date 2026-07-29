use serde::Serialize;

use super::{
    agent::AgentSession,
    terminal::{SessionLifecycleEvent, TerminalLogIndex, TerminalSession},
};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub terminal: TerminalSession,
    pub agent_session: Option<AgentSession>,
    pub lifecycle_events: Vec<SessionLifecycleEvent>,
    pub log_index: TerminalLogIndex,
}
