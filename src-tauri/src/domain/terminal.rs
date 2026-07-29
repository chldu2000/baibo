use std::fmt;

use serde::{Deserialize, Serialize};

use super::workspace::WorkspaceId;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct TerminalId(String);

impl TerminalId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TerminalId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<String> for TerminalId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct TerminalSubscriptionId(String);

impl TerminalSubscriptionId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalStatus {
    Starting,
    Running,
    Exited,
    Failed,
    Stopped,
    Interrupted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionKind {
    Shell,
    Agent,
    Legacy,
}

impl SessionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shell => "shell",
            Self::Agent => "agent",
            Self::Legacy => "legacy",
        }
    }
}

impl TryFrom<&str> for SessionKind {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "shell" => Ok(Self::Shell),
            "agent" => Ok(Self::Agent),
            "legacy" => Ok(Self::Legacy),
            other => Err(format!("unknown session kind: {other}")),
        }
    }
}

impl TerminalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Exited => "exited",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
            Self::Interrupted => "interrupted",
        }
    }
}

impl TryFrom<&str> for TerminalStatus {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "exited" => Ok(Self::Exited),
            "failed" => Ok(Self::Failed),
            "stopped" => Ok(Self::Stopped),
            "interrupted" => Ok(Self::Interrupted),
            other => Err(format!("unknown terminal status: {other}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSession {
    pub id: TerminalId,
    pub workspace_id: WorkspaceId,
    pub title: String,
    pub shell: String,
    pub cwd: String,
    pub status: TerminalStatus,
    pub cols: u16,
    pub rows: u16,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub exit_code: Option<i32>,
    pub termination_reason: Option<String>,
    pub session_kind: SessionKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LifecycleEventKind {
    Created,
    Running,
    Exited,
    Failed,
    Stopped,
    Interrupted,
}

impl LifecycleEventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Running => "running",
            Self::Exited => "exited",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
            Self::Interrupted => "interrupted",
        }
    }
}

impl TryFrom<&str> for LifecycleEventKind {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "created" => Ok(Self::Created),
            "running" => Ok(Self::Running),
            "exited" => Ok(Self::Exited),
            "failed" => Ok(Self::Failed),
            "stopped" => Ok(Self::Stopped),
            "interrupted" => Ok(Self::Interrupted),
            other => Err(format!("unknown lifecycle event kind: {other}")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLifecycleEvent {
    pub terminal_id: TerminalId,
    pub sequence: i64,
    pub kind: LifecycleEventKind,
    pub status: TerminalStatus,
    pub occurred_at: i64,
    pub exit_code: Option<i32>,
    pub reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalLogCoverage {
    Complete,
    Truncated,
    Unknown,
}

impl TryFrom<&str> for TerminalLogCoverage {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "complete" => Ok(Self::Complete),
            "truncated" => Ok(Self::Truncated),
            "unknown" => Ok(Self::Unknown),
            other => Err(format!("unknown terminal log coverage: {other}")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalLogIndex {
    pub terminal_id: TerminalId,
    pub first_sequence: Option<i64>,
    pub last_sequence: Option<i64>,
    pub chunk_count: i64,
    pub retained_bytes: i64,
    pub coverage: TerminalLogCoverage,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalAttachment {
    pub subscription_id: TerminalSubscriptionId,
    pub session: TerminalSession,
}

#[derive(Clone, Debug, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "event",
    content = "data"
)]
pub enum TerminalEvent {
    SessionUpdated { session: TerminalSession },
    OutputLagged { terminal_id: TerminalId },
}

pub struct NewTerminalSession {
    pub id: TerminalId,
    pub workspace_id: WorkspaceId,
    pub title: String,
    pub auto_title: bool,
    pub shell: String,
    pub cwd: String,
    pub cols: u16,
    pub rows: u16,
    pub now: i64,
    pub session_kind: SessionKind,
}
