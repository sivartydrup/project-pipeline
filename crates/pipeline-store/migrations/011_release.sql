ALTER TABLE release_candidates ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';
UPDATE release_candidates SET updated_at=created_at;

CREATE TABLE release_builds (
    id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL REFERENCES release_candidates(id) ON DELETE CASCADE,
    artifact_id TEXT NOT NULL REFERENCES artifacts(id),
    platform TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(candidate_id, artifact_id)
);

CREATE TABLE release_checks (
    id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL REFERENCES release_candidates(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    required INTEGER NOT NULL CHECK(required IN (0,1)),
    status TEXT NOT NULL CHECK(status IN ('pending','passed','failed')),
    evidence_artifact_id TEXT REFERENCES artifacts(id),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(candidate_id, name)
);

CREATE TABLE release_deployments (
    id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL REFERENCES release_candidates(id) ON DELETE CASCADE,
    execution_approval_id TEXT NOT NULL REFERENCES approvals(id),
    result TEXT NOT NULL CHECK(result IN ('succeeded','failed')),
    smoke_result TEXT NOT NULL CHECK(smoke_result IN ('pending','passed','failed')),
    notes TEXT NOT NULL,
    actor TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_release_project ON release_candidates(project_id, created_at);
