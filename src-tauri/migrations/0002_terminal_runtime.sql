CREATE TABLE terminal_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    shell TEXT NOT NULL,
    cwd TEXT NOT NULL,
    status TEXT NOT NULL CHECK (
        status IN ('starting', 'running', 'exited', 'failed', 'stopped', 'interrupted')
    ),
    cols INTEGER NOT NULL CHECK (cols BETWEEN 2 AND 500),
    rows INTEGER NOT NULL CHECK (rows BETWEEN 1 AND 200),
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    ended_at INTEGER,
    exit_code INTEGER,
    termination_reason TEXT
);

CREATE INDEX terminal_sessions_workspace_order
ON terminal_sessions(workspace_id, created_at DESC, id ASC);

CREATE TABLE terminal_log_chunks (
    terminal_id TEXT NOT NULL REFERENCES terminal_sessions(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL,
    data BLOB NOT NULL,
    byte_length INTEGER NOT NULL CHECK (byte_length >= 0),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (terminal_id, sequence)
);

CREATE INDEX terminal_log_chunks_terminal_order
ON terminal_log_chunks(terminal_id, sequence);
