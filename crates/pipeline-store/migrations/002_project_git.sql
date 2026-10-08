ALTER TABLE projects ADD COLUMN git_root TEXT;
CREATE INDEX idx_projects_git_root ON projects(git_root);
