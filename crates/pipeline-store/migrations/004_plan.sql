CREATE TABLE plan_revisions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('draft','approved')),
    content_json TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    scope_revision INTEGER,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(project_id, revision),
    UNIQUE(project_id, scope_revision)
);

ALTER TABLE tasks ADD COLUMN logical_id TEXT;
ALTER TABLE tasks ADD COLUMN plan_revision_id TEXT REFERENCES plan_revisions(id);
ALTER TABLE criteria ADD COLUMN logical_id TEXT;
ALTER TABLE criteria ADD COLUMN evidence_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE criteria ADD COLUMN verified_by TEXT;

UPDATE tasks SET logical_id=id WHERE logical_id IS NULL;
UPDATE criteria SET logical_id=id WHERE logical_id IS NULL;

CREATE UNIQUE INDEX idx_tasks_scope_logical ON tasks(project_id, scope_revision, logical_id);
CREATE INDEX idx_tasks_plan_revision ON tasks(plan_revision_id);
CREATE INDEX idx_plan_revisions_project ON plan_revisions(project_id, revision);
