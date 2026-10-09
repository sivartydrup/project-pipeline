CREATE TABLE managed_checkouts (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    path TEXT NOT NULL UNIQUE,
    base_commit TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('intent','prepared','active','retained','failed','discarded')),
    run_id TEXT REFERENCES agent_runs(id),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_managed_checkouts_project_state ON managed_checkouts(project_id,state);

CREATE TABLE run_checkpoints (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    event_cursor INTEGER NOT NULL,
    git_head TEXT,
    diff_summary TEXT NOT NULL,
    task_revision INTEGER NOT NULL,
    run_state TEXT NOT NULL,
    recorded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_run_checkpoints_run ON run_checkpoints(run_id,recorded_at);

CREATE TABLE run_limits (
    run_id TEXT PRIMARY KEY REFERENCES agent_runs(id) ON DELETE CASCADE,
    wall_seconds INTEGER NOT NULL CHECK(wall_seconds > 0),
    token_budget INTEGER NOT NULL CHECK(token_budget > 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE policy_requests (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    action_class TEXT NOT NULL,
    target TEXT NOT NULL,
    command_digest TEXT NOT NULL,
    effect_summary TEXT NOT NULL,
    scope_revision INTEGER NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending','approved','denied','consumed')),
    revision INTEGER NOT NULL DEFAULT 1,
    expires_at_unix INTEGER,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(run_id,action_class,target,command_digest)
);
CREATE INDEX idx_policy_requests_project_status ON policy_requests(project_id,status);

UPDATE agent_runs SET state='queued' WHERE state='ready';
UPDATE agent_runs SET state='cancelled' WHERE state='closed';
