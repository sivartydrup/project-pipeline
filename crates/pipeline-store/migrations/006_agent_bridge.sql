CREATE TABLE agent_capabilities (
    token_hash TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    scope_revision INTEGER NOT NULL,
    run_id TEXT NOT NULL UNIQUE REFERENCES agent_runs(id) ON DELETE CASCADE,
    operations_json TEXT NOT NULL,
    expires_at_unix INTEGER NOT NULL,
    revoked_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE agent_idempotency (
    run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    response_json TEXT NOT NULL,
    PRIMARY KEY(run_id, key)
);
CREATE TABLE agent_denials (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT,
    operation TEXT NOT NULL,
    reason TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_agent_capabilities_scope ON agent_capabilities(project_id, task_id, run_id);
