use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use git2::{ErrorCode as GitErrorCode, Repository};
use serde::Serialize;
use thiserror::Error;

use crate::{
    domain::workspace::{
        GitMetadata, NewWorkspace, Workspace, WorkspaceId, WorkspaceRegistrySnapshot,
    },
    persistence::WorkspaceRepository,
};

#[derive(Clone)]
pub struct WorkspaceService {
    repository: WorkspaceRepository,
    app_data_root: PathBuf,
}

impl WorkspaceService {
    pub fn new(repository: WorkspaceRepository, app_data_root: PathBuf) -> Self {
        Self {
            repository,
            app_data_root,
        }
    }

    pub fn list(&self) -> Result<WorkspaceRegistrySnapshot, WorkspaceError> {
        self.repository.snapshot()
    }

    pub fn get_registered(&self, id: &WorkspaceId) -> Result<Workspace, WorkspaceError> {
        self.repository.get(id)
    }

    pub fn resolve_for_terminal(&self, id: &WorkspaceId) -> Result<Workspace, WorkspaceError> {
        let workspace = self.repository.get(id)?;
        self.validate_path(Path::new(&workspace.canonical_path))
            .map_err(|error| match error {
                WorkspaceError::PathNotFound
                | WorkspaceError::NotDirectory
                | WorkspaceError::PathUnreadable(_) => {
                    WorkspaceError::unavailable(workspace.name.clone())
                }
                other => other,
            })?;
        Ok(workspace)
    }

    pub fn register_path(
        &self,
        selected_path: &Path,
    ) -> Result<WorkspaceRegistrySnapshot, WorkspaceError> {
        let validated = self.validate_path(selected_path)?;
        let name = validated
            .canonical_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(WorkspaceError::unsupported_path)?
            .to_owned();
        let name = validate_name(&name)?;

        self.repository.register(NewWorkspace {
            id: WorkspaceId::new(),
            name,
            canonical_path: path_to_string(&validated.canonical_path)?,
            repository_root: validated.git.repository_root,
            git_repository: validated.git.git_repository,
            now: now_millis()?,
        })
    }

    pub fn open(&self, id: WorkspaceId) -> Result<WorkspaceRegistrySnapshot, WorkspaceError> {
        let workspace = self.repository.get(&id)?;
        let validated = self
            .validate_path(Path::new(&workspace.canonical_path))
            .map_err(|error| match error {
                WorkspaceError::PathNotFound
                | WorkspaceError::NotDirectory
                | WorkspaceError::PathUnreadable(_) => WorkspaceError::unavailable(workspace.name),
                other => other,
            })?;
        self.repository.open(&id, validated.git, now_millis()?)
    }

    pub fn rename(
        &self,
        id: WorkspaceId,
        name: &str,
    ) -> Result<WorkspaceRegistrySnapshot, WorkspaceError> {
        self.repository.rename(&id, &validate_name(name)?)
    }

    pub fn remove(&self, id: WorkspaceId) -> Result<WorkspaceRegistrySnapshot, WorkspaceError> {
        self.repository.remove(&id)
    }

    fn validate_path(&self, selected_path: &Path) -> Result<ValidatedPath, WorkspaceError> {
        let canonical_path =
            fs::canonicalize(selected_path).map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => WorkspaceError::PathNotFound,
                std::io::ErrorKind::PermissionDenied => {
                    WorkspaceError::PathUnreadable("没有权限访问该目录".into())
                }
                _ => WorkspaceError::PathUnreadable(error.to_string()),
            })?;
        let metadata = fs::metadata(&canonical_path)
            .map_err(|error| WorkspaceError::PathUnreadable(error.to_string()))?;
        if !metadata.is_dir() {
            return Err(WorkspaceError::NotDirectory);
        }
        if canonical_path.parent().is_none() {
            return Err(WorkspaceError::FilesystemRoot);
        }

        let app_data_root =
            fs::canonicalize(&self.app_data_root).unwrap_or_else(|_| self.app_data_root.clone());
        if canonical_path.starts_with(&app_data_root) {
            return Err(WorkspaceError::AppDataDirectory);
        }

        fs::read_dir(&canonical_path)
            .map_err(|error| WorkspaceError::PathUnreadable(error.to_string()))?;
        path_to_string(&canonical_path)?;
        let git = detect_git(&canonical_path)?;

        Ok(ValidatedPath {
            canonical_path,
            git,
        })
    }
}

