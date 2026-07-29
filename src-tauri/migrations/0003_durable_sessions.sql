ALTER TABLE terminal_sessions
ADD COLUMN session_kind TEXT NOT NULL DEFAULT 'legacy'
CHECK (session_kind IN ('shell', 'agent', 'legacy'));

UPDATE terminal_sessions
SET session_kind = 'shell'
WHERE title GLOB 'Shell [0-9]*';

CREATE TABLE agent_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    terminal_id TEXT NOT NULL UNIQUE REFERENCES terminal_sessions(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL CHECK (provider_id IN ('codex', 'pi')),
    provider_session_id TEXT,
    launch_mode TEXT NOT NULL CHECK (launch_mode = 'interactive_pty'),
    isolation_mode TEXT NOT NULL CHECK (isolation_mode = 'workspace'),
    restarted_from_session_id TEXT REFERENCES agent_sessions(id) ON DELETE SET NULL,
    executable_path TEXT NOT NULL,
    launch_argv_json TEXT NOT NULL,
    provider_version TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX agent_sessions_workspace_order
ON agent_sessions(workspace_id, created_at DESC, id ASC);

CREATE TABLE terminal_lifecycle_events (
    terminal_id TEXT NOT NULL REFERENCES terminal_sessions(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (
        kind IN ('created', 'running', 'exited', 'failed', 'stopped', 'interrupted')
    ),
    status TEXT NOT NULL CHECK (
        status IN ('starting', 'running', 'exited', 'failed', 'stopped', 'interrupted')
    ),
    occurred_at INTEGER NOT NULL,
    exit_code INTEGER,
    reason TEXT,
    dedupe_key TEXT NOT NULL,
    PRIMARY KEY (terminal_id, sequence),
    UNIQUE (terminal_id, dedupe_key)
);

CREATE INDEX terminal_lifecycle_events_order
ON terminal_lifecycle_events(terminal_id, sequence);

INSERT INTO terminal_lifecycle_events (
    terminal_id, sequence, kind, status, occurred_at, exit_code, reason, dedupe_key
)
SELECT
    id,
    1,
    CASE WHEN status = 'starting' THEN 'created' ELSE status END,
    status,
    COALESCE(ended_at, started_at, created_at),
    exit_code,
    termination_reason,
    'migration_v3'
FROM terminal_sessions;

CREATE TABLE terminal_log_index (
    terminal_id TEXT PRIMARY KEY NOT NULL
        REFERENCES terminal_sessions(id) ON DELETE CASCADE,
    next_sequence INTEGER NOT NULL CHECK (next_sequence >= 1),
    first_sequence INTEGER,
    last_sequence INTEGER,
    chunk_count INTEGER NOT NULL CHECK (chunk_count >= 0),
    retained_bytes INTEGER NOT NULL CHECK (retained_bytes >= 0),
    coverage TEXT NOT NULL CHECK (coverage IN ('complete', 'truncated', 'unknown')),
    updated_at INTEGER NOT NULL,
    CHECK (
        (chunk_count = 0 AND first_sequence IS NULL AND last_sequence IS NULL)
        OR
        (chunk_count > 0 AND first_sequence IS NOT NULL AND last_sequence IS NOT NULL)
    )
);

INSERT INTO terminal_log_index (
    terminal_id, next_sequence, first_sequence, last_sequence,
    chunk_count, retained_bytes, coverage, updated_at
)
SELECT
    terminal.id,
    COALESCE(MAX(chunk.sequence), 0) + 1,
    MIN(chunk.sequence),
    MAX(chunk.sequence),
    COUNT(chunk.sequence),
    COALESCE(SUM(chunk.byte_length), 0),
    'unknown',
    COALESCE(MAX(chunk.created_at), terminal.created_at)
FROM terminal_sessions AS terminal
LEFT JOIN terminal_log_chunks AS chunk ON chunk.terminal_id = terminal.id
GROUP BY terminal.id;
