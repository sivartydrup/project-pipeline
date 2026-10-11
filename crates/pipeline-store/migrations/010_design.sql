CREATE TABLE design_artifacts (
    id TEXT PRIMARY KEY,
    root_id TEXT NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id),
    decision_id TEXT REFERENCES decisions(id),
    kind TEXT NOT NULL CHECK(kind IN ('flow','wireframe','screenshot','component','copy','asset','accessibility')),
    title TEXT NOT NULL CHECK(length(trim(title)) > 0),
    body TEXT NOT NULL DEFAULT '',
    uri TEXT,
    sha256 TEXT NOT NULL,
    mime_type TEXT,
    source_run_id TEXT REFERENCES agent_runs(id),
    git_commit TEXT,
    version INTEGER NOT NULL CHECK(version > 0),
    status TEXT NOT NULL CHECK(status IN ('in_review','approved','changes_requested')),
    review_note TEXT,
    reviewed_by TEXT,
    reviewed_at TEXT,
    actor TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(project_id,root_id,version)
);
CREATE INDEX idx_design_project_root ON design_artifacts(project_id,root_id,version DESC);
