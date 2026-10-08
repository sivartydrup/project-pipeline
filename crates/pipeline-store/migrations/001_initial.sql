CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    path TEXT NOT NULL UNIQUE,
    stage TEXT NOT NULL DEFAULT 'idea' CHECK(stage IN ('idea','discovery','planning','design','build','verify','release','operate','paused','archived')),
    health TEXT NOT NULL DEFAULT 'on_track' CHECK(health IN ('on_track','needs_input','blocked','at_risk','failed')),
    active_scope_revision INTEGER NOT NULL DEFAULT 0,
    revision INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE brief_revisions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL,
    status TEXT NOT NULL,
    content_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(project_id, revision)
);

CREATE TABLE research_findings (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    brief_revision_id TEXT REFERENCES brief_revisions(id),
    claim TEXT NOT NULL,
    source_uri TEXT,
    accessed_at TEXT,
    confidence TEXT NOT NULL,
    is_hypothesis INTEGER NOT NULL CHECK(is_hypothesis IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE milestones (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    outcome TEXT NOT NULL,
    position INTEGER NOT NULL,
    scope_revision INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE epics (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    milestone_id TEXT REFERENCES milestones(id),
    title TEXT NOT NULL,
    outcome TEXT NOT NULL,
    position INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    epic_id TEXT REFERENCES epics(id),
    parent_task_id TEXT REFERENCES tasks(id),
    title TEXT NOT NULL,
    outcome TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('draft','ready','running','review','accepted','blocked','cancelled')),
    weight INTEGER NOT NULL DEFAULT 1 CHECK(weight > 0),
    priority INTEGER NOT NULL DEFAULT 0,
    risk TEXT NOT NULL DEFAULT 'low',
    scope_revision INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE dependencies (
    from_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    to_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK(kind IN ('blocks','informs')),
    PRIMARY KEY(from_task_id, to_task_id, kind),
    CHECK(from_task_id <> to_task_id)
);

CREATE TABLE criteria (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    assertion TEXT NOT NULL,
    verifier TEXT NOT NULL,
    required_evidence_json TEXT NOT NULL DEFAULT '[]',
    accepted_at TEXT
);

CREATE TABLE decisions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id),
    question TEXT NOT NULL,
    alternatives_json TEXT NOT NULL,
    selected_option TEXT,
    rationale TEXT,
    impact TEXT NOT NULL,
    status TEXT NOT NULL,
    supersedes_id TEXT REFERENCES decisions(id),
    actor TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE approvals (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    subject_type TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    subject_revision INTEGER NOT NULL,
    subject_hash TEXT NOT NULL,
    action_class TEXT NOT NULL,
    target TEXT NOT NULL,
    approver TEXT NOT NULL,
    expires_at TEXT,
    revoked_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE agent_runs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL REFERENCES tasks(id),
    harness TEXT NOT NULL,
    external_session_id TEXT,
    checkout_path TEXT NOT NULL,
    state TEXT NOT NULL,
    started_at TEXT,
    ended_at TEXT,
    revision INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE artifacts (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id),
    run_id TEXT REFERENCES agent_runs(id),
    kind TEXT NOT NULL,
    uri TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    mime_type TEXT,
    git_commit TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE run_events (
    run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL,
    kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(run_id, sequence)
);

CREATE TABLE test_results (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    run_id TEXT REFERENCES agent_runs(id),
    command TEXT NOT NULL,
    exit_code INTEGER,
    environment_json TEXT NOT NULL,
    log_artifact_id TEXT REFERENCES artifacts(id),
    git_commit TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE release_candidates (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    version TEXT NOT NULL,
    git_commit TEXT NOT NULL,
    target TEXT NOT NULL,
    checklist_json TEXT NOT NULL,
    rollback_notes TEXT NOT NULL,
    status TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE activity_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    actor TEXT NOT NULL,
    operation TEXT NOT NULL,
    subject_type TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    subject_revision INTEGER NOT NULL,
    before_json TEXT,
    after_json TEXT,
    correlation_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_tasks_project_status ON tasks(project_id, status);
CREATE INDEX idx_decisions_project_status ON decisions(project_id, status);
CREATE INDEX idx_activity_project_id ON activity_events(project_id, id);
CREATE INDEX idx_runs_project_state ON agent_runs(project_id, state);

CREATE TRIGGER activity_events_no_update BEFORE UPDATE ON activity_events
BEGIN
    SELECT RAISE(ABORT, 'activity events are append-only');
END;

CREATE TRIGGER activity_events_no_delete BEFORE DELETE ON activity_events
BEGIN
    SELECT RAISE(ABORT, 'activity events are append-only');
END;
