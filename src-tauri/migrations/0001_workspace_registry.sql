CREATE TABLE workspaces (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 128),
    canonical_path TEXT NOT NULL UNIQUE,
    repository_root TEXT,
    git_repository INTEGER NOT NULL CHECK (git_repository IN (0, 1)),
    created_at INTEGER NOT NULL,
    last_opened_at INTEGER NOT NULL,
    CHECK (
        (git_repository = 0 AND repository_root IS NULL)
        OR (git_repository = 1 AND repository_root IS NOT NULL)
    )
);

CREATE TABLE application_state (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    active_workspace_id TEXT REFERENCES workspaces(id) ON DELETE SET NULL
);

INSERT INTO application_state (id, active_workspace_id) VALUES (1, NULL);
