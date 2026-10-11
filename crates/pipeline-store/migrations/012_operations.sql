CREATE TABLE lifecycle_entries (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id),
    artifact_id TEXT REFERENCES artifacts(id),
    kind TEXT NOT NULL CHECK(kind IN ('positioning','launch_copy','launch_asset','channel_plan','metric','feedback','bug','incident','maintenance')),
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    channel TEXT NOT NULL DEFAULT '',
    metric_value TEXT NOT NULL DEFAULT '',
    metric_unit TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL CHECK(status IN ('draft','open','in_progress','blocked','resolved','published')),
    severity TEXT NOT NULL CHECK(severity IN ('low','medium','high','critical')),
    publication_target TEXT NOT NULL DEFAULT '',
    publication_approval_id TEXT REFERENCES approvals(id),
    revision INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_lifecycle_project_kind ON lifecycle_entries(project_id,kind,status);
