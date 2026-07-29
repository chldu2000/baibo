use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct WorkspaceId(String);

impl WorkspaceId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<String> for WorkspaceId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    pub canonical_path: String,
    pub repository_root: Option<String>,
    pub git_repository: bool,
    pub created_at: i64,
    pub last_opened_at: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRegistrySnapshot {
    pub workspaces: Vec<Workspace>,
    pub active_workspace_id: Option<WorkspaceId>,
}

#[derive(Debug)]
pub struct NewWorkspace {
    pub id: WorkspaceId,
    pub name: String,
    pub canonical_path: String,
    pub repository_root: Option<String>,
    pub git_repository: bool,
    pub now: i64,
}

#[derive(Clone, Debug)]
pub struct GitMetadata {
    pub repository_root: Option<String>,
    pub git_repository: bool,
}