struct ValidatedPath {
    canonical_path: PathBuf,
    git: GitMetadata,
}

fn detect_git(path: &Path) -> Result<GitMetadata, WorkspaceError> {
    match Repository::discover(path) {
        Ok(repository) => {
            let root = repository
                .workdir()
                .unwrap_or_else(|| repository.path())
                .canonicalize()
                .map_err(|error| WorkspaceError::GitDetection(error.to_string()))?;
            Ok(GitMetadata {
                repository_root: Some(path_to_string(&root)?),
                git_repository: true,
            })
        }
        Err(error) if error.code() == GitErrorCode::NotFound => Ok(GitMetadata {
            repository_root: None,
            git_repository: false,
        }),
        Err(error) => Err(WorkspaceError::GitDetection(error.message().to_owned())),
    }
}

fn validate_name(name: &str) -> Result<String, WorkspaceError> {
    let trimmed = name.trim();
    let length = trimmed.chars().count();
    if length == 0 {
        return Err(WorkspaceError::InvalidName("工作空间名称不能为空".into()));
    }
    if length > 128 {
        return Err(WorkspaceError::InvalidName(
            "工作空间名称不能超过 128 个字符".into(),
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(WorkspaceError::InvalidName(
            "工作空间名称不能包含控制字符".into(),
        ));
    }
    Ok(trimmed.to_owned())
}

fn path_to_string(path: &Path) -> Result<String, WorkspaceError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(WorkspaceError::unsupported_path)
}

fn now_millis() -> Result<i64, WorkspaceError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| WorkspaceError::Clock(error.to_string()))?
        .as_millis();
    i64::try_from(millis).map_err(|_| WorkspaceError::Clock("系统时间超出支持范围".into()))
}

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("应用数据目录不能注册为工作空间")]
    AppDataDirectory,
    #[error("工作空间数据库不可用")]
    Database(String),
    #[error("工作空间“{0}”已经注册")]
    DuplicateWorkspace(String),
    #[error("不能注册文件系统根目录")]
    FilesystemRoot,
    #[error("无法识别 Git 仓库")]
    GitDetection(String),
    #[error("{0}")]
    InvalidName(String),
    #[error("所选路径不是目录")]
    NotDirectory,
    #[error("所选目录不存在")]
    PathNotFound,
    #[error("无法读取所选目录")]
    PathUnreadable(String),
    #[error("数据库迁移失败")]
    Migration(String),
    #[error("不支持该路径编码")]
    UnsupportedPath,
    #[error("工作空间“{0}”当前不可用，请恢复目录后重试或移除登记")]
    WorkspaceUnavailable(String),
    #[error("未找到工作空间 {0}")]
    WorkspaceNotFound(WorkspaceId),
    #[error("系统时间不可用")]
    Clock(String),
}

