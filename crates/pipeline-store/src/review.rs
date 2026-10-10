use super::discovery::insert_event;
use super::{Result, Store, StoreError};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ArtifactEvidence {
    pub id: String,
    pub kind: String,
    pub uri: String,
    pub sha256: String,
}

#[derive(Debug, Clone)]
pub struct TestEvidence {
    pub id: String,
    pub command: String,
    pub exit_code: Option<i64>,
    pub log_artifact_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RunEventEvidence {
    pub sequence: i64,
    pub kind: String,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct TaskRunReview {
    pub run_id: String,
    pub task_id: String,
    pub state: String,
    pub checkout_path: String,
    pub packet: Option<Value>,
    pub packet_sha256: Option<String>,
    pub packet_verified: bool,
    pub submission: Option<Value>,
    pub artifacts: Vec<ArtifactEvidence>,
    pub tests: Vec<TestEvidence>,
    pub events: Vec<RunEventEvidence>,
}

impl Store {
    pub fn approve_agent_prompt(
        &mut self,
        run_id: &str,
        provider: &str,
        model: &str,
        actor: &str,
    ) -> Result<String> {
        if actor != "owner" {
            return Err(StoreError::PlanRequiresOwner);
        }
        if provider.trim().is_empty() || model.trim().is_empty() {
            return Err(StoreError::EmptyField("model"));
        }
        let tx = self.connection.transaction()?;
        let (project_id, state, revision, packet_hash): (String, String, i64, String) = tx
            .query_row(
                "SELECT r.project_id,r.state,r.revision,p.sha256 FROM agent_runs r
             JOIN run_packets p ON p.run_id=r.id WHERE r.id=?1",
                [run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?
            .ok_or_else(|| StoreError::NotFound(run_id.into()))?;
        if state != "starting" {
            return Err(StoreError::RunTransition(
                "prompt approval requires starting run".into(),
            ));
        }
        let prior: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM approvals WHERE subject_type='agent_run'
            AND subject_id=?1 AND action_class='external_model_prompt')",
            [run_id],
            |row| row.get(0),
        )?;
        if prior {
            return Err(StoreError::PolicyDenied(
                "run already has a model prompt approval".into(),
            ));
        }
        let target = format!("{provider}/{model}");
        let subject_hash = format!(
            "{:x}",
            Sha256::digest(format!("{packet_hash}:{target}").as_bytes())
        );
        let id = Uuid::new_v4().to_string();
        tx.execute("INSERT INTO approvals(id,project_id,subject_type,subject_id,subject_revision,
            subject_hash,action_class,target,approver,expires_at)
            VALUES (?1,?2,'agent_run',?3,?4,?5,'external_model_prompt',?6,'owner',datetime('now','+15 minutes'))",
            params![id,project_id,run_id,revision,subject_hash,target])?;
        insert_event(
            &tx,
            &project_id,
            actor,
            "run.prompt_approve",
            "run",
            run_id,
            revision,
            None,
            Some(json!({"provider":provider,"model":model,"approval_id":id})),
            run_id,
        )?;
        tx.commit()?;
        Ok(id)
    }

    pub fn consume_agent_prompt_approval(
        &mut self,
        run_id: &str,
        provider: &str,
        model: &str,
    ) -> Result<()> {
        let tx = self.connection.transaction()?;
        let (project_id, packet_hash, revision): (String, String, i64) = tx.query_row(
            "SELECT r.project_id,p.sha256,r.revision FROM agent_runs r JOIN run_packets p ON p.run_id=r.id
             WHERE r.id=?1 AND r.state='running'", [run_id],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        ).optional()?.ok_or_else(|| StoreError::RunTransition("running run packet required".into()))?;
        let target = format!("{provider}/{model}");
        let subject_hash = format!(
            "{:x}",
            Sha256::digest(format!("{packet_hash}:{target}").as_bytes())
        );
        let approval: Option<String> = tx.query_row(
            "SELECT id FROM approvals WHERE project_id=?1 AND subject_type='agent_run' AND subject_id=?2
             AND subject_hash=?3 AND action_class='external_model_prompt' AND target=?4
             AND approver='owner' AND revoked_at IS NULL AND expires_at>CURRENT_TIMESTAMP
             ORDER BY created_at DESC LIMIT 1",
            params![project_id,run_id,subject_hash,target], |row| row.get(0),
        ).optional()?;
        let id = approval.ok_or_else(|| {
            StoreError::PolicyDenied("exact model prompt approval missing or consumed".into())
        })?;
        tx.execute(
            "UPDATE approvals SET revoked_at=CURRENT_TIMESTAMP WHERE id=?1 AND revoked_at IS NULL",
            [&id],
        )?;
        insert_event(
            &tx,
            &project_id,
            "scheduler",
            "run.prompt_consume",
            "run",
            run_id,
            revision,
            None,
            Some(json!({"approval_id":id,"provider":provider,"model":model})),
            run_id,
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn latest_task_feedback(
        &self,
        project_id: &str,
        logical_id: &str,
    ) -> Result<Option<String>> {
        let value: Option<String> = self
            .connection
            .query_row(
                "SELECT a.after_json FROM activity_events a JOIN tasks t ON t.id=a.subject_id
             JOIN projects p ON p.id=t.project_id WHERE a.project_id=?1 AND t.logical_id=?2
             AND t.scope_revision=p.active_scope_revision AND a.operation='task.request_changes'
             ORDER BY a.id DESC LIMIT 1",
                params![project_id, logical_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|item| {
                item.get("reason")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            }))
    }

    pub fn save_run_packet(&mut self, run_id: &str, packet: &Value) -> Result<()> {
        let json = serde_json::to_string(packet)?;
        let digest = format!("{:x}", Sha256::digest(json.as_bytes()));
        let tx = self.connection.transaction()?;
        let project_id: String = tx
            .query_row(
                "SELECT project_id FROM agent_runs WHERE id=?1 AND state='starting'",
                [run_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| StoreError::RunTransition("run packet requires starting run".into()))?;
        tx.execute(
            "INSERT INTO run_packets(run_id,packet_json,sha256) VALUES (?1,?2,?3)",
            params![run_id, json, digest],
        )?;
        insert_event(
            &tx,
            &project_id,
            "scheduler",
            "run.packet",
            "run",
            run_id,
            1,
            None,
            Some(json!({"sha256":digest})),
            run_id,
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn attach_run_diff(&mut self, run_id: &str, path: &Path) -> Result<String> {
        let bytes =
            fs::read(path).map_err(|_| StoreError::MissingRunEvidence("diff unreadable".into()))?;
        if bytes.is_empty() {
            return Err(StoreError::MissingRunEvidence("diff is empty".into()));
        }
        let digest = format!("{:x}", Sha256::digest(&bytes));
        let tx = self.connection.transaction()?;
        let (project_id, task_id): (String, String) = tx.query_row(
            "SELECT project_id,task_id FROM agent_runs WHERE id=?1 AND state IN ('running','waiting_for_input','review')",
            [run_id], |row| Ok((row.get(0)?,row.get(1)?)),
        ).optional()?.ok_or_else(|| StoreError::RunTransition("run cannot attach diff".into()))?;
        let id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO artifacts(id,project_id,task_id,run_id,kind,uri,sha256,mime_type)
            VALUES (?1,?2,?3,?4,'diff',?5,?6,'text/x-diff')",
            params![
                id,
                project_id,
                task_id,
                run_id,
                path.to_string_lossy(),
                digest
            ],
        )?;
        insert_event(
            &tx,
            &project_id,
            "scheduler",
            "artifact.attach_diff",
            "artifact",
            &id,
            1,
            None,
            Some(json!({"run_id":run_id,"sha256":digest})),
            run_id,
        )?;
        tx.commit()?;
        Ok(id)
    }

    pub fn list_task_run_reviews(&self, project_id: &str) -> Result<Vec<TaskRunReview>> {
        let mut runs = self.connection.prepare(
            "SELECT r.id,t.logical_id,r.state,r.checkout_path FROM agent_runs r
             JOIN tasks t ON t.id=r.task_id WHERE r.project_id=?1
             ORDER BY r.rowid DESC LIMIT 40",
        )?;
        let rows = runs.query_map([project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        let mut reviews = Vec::new();
        for row in rows {
            let (run_id, task_id, state, checkout_path) = row?;
            let submission: Option<String> = self.connection.query_row(
                "SELECT after_json FROM activity_events WHERE project_id=?1 AND operation='task.submit'
                 AND actor=?2 AND subject_type='task' ORDER BY id DESC LIMIT 1",
                params![project_id, format!("agent:{run_id}")], |row| row.get(0),
            ).optional()?;
            let packet: Option<(String, String)> = self
                .connection
                .query_row(
                    "SELECT packet_json,sha256 FROM run_packets WHERE run_id=?1",
                    [&run_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let packet_verified = packet.as_ref().is_some_and(|(value, hash)| {
                format!("{:x}", Sha256::digest(value.as_bytes())) == *hash
            });
            let packet_sha256 = packet.as_ref().map(|(_, hash)| hash.clone());
            let mut artifacts = self.connection.prepare(
                "SELECT id,kind,uri,sha256 FROM artifacts WHERE run_id=?1 ORDER BY created_at,id",
            )?;
            let artifacts = artifacts
                .query_map([&run_id], |row| {
                    Ok(ArtifactEvidence {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        uri: row.get(2)?,
                        sha256: row.get(3)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let mut tests = self.connection.prepare(
                "SELECT id,command,exit_code,log_artifact_id FROM test_results WHERE run_id=?1 ORDER BY created_at,id",
            )?;
            let tests = tests
                .query_map([&run_id], |row| {
                    Ok(TestEvidence {
                        id: row.get(0)?,
                        command: row.get(1)?,
                        exit_code: row.get(2)?,
                        log_artifact_id: row.get(3)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let mut events = self.connection.prepare(
                "SELECT sequence,kind,payload_json FROM run_events WHERE run_id=?1 ORDER BY sequence DESC LIMIT 200",
            )?;
            let mut events = events
                .query_map([&run_id], |row| {
                    let payload: String = row.get(2)?;
                    let summary = serde_json::from_str::<Value>(&payload)
                        .ok()
                        .and_then(|v| v.get("summary").and_then(Value::as_str).map(str::to_owned))
                        .unwrap_or_default();
                    Ok(RunEventEvidence {
                        sequence: row.get(0)?,
                        kind: row.get(1)?,
                        summary,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            events.reverse();
            reviews.push(TaskRunReview {
                run_id,
                task_id,
                state,
                checkout_path,
                packet: packet
                    .map(|(value, _)| serde_json::from_str(&value))
                    .transpose()?,
                packet_sha256,
                packet_verified,
                submission: submission
                    .map(|value| serde_json::from_str(&value))
                    .transpose()?,
                artifacts,
                tests,
                events,
            });
        }
        Ok(reviews)
    }
}

pub(crate) fn require_agent_review_evidence(connection: &Connection, task_id: &str) -> Result<()> {
    let run_id: Option<String> = connection
        .query_row(
            "SELECT id FROM agent_runs WHERE task_id=?1 ORDER BY rowid DESC LIMIT 1",
            [task_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(run_id) = run_id else {
        return Ok(());
    };
    let (run_state, project_id): (String, String) = connection.query_row(
        "SELECT state,project_id FROM agent_runs WHERE id=?1",
        [&run_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if !matches!(run_state.as_str(), "review" | "completed") {
        return Err(StoreError::MissingRunEvidence(
            "run is not submitted for review".into(),
        ));
    }
    let packet: Option<(String, String)> = connection
        .query_row(
            "SELECT packet_json,sha256 FROM run_packets WHERE run_id=?1",
            [&run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (packet_json, packet_hash) = packet
        .ok_or_else(|| StoreError::MissingRunEvidence("approved run packet missing".into()))?;
    if format!("{:x}", Sha256::digest(packet_json.as_bytes())) != packet_hash {
        return Err(StoreError::MissingRunEvidence(
            "run packet hash changed".into(),
        ));
    }
    let packet: Value = serde_json::from_str(&packet_json)?;
    let (logical_id, scope_revision): (String, i64) = connection.query_row(
        "SELECT t.logical_id,p.active_scope_revision FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
        [task_id], |row| Ok((row.get(0)?,row.get(1)?)),
    )?;
    if packet["project_id"] != project_id
        || packet["task_id"] != logical_id
        || packet["scope_revision"].as_i64() != Some(scope_revision)
    {
        return Err(StoreError::MissingRunEvidence(
            "run packet is outside active scope".into(),
        ));
    }
    let submission: Option<String> = connection
        .query_row(
            "SELECT after_json FROM activity_events WHERE project_id=?1 AND operation='task.submit'
             AND actor=?2 AND subject_id=?3 ORDER BY id DESC LIMIT 1",
            params![project_id, format!("agent:{run_id}"), task_id],
            |row| row.get(0),
        )
        .optional()?;
    let submission: Value = submission
        .and_then(|s| serde_json::from_str(&s).ok())
        .ok_or_else(|| StoreError::MissingRunEvidence("agent submission missing".into()))?;
    let changed = submission
        .pointer("/submission/changed_files")
        .and_then(Value::as_array)
        .ok_or_else(|| StoreError::MissingRunEvidence("changed files missing".into()))?;
    if changed.is_empty() {
        return Err(StoreError::MissingRunEvidence(
            "changed files missing".into(),
        ));
    }
    let submitted_tests = submission
        .pointer("/submission/test_results")
        .and_then(Value::as_array)
        .ok_or_else(|| StoreError::MissingRunEvidence("test results missing".into()))?;
    if submitted_tests.is_empty() {
        return Err(StoreError::MissingRunEvidence(
            "test results missing".into(),
        ));
    }
    let mut test_count = 0;
    for test_id in submitted_tests {
        let id = test_id
            .as_str()
            .ok_or_else(|| StoreError::MissingRunEvidence("invalid test reference".into()))?;
        let row: Option<(Option<i64>, Option<String>)> = connection.query_row(
                "SELECT exit_code,log_artifact_id FROM test_results WHERE id=?1 AND task_id=?2 AND run_id=?3",
                params![id, task_id, run_id], |row| Ok((row.get(0)?, row.get(1)?)),
            ).optional()?;
        let Some((Some(0), Some(log_id))) = row else {
            return Err(StoreError::MissingRunEvidence(
                "passing linked test and log required".into(),
            ));
        };
        require_artifact_hash(connection, &log_id, &run_id)?;
        test_count += 1;
    }
    if test_count == 0 {
        return Err(StoreError::MissingRunEvidence(
            "passing test required".into(),
        ));
    }
    let diff_id: Option<String> = connection.query_row(
            "SELECT id FROM artifacts WHERE task_id=?1 AND run_id=?2 AND kind='diff' ORDER BY rowid DESC LIMIT 1",
            params![task_id, run_id], |row| row.get(0),
        ).optional()?;
    let diff_id =
        diff_id.ok_or_else(|| StoreError::MissingRunEvidence("diff artifact missing".into()))?;
    require_artifact_hash(connection, &diff_id, &run_id)?;
    let mut criteria = connection
        .prepare("SELECT required_evidence_json,evidence_json FROM criteria WHERE task_id=?1")?;
    let rows = criteria.query_map([task_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (required, evidence) = row?;
        let required: Vec<String> = serde_json::from_str(&required)?;
        let evidence: Value = serde_json::from_str(&evidence)?;
        for kind in required {
            let reference = evidence
                .get(&kind)
                .and_then(Value::as_str)
                .unwrap_or_default();
            let artifact: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM artifacts WHERE id=?1 AND task_id=?2 AND run_id=?3)",
                params![reference, task_id, run_id],
                |row| row.get(0),
            )?;
            let test: bool = connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM test_results WHERE id=?1 AND task_id=?2 AND run_id=?3 AND exit_code=0)",
                    params![reference,task_id,run_id], |row| row.get(0),
                )?;
            if !artifact && !test {
                return Err(StoreError::MissingRunEvidence(format!(
                    "criterion {kind} must reference an artifact or passing test from this run"
                )));
            }
            if artifact {
                require_artifact_hash(connection, reference, &run_id)?;
            }
        }
    }
    Ok(())
}

fn require_artifact_hash(connection: &Connection, artifact_id: &str, run_id: &str) -> Result<()> {
    let artifact: Option<(String, String)> = connection
        .query_row(
            "SELECT uri,sha256 FROM artifacts WHERE id=?1 AND run_id=?2",
            params![artifact_id, run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((uri, digest)) = artifact else {
        return Err(StoreError::MissingRunEvidence(
            "linked artifact missing".into(),
        ));
    };
    let bytes =
        fs::read(&uri).map_err(|_| StoreError::MissingRunEvidence("artifact unreadable".into()))?;
    if format!("{:x}", Sha256::digest(bytes)) != digest {
        return Err(StoreError::MissingRunEvidence(
            "artifact hash changed".into(),
        ));
    }
    Ok(())
}

impl Store {
    pub fn request_task_changes(
        &mut self,
        project_id: &str,
        logical_id: &str,
        expected_revision: i64,
        reason: &str,
        actor: &str,
        correlation_id: &str,
    ) -> Result<()> {
        if actor != "owner" {
            return Err(StoreError::PlanRequiresOwner);
        }
        if reason.trim().is_empty() {
            return Err(StoreError::EmptyField("reason"));
        }
        let tx = self.connection.transaction()?;
        let row: Option<(String, String, i64)> = tx.query_row(
            "SELECT t.id,t.status,t.revision FROM tasks t JOIN projects p ON p.id=t.project_id
             WHERE t.project_id=?1 AND t.logical_id=?2 AND t.scope_revision=p.active_scope_revision",
            params![project_id, logical_id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        ).optional()?;
        let (task_id, status, revision) =
            row.ok_or_else(|| StoreError::TaskNotInActiveScope(logical_id.into()))?;
        if revision != expected_revision {
            return Err(StoreError::RevisionConflict {
                id: task_id,
                expected: expected_revision,
                actual: revision,
            });
        }
        if status != "review" {
            return Err(StoreError::InvalidTaskTransition {
                from: status,
                to: "ready",
            });
        }
        let active: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_runs WHERE task_id=?1 AND state IN ('queued','starting','running','waiting_for_input','review'))",
            [&task_id], |row| row.get(0),
        )?;
        if active {
            return Err(StoreError::RunTransition(
                "finish or stop the current run before requesting changes".into(),
            ));
        }
        tx.execute("UPDATE tasks SET status='ready',revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?1", [&task_id])?;
        tx.execute("UPDATE criteria SET accepted_at=NULL,verified_by=NULL,evidence_json='{}' WHERE task_id=?1", [&task_id])?;
        insert_event(
            &tx,
            project_id,
            actor,
            "task.request_changes",
            "task",
            &task_id,
            revision + 1,
            Some(json!({"status":"review"})),
            Some(json!({"status":"ready","reason":reason.trim()})),
            correlation_id,
        )?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn agent_task_acceptance_requires_linked_untampered_diff_and_test_log() {
        let folder = tempdir().unwrap();
        let diff = folder.path().join("diff.patch");
        let log = folder.path().join("test.log");
        fs::write(&diff, "diff --git a/a b/a\n+added\n").unwrap();
        fs::write(&log, "test passed\n").unwrap();
        let mut store = Store::open_in_memory().unwrap();
        store
            .create_project(
                "p",
                "Project",
                folder.path().to_str().unwrap(),
                "owner",
                "create",
            )
            .unwrap();
        store
            .connection
            .execute(
                "UPDATE projects SET active_scope_revision=1 WHERE id='p'",
                [],
            )
            .unwrap();
        store.connection.execute("INSERT INTO tasks(id,project_id,title,outcome,status,scope_revision,logical_id,revision)
            VALUES ('t','p','Task','Done','review',1,'T',2)", []).unwrap();
        store.connection.execute("INSERT INTO criteria(id,task_id,assertion,verifier,required_evidence_json,evidence_json,verified_by,accepted_at,logical_id)
            VALUES ('c','t','Works','owner','[\"test-log\"]','{\"test-log\":\"log\"}','owner',CURRENT_TIMESTAMP,'C')", []).unwrap();
        store
            .connection
            .execute(
                "INSERT INTO agent_runs(id,project_id,task_id,harness,checkout_path,state)
            VALUES ('r','p','t','opencode',?1,'completed')",
                [folder.path().to_str().unwrap()],
            )
            .unwrap();
        let packet = json!({"project_id":"p","task_id":"T","scope_revision":1}).to_string();
        store
            .connection
            .execute(
                "INSERT INTO run_packets(run_id,packet_json,sha256) VALUES ('r',?1,?2)",
                params![packet, format!("{:x}", Sha256::digest(packet.as_bytes()))],
            )
            .unwrap();
        store
            .connection
            .execute("UPDATE run_packets SET sha256='bad' WHERE run_id='r'", [])
            .unwrap();
        assert!(matches!(
            store.accept_task("p", "T", 2, "owner", "accept"),
            Err(StoreError::MissingRunEvidence(_))
        ));
        store
            .connection
            .execute(
                "UPDATE run_packets SET sha256=?1 WHERE run_id='r'",
                [format!("{:x}", Sha256::digest(packet.as_bytes()))],
            )
            .unwrap();
        assert!(matches!(
            store.accept_task("p", "T", 2, "owner", "accept"),
            Err(StoreError::MissingRunEvidence(_))
        ));

        let diff_hash = format!("{:x}", Sha256::digest(fs::read(&diff).unwrap()));
        let log_hash = format!("{:x}", Sha256::digest(fs::read(&log).unwrap()));
        store
            .connection
            .execute(
                "INSERT INTO artifacts(id,project_id,task_id,run_id,kind,uri,sha256)
            VALUES ('diff','p','t','r','diff',?1,?2)",
                params![diff.to_str(), diff_hash],
            )
            .unwrap();
        store
            .connection
            .execute(
                "INSERT INTO artifacts(id,project_id,task_id,run_id,kind,uri,sha256)
            VALUES ('log','p','t','r','test-log',?1,?2)",
                params![log.to_str(), log_hash],
            )
            .unwrap();
        store.connection.execute("INSERT INTO test_results(id,task_id,run_id,command,exit_code,environment_json,log_artifact_id)
            VALUES ('test','t','r','cargo test',0,'{}','log')", []).unwrap();
        store.connection.execute("INSERT INTO activity_events(project_id,actor,operation,subject_type,subject_id,subject_revision,after_json,correlation_id)
            VALUES ('p','agent:r','task.submit','task','t',2,?1,'submit')",
            [json!({"submission":{"changed_files":["a"],"test_results":["test"]}}).to_string()]).unwrap();
        fs::write(&log, "changed").unwrap();
        assert!(matches!(
            store.accept_task("p", "T", 2, "owner", "accept"),
            Err(StoreError::MissingRunEvidence(_))
        ));
        fs::write(&log, "test passed\n").unwrap();
        store.accept_task("p", "T", 2, "owner", "accept").unwrap();
    }

    #[test]
    fn request_changes_requires_finished_run_and_resets_verification() {
        let folder = tempdir().unwrap();
        let mut store = Store::open_in_memory().unwrap();
        store
            .create_project(
                "p",
                "Project",
                folder.path().to_str().unwrap(),
                "owner",
                "create",
            )
            .unwrap();
        store
            .connection
            .execute(
                "UPDATE projects SET active_scope_revision=1 WHERE id='p'",
                [],
            )
            .unwrap();
        store.connection.execute("INSERT INTO tasks(id,project_id,title,outcome,status,scope_revision,logical_id,revision)
            VALUES ('t','p','Task','Done','review',1,'T',2)", []).unwrap();
        store.connection.execute("INSERT INTO criteria(id,task_id,assertion,verifier,required_evidence_json,evidence_json,verified_by,accepted_at,logical_id)
            VALUES ('c','t','Works','owner','[\"test-log\"]','{\"test-log\":\"old\"}','owner',CURRENT_TIMESTAMP,'C')", []).unwrap();
        store
            .connection
            .execute(
                "INSERT INTO agent_runs(id,project_id,task_id,harness,checkout_path,state)
            VALUES ('r','p','t','opencode',?1,'review')",
                [folder.path().to_str().unwrap()],
            )
            .unwrap();
        assert!(matches!(
            store.request_task_changes("p", "T", 2, "fix it", "owner", "change"),
            Err(StoreError::RunTransition(_))
        ));
        store
            .connection
            .execute("UPDATE agent_runs SET state='completed' WHERE id='r'", [])
            .unwrap();
        assert!(matches!(
            store.request_task_changes("p", "T", 2, "", "owner", "change"),
            Err(StoreError::EmptyField("reason"))
        ));
        store
            .request_task_changes("p", "T", 2, "fix it", "owner", "change")
            .unwrap();
        let (status, revision): (String, i64) = store
            .connection
            .query_row(
                "SELECT status,revision FROM tasks WHERE id='t'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((status.as_str(), revision), ("ready", 3));
        let verified: Option<String> = store
            .connection
            .query_row("SELECT accepted_at FROM criteria WHERE id='c'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(verified.is_none());
        assert_eq!(
            store.latest_task_feedback("p", "T").unwrap().as_deref(),
            Some("fix it")
        );
    }

    #[test]
    fn model_prompt_approval_is_exact_and_one_use() {
        let folder = tempdir().unwrap();
        let mut store = Store::open_in_memory().unwrap();
        store
            .create_project(
                "p",
                "Project",
                folder.path().to_str().unwrap(),
                "owner",
                "create",
            )
            .unwrap();
        store
            .connection
            .execute(
                "UPDATE projects SET active_scope_revision=1 WHERE id='p'",
                [],
            )
            .unwrap();
        store
            .connection
            .execute(
                "INSERT INTO tasks(id,project_id,title,outcome,status,scope_revision,logical_id)
            VALUES ('t','p','Task','Done','ready',1,'T')",
                [],
            )
            .unwrap();
        store
            .connection
            .execute(
                "INSERT INTO agent_runs(id,project_id,task_id,harness,checkout_path,state)
            VALUES ('r','p','t','opencode',?1,'starting')",
                [folder.path().to_str().unwrap()],
            )
            .unwrap();
        store
            .save_run_packet("r", &json!({"scope_revision":1,"task_id":"T"}))
            .unwrap();
        assert!(matches!(
            store.approve_agent_prompt("r", "openrouter", "model", "agent"),
            Err(StoreError::PlanRequiresOwner)
        ));
        store
            .approve_agent_prompt("r", "openrouter", "model", "owner")
            .unwrap();
        assert!(matches!(
            store.approve_agent_prompt("r", "openrouter", "model", "owner"),
            Err(StoreError::PolicyDenied(_))
        ));
        store
            .connection
            .execute("UPDATE agent_runs SET state='running' WHERE id='r'", [])
            .unwrap();
        assert!(matches!(
            store.consume_agent_prompt_approval("r", "openrouter", "other"),
            Err(StoreError::PolicyDenied(_))
        ));
        store
            .consume_agent_prompt_approval("r", "openrouter", "model")
            .unwrap();
        assert!(matches!(
            store.consume_agent_prompt_approval("r", "openrouter", "model"),
            Err(StoreError::PolicyDenied(_))
        ));
    }
}
