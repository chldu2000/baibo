use rusqlite::{
    params, Connection, ErrorCode, OptionalExtension, Transaction, TransactionBehavior,
};

use crate::{
    domain::workspace::{
        GitMetadata, NewWorkspace, Workspace, WorkspaceId, WorkspaceRegistrySnapshot,
    },
    services::workspace::WorkspaceError,
};

use super::Database;

#[derive(Clone)]
pub struct WorkspaceRepository {
    database: Database,
}

impl WorkspaceRepository {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn snapshot(&self) -> Result<WorkspaceRegistrySnapshot, WorkspaceError> {
        let connection = self.database.lock()?;
        snapshot(&connection)
    }

    pub fn get(&self, id: &WorkspaceId) -> Result<Workspace, WorkspaceError> {
        let connection = self.database.lock()?;
        find_workspace(&connection, id)?.ok_or_else(|| WorkspaceError::not_found(id))
    }

    pub fn register(
        &self,
        workspace: NewWorkspace,
    ) -> Result<WorkspaceRegistrySnapshot, WorkspaceError> {
        let mut connection = self.database.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(WorkspaceError::from_database)?;

        if let Err(error) = transaction.execute(
            "INSERT INTO workspaces (
                id, name, canonical_path, repository_root, git_repository, created_at, last_opened_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            params![
                workspace.id.as_str(),
                workspace.name,
                workspace.canonical_path,
                workspace.repository_root,
                workspace.git_repository,
                workspace.now,
            ],
        ) {
            if error.sqlite_error_code() == Some(ErrorCode::ConstraintViolation) {
                if let Some(existing_name) = transaction
                    .query_row(
                        "SELECT name FROM workspaces WHERE canonical_path = ?1",
                        params![workspace.canonical_path],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()
                    .map_err(WorkspaceError::from_database)?
                {
                    return Err(WorkspaceError::duplicate(existing_name));
                }
            }
            return Err(WorkspaceError::from_database(error));
        }

        set_active(&transaction, Some(&workspace.id))?;
        let result = snapshot(&transaction)?;
        transaction
            .commit()
            .map_err(WorkspaceError::from_database)?;
        Ok(result)
    }

    pub fn open(
        &self,
        id: &WorkspaceId,
        git: GitMetadata,
        now: i64,
    ) -> Result<WorkspaceRegistrySnapshot, WorkspaceError> {
        let mut connection = self.database.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(WorkspaceError::from_database)?;
        let changed = transaction
            .execute(
                "UPDATE workspaces
                 SET repository_root = ?1,
                     git_repository = ?2,
                     last_opened_at = MAX(?3, last_opened_at + 1)
                 WHERE id = ?4",
                params![git.repository_root, git.git_repository, now, id.as_str()],
            )
            .map_err(WorkspaceError::from_database)?;
        ensure_changed(changed, id)?;
        set_active(&transaction, Some(id))?;
        let result = snapshot(&transaction)?;
        transaction
            .commit()
            .map_err(WorkspaceError::from_database)?;
        Ok(result)
    }

    pub fn rename(
        &self,
        id: &WorkspaceId,
        name: &str,
    ) -> Result<WorkspaceRegistrySnapshot, WorkspaceError> {
        let mut connection = self.database.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(WorkspaceError::from_database)?;
        let changed = transaction
            .execute(
                "UPDATE workspaces SET name = ?1 WHERE id = ?2",
                params![name, id.as_str()],
            )
            .map_err(WorkspaceError::from_database)?;
        ensure_changed(changed, id)?;
        let result = snapshot(&transaction)?;
        transaction
            .commit()
            .map_err(WorkspaceError::from_database)?;
        Ok(result)
    }

    pub fn remove(&self, id: &WorkspaceId) -> Result<WorkspaceRegistrySnapshot, WorkspaceError> {
        let mut connection = self.database.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(WorkspaceError::from_database)?;
        let active_id = active_workspace_id(&transaction)?;
        let changed = transaction
            .execute("DELETE FROM workspaces WHERE id = ?1", params![id.as_str()])
            .map_err(WorkspaceError::from_database)?;
        ensure_changed(changed, id)?;

        if active_id.as_ref() == Some(id) {
            let fallback = transaction
                .query_row(
                    "SELECT id FROM workspaces
                     ORDER BY last_opened_at DESC, created_at ASC, id ASC
                     LIMIT 1",
                    [],
                    |row| row.get::<_, String>(0).map(WorkspaceId::from),
                )
                .optional()
                .map_err(WorkspaceError::from_database)?;
            set_active(&transaction, fallback.as_ref())?;
        }

        let result = snapshot(&transaction)?;
        transaction
            .commit()
            .map_err(WorkspaceError::from_database)?;
        Ok(result)
    }
}

fn snapshot(connection: &Connection) -> Result<WorkspaceRegistrySnapshot, WorkspaceError> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, canonical_path, repository_root, git_repository,
                    created_at, last_opened_at
             FROM workspaces
             ORDER BY last_opened_at DESC, created_at ASC, id ASC",
        )
        .map_err(WorkspaceError::from_database)?;
    let workspaces = statement
        .query_map([], map_workspace)
        .map_err(WorkspaceError::from_database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(WorkspaceError::from_database)?;

    Ok(WorkspaceRegistrySnapshot {
        workspaces,
        active_workspace_id: active_workspace_id(connection)?,
    })
}

fn find_workspace(
    connection: &Connection,
    id: &WorkspaceId,
) -> Result<Option<Workspace>, WorkspaceError> {
    connection
        .query_row(
            "SELECT id, name, canonical_path, repository_root, git_repository,
                    created_at, last_opened_at
             FROM workspaces WHERE id = ?1",
            params![id.as_str()],
            map_workspace,
        )
        .optional()
        .map_err(WorkspaceError::from_database)
}

fn map_workspace(row: &rusqlite::Row<'_>) -> rusqlite::Result<Workspace> {
    Ok(Workspace {
        id: WorkspaceId::from(row.get::<_, String>(0)?),
        name: row.get(1)?,
        canonical_path: row.get(2)?,
        repository_root: row.get(3)?,
        git_repository: row.get(4)?,
        created_at: row.get(5)?,
        last_opened_at: row.get(6)?,
    })
}

fn active_workspace_id(connection: &Connection) -> Result<Option<WorkspaceId>, WorkspaceError> {
    connection
        .query_row(
            "SELECT active_workspace_id FROM application_state WHERE id = 1",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .map(|id| id.map(WorkspaceId::from))
        .map_err(WorkspaceError::from_database)
}

fn set_active(
    transaction: &Transaction<'_>,
    id: Option<&WorkspaceId>,
) -> Result<(), WorkspaceError> {
    transaction
        .execute(
            "UPDATE application_state SET active_workspace_id = ?1 WHERE id = 1",
            params![id.map(WorkspaceId::as_str)],
        )
        .map_err(WorkspaceError::from_database)?;
    Ok(())
}

fn ensure_changed(changed: usize, id: &WorkspaceId) -> Result<(), WorkspaceError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(WorkspaceError::not_found(id))
    }
}
