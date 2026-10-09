ALTER TABLE decisions ADD COLUMN evidence_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE decisions ADD COLUMN recommendation TEXT NOT NULL DEFAULT '';
ALTER TABLE decisions ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';
UPDATE decisions SET updated_at=created_at;

CREATE INDEX idx_decisions_task ON decisions(task_id, created_at);
CREATE INDEX idx_approvals_subject ON approvals(project_id, subject_type, subject_id, subject_revision);
