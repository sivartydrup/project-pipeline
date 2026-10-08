ALTER TABLE projects ADD COLUMN approved_brief_revision INTEGER NOT NULL DEFAULT 0;
ALTER TABLE research_findings ADD COLUMN summary TEXT NOT NULL DEFAULT '';
ALTER TABLE research_findings ADD COLUMN relevance TEXT NOT NULL DEFAULT '';
CREATE INDEX idx_brief_revisions_project_revision ON brief_revisions(project_id, revision DESC);
CREATE INDEX idx_research_findings_project ON research_findings(project_id, created_at DESC);