impl WorkspaceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::AppDataDirectory => "app_data_directory",
            Self::Database(_) => "database_unavailable",
            Self::DuplicateWorkspace(_) => "duplicate_workspace",
            Self::FilesystemRoot => "filesystem_root",
            Self::GitDetection(_) => "git_detection_failed",
            Self::InvalidName(_) => "invalid_name",
            Self::NotDirectory => "not_directory",
            Self::PathNotFound => "path_not_found",
            Self::PathUnreadable(_) => "path_unreadable",
            Self::Migration(_) => "migration_failed",
            Self::UnsupportedPath => "unsupported_path_encoding",
            Self::WorkspaceUnavailable(_) => "workspace_unavailable",
            Self::WorkspaceNotFound(_) => "workspace_not_found",
            Self::Clock(_) => "clock_unavailable",
        }
    }

    pub fn safe_message(&self) -> String {
        self.to_string()
    }

    pub fn database(detail: String) -> Self {
        Self::Database(detail)
    }

    pub fn migration(detail: String) -> Self {
        Self::Migration(detail)
    }

    pub fn from_database(error: rusqlite::Error) -> Self {
        Self::Database(error.to_string())
    }

    pub fn duplicate(name: String) -> Self {
        Self::DuplicateWorkspace(name)
    }

    pub fn not_found(id: &WorkspaceId) -> Self {
        Self::WorkspaceNotFound(id.clone())
    }

    fn unavailable(name: String) -> Self {
        Self::WorkspaceUnavailable(name)
    }

    fn unsupported_path() -> Self {
        Self::UnsupportedPath
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl From<WorkspaceError> for CommandError {
    fn from(error: WorkspaceError) -> Self {
        log::error!(
            target: "baibo::workspace",
            "workspace operation failed: {}",
            error.code()
        );
        Self {
            code: error.code(),
            message: error.safe_message(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path, sync::Arc};

    use tempfile::TempDir;

    use crate::persistence::{Database, WorkspaceRepository};

    use super::{validate_name, WorkspaceService};

    struct Context {
        _temp: TempDir,
        service: WorkspaceService,
        workspace_root: std::path::PathBuf,
        database_path: std::path::PathBuf,
        app_data_root: std::path::PathBuf,
    }

    impl Context {
        fn new() -> Self {
            let temp = TempDir::new().expect("temp directory");
            let app_data_root = temp.path().join("app-data").join("baibo");
            let database_path = app_data_root.join("baibo.sqlite3");
            let workspace_root = temp.path().join("workspaces");
            fs::create_dir_all(&workspace_root).expect("workspace root");
            let database = Database::open(&database_path).expect("database");
            let repository = WorkspaceRepository::new(database);
            let service = WorkspaceService::new(repository, app_data_root.clone());

            Self {
                _temp: temp,
                service,
                workspace_root,
                database_path,
                app_data_root,
            }
        }

        fn directory(&self, name: &str) -> std::path::PathBuf {
            let path = self.workspace_root.join(name);
            fs::create_dir_all(&path).expect("workspace directory");
            path
        }
    }

    fn repository_with_commit(path: &Path) -> git2::Repository {
        let repository = git2::Repository::init(path).expect("git repository");
        fs::write(path.join("README.md"), "fixture").expect("fixture");
        let mut index = repository.index().expect("index");
        index.add_path(Path::new("README.md")).expect("add fixture");
        index.write().expect("write index");
        let tree_id = index.write_tree().expect("write tree");
        let tree = repository.find_tree(tree_id).expect("tree");
        let signature = git2::Signature::now("Baibo Test", "test@baibo.local").expect("signature");
        repository
            .commit(Some("HEAD"), &signature, &signature, "fixture", &tree, &[])
            .expect("commit");
        drop(tree);
        repository
    }

    #[test]
    fn registers_git_and_non_git_directories_and_activates_them() {
        let context = Context::new();
        let plain = context.directory("plain");
        let git = context.directory("git");
        git2::Repository::init(&git).expect("git repository");

        let first = context.service.register_path(&plain).expect("plain");
        let second = context.service.register_path(&git).expect("git");

        assert_eq!(first.workspaces.len(), 1);
        assert!(!first.workspaces[0].git_repository);
        assert_eq!(second.workspaces.len(), 2);
        let registered_git = second
            .workspaces
            .iter()
            .find(|workspace| workspace.name == "git")
            .expect("registered git");
        let canonical_git = git.canonicalize().expect("canonical git");
        assert!(registered_git.git_repository);
        assert_eq!(
            registered_git.repository_root.as_deref(),
            canonical_git.to_str()
        );
        assert_eq!(
            second.active_workspace_id.as_ref(),
            Some(&registered_git.id)
        );
    }

    #[test]
    fn keeps_selected_subdirectory_and_discovers_repository_root() {
        let context = Context::new();
        let repository_root = context.directory("repo");
        git2::Repository::init(&repository_root).expect("git repository");
        let nested = repository_root.join("packages").join("app");
        fs::create_dir_all(&nested).expect("nested workspace");

        let snapshot = context.service.register_path(&nested).expect("register");
        let workspace = &snapshot.workspaces[0];
        let canonical_nested = nested.canonicalize().expect("canonical nested");
        let canonical_root = repository_root.canonicalize().expect("canonical root");

        assert_eq!(workspace.canonical_path, canonical_nested.to_string_lossy());
        assert_eq!(
            workspace.repository_root.as_deref(),
            canonical_root.to_str()
        );
    }

    #[test]
    fn rejects_duplicate_canonical_paths_but_allows_siblings() {
        let context = Context::new();
        let repository_root = context.directory("repo");
        git2::Repository::init(&repository_root).expect("git repository");
        let first = repository_root.join("first");
        let second = repository_root.join("second");
        fs::create_dir_all(&first).expect("first");
        fs::create_dir_all(&second).expect("second");

        context
            .service
            .register_path(&first)
            .expect("first register");
        let duplicate = context
            .service
            .register_path(&first)
            .expect_err("duplicate");
        let snapshot = context
            .service
            .register_path(&second)
            .expect("second register");

        assert_eq!(duplicate.code(), "duplicate_workspace");
        assert_eq!(snapshot.workspaces.len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn resolves_symlinks_before_duplicate_detection() {
        use std::os::unix::fs::symlink;

        let context = Context::new();
        let directory = context.directory("real");
        let link = context.workspace_root.join("linked");
        symlink(&directory, &link).expect("symlink");

        context
            .service
            .register_path(&directory)
            .expect("real register");
        let error = context
            .service
            .register_path(&link)
            .expect_err("symlink duplicate");

        assert_eq!(error.code(), "duplicate_workspace");
    }

    #[test]
    fn validates_paths_and_names() {
        let context = Context::new();
        let file = context.workspace_root.join("file.txt");
        fs::write(&file, "content").expect("file");

        assert_eq!(
            context
                .service
                .register_path(&context.workspace_root.join("missing"))
                .expect_err("missing")
                .code(),
            "path_not_found"
        );
        assert_eq!(
            context
                .service
                .register_path(&file)
                .expect_err("file")
                .code(),
            "not_directory"
        );
        assert_eq!(
            context
                .service
                .register_path(Path::new("/"))
                .expect_err("root")
                .code(),
            "filesystem_root"
        );
        assert_eq!(
            context
                .service
                .register_path(&context.app_data_root)
                .expect_err("app data")
                .code(),
            "app_data_directory"
        );
        assert!(validate_name("  renamed  ").is_ok());
        assert_eq!(
            validate_name(" \n ").expect_err("empty").code(),
            "invalid_name"
        );
        assert_eq!(
            validate_name(&"a".repeat(129))
                .expect_err("too long")
                .code(),
            "invalid_name"
        );
        assert_eq!(
            validate_name("bad\u{0000}name")
                .expect_err("control")
                .code(),
            "invalid_name"
        );
    }

    #[test]
    fn renames_switches_and_removes_without_touching_files() {
        let context = Context::new();
        let first_path = context.directory("first");
        let second_path = context.directory("second");
        let marker = first_path.join("keep.txt");
        fs::write(&marker, "keep").expect("marker");
        let first = context.service.register_path(&first_path).expect("first");
        let first_id = first.workspaces[0].id.clone();
        let second = context.service.register_path(&second_path).expect("second");
        let second_id = second.active_workspace_id.expect("second active");

        let opened = context.service.open(first_id.clone()).expect("open first");
        assert_eq!(opened.active_workspace_id.as_ref(), Some(&first_id));
        let renamed = context
            .service
            .rename(first_id.clone(), "  项目一  ")
            .expect("rename");
        assert_eq!(renamed.workspaces[0].name, "项目一");
        let removed = context.service.remove(first_id).expect("remove");

        assert_eq!(removed.active_workspace_id.as_ref(), Some(&second_id));
        assert!(first_path.exists());
        assert!(marker.exists());
    }

    #[test]
    fn failed_workspace_ids_do_not_modify_another_workspace() {
        let context = Context::new();
        let directory = context.directory("kept");
        let original = context.service.register_path(&directory).expect("register");

        let error = context
            .service
            .rename(crate::domain::workspace::WorkspaceId::new(), "changed")
            .expect_err("unknown id");
        let after = context.service.list().expect("list");

        assert_eq!(error.code(), "workspace_not_found");
        assert_eq!(after, original);
    }

    #[test]
    fn keeps_registration_when_the_path_disappears() {
        let context = Context::new();
        let first_path = context.directory("first");
        let second_path = context.directory("second");
        let first = context.service.register_path(&first_path).expect("first");
        let first_id = first.active_workspace_id.expect("first active");
        let second = context.service.register_path(&second_path).expect("second");
        let second_id = second.active_workspace_id.expect("second active");
        fs::rename(&first_path, context.workspace_root.join("moved")).expect("move");

        let error = context
            .service
            .open(first_id.clone())
            .expect_err("unavailable");
        let snapshot = context.service.list().expect("list");

        assert_eq!(error.code(), "workspace_unavailable");
        assert_eq!(snapshot.active_workspace_id.as_ref(), Some(&second_id));
        assert!(snapshot
            .workspaces
            .iter()
            .any(|workspace| workspace.id == first_id));
    }

    #[test]
    fn persists_workspaces_and_active_selection_across_reopen() {
        let context = Context::new();
        let first = context.directory("first");
        let second = context.directory("second");
        context.service.register_path(&first).expect("first");
        let expected = context.service.register_path(&second).expect("second");
        let database_path = context.database_path.clone();
        let app_data_root = context.app_data_root.clone();
        drop(context.service);

        let database = Database::open(&database_path).expect("reopen database");
        let service = WorkspaceService::new(WorkspaceRepository::new(database), app_data_root);
        let actual = service.list().expect("restored snapshot");

        assert_eq!(actual, expected);
    }

    #[test]
    fn registers_a_bare_repository() {
        let context = Context::new();
        let bare = context.workspace_root.join("bare.git");
        git2::Repository::init_bare(&bare).expect("bare repository");

        let snapshot = context.service.register_path(&bare).expect("register bare");
        let workspace = &snapshot.workspaces[0];
        let canonical_bare = bare.canonicalize().expect("canonical bare");

        assert!(workspace.git_repository);
        assert_eq!(
            workspace.repository_root.as_deref(),
            canonical_bare.to_str()
        );
    }

    #[test]
    fn detects_a_linked_git_worktree() {
        let context = Context::new();
        let repository_path = context.directory("main");
        let repository = repository_with_commit(&repository_path);
        let worktree_path = context.workspace_root.join("linked-worktree");
        repository
            .worktree("cp1-linked", &worktree_path, None)
            .expect("linked worktree");

        let snapshot = context
            .service
            .register_path(&worktree_path)
            .expect("register worktree");
        let workspace = &snapshot.workspaces[0];
        let canonical_worktree = worktree_path.canonicalize().expect("canonical worktree");

        assert!(workspace.git_repository);
        assert_eq!(
            workspace.repository_root.as_deref(),
            canonical_worktree.to_str()
        );
    }

    #[test]
    fn concurrent_duplicate_registration_has_one_winner() {
        let context = Context::new();
        let directory = context.directory("shared");
        let service = Arc::new(context.service.clone());
        let first_service = Arc::clone(&service);
        let first_path = directory.clone();
        let first = std::thread::spawn(move || first_service.register_path(&first_path));
        let second_service = Arc::clone(&service);
        let second = std::thread::spawn(move || second_service.register_path(&directory));

        let results = [
            first.join().expect("first thread"),
            second.join().expect("second thread"),
        ];
        let successes = results.iter().filter(|result| result.is_ok()).count();
        let duplicates = results
            .iter()
            .filter(|result| {
                result
                    .as_ref()
                    .is_err_and(|error| error.code() == "duplicate_workspace")
            })
            .count();

        assert_eq!(successes, 1);
        assert_eq!(duplicates, 1);
        assert_eq!(service.list().expect("snapshot").workspaces.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_utf8_path_encoding() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let invalid = std::path::PathBuf::from(OsString::from_vec(vec![b'i', b'n', b'v', 0xFF]));
        let error = super::path_to_string(&invalid).expect_err("non utf8");

        assert_eq!(error.code(), "unsupported_path_encoding");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_unreadable_directories() {
        use std::os::unix::fs::PermissionsExt;

        let context = Context::new();
        let directory = context.directory("unreadable");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o000))
            .expect("remove permissions");
        let result = context.service.register_path(&directory);
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .expect("restore permissions");

        let error = result.expect_err("unreadable directory");
        assert_eq!(error.code(), "path_unreadable");
    }
}
