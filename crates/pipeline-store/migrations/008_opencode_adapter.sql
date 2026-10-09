CREATE TABLE adapter_event_receipts (
    run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    external_event_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    PRIMARY KEY(run_id, external_event_id),
    UNIQUE(run_id, sequence)
);
CREATE UNIQUE INDEX idx_agent_runs_external_session ON agent_runs(external_session_id)
    WHERE external_session_id IS NOT NULL;
