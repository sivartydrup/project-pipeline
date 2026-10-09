use super::discovery::insert_event;
use super::{Result, Store, StoreError};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MAX_TOKEN_SECONDS: i64 = 900;
const OPERATIONS: &[&str] = &[
    "project.get",
    "project.context",
    "task.list",
    "task.get",
    "task.create",
    "task.update",
    "task.block",
    "task.submit",
    "decision.propose",
    "decision.get",
    "question.ask",
    "artifact.attach",
    "research.record",
    "test.record",
    "run.progress",
    "release.propose",
];

#[derive(Clone, Serialize, Deserialize)]
pub struct AgentCommand {
    #[serde(default)]
    pub token: String,
    pub project_id: String,
    pub run_id: String,
    pub operation: String,
    #[serde(default)]
    pub idempotency_key: String,
    #[serde(default)]
    pub expected_revision: i64,
    #[serde(default)]
    pub target_id: Option<String>,
    #[serde(default)]
    pub payload: Value,
}

impl std::fmt::Debug for AgentCommand {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentCommand")
            .field("project_id", &self.project_id)
            .field("run_id", &self.run_id)
            .field("operation", &self.operation)
            .field("idempotency_key", &self.idempotency_key)
            .field("expected_revision", &self.expected_revision)
            .field("target_id", &self.target_id)
            .field("token", &"[REDACTED]")
            .field("payload", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentReply {
    pub result: Value,
    pub revision: i64,
    pub activity_event_id: Option<i64>,
    pub replayed: bool,
}

#[derive(Clone)]
pub struct AgentGrant {
    pub run_id: String,
    pub token: String,
    pub expires_at_unix: i64,
}

impl std::fmt::Debug for AgentGrant {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentGrant")
            .field("run_id", &self.run_id)
            .field("token", &"[REDACTED]")
            .field("expires_at_unix", &self.expires_at_unix)
            .finish()
    }
}

type GrantTuple = (String, String, i64, String, String, i64, Option<String>);

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn required<'a>(payload: &'a Value, key: &'static str) -> Result<&'a str> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .ok_or(StoreError::EmptyField(key))
}

fn contains_secret(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields.iter().any(|(key, value)| {
            let name = key.to_ascii_lowercase();
            [
                "token",
                "secret",
                "password",
                "api_key",
                "authorization",
                "credential",
            ]
            .iter()
            .any(|part| name.contains(part))
                || contains_secret(value)
        }),
        Value::Array(items) => items.iter().any(contains_secret),
        Value::String(text) => {
            let text = text.to_ascii_lowercase();
            text.contains("bearer ") || text.contains("sk-")
        }
        _ => false,
    }
}

fn redact_value(value: Value, token: &str) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| {
                    let name = key.to_ascii_lowercase();
                    let sensitive = [
                        "token",
                        "secret",
                        "password",
                        "api_key",
                        "authorization",
                        "credential",
                    ]
                    .iter()
                    .any(|part| name.contains(part));
                    (
                        key,
                        if sensitive {
                            json!("[REDACTED]")
                        } else {
                            redact_value(value, token)
                        },
                    )
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| redact_value(item, token))
                .collect(),
        ),
        Value::String(text)
            if text.to_ascii_lowercase().contains("bearer ")
                || text.to_ascii_lowercase().contains("sk-")
                || (!token.is_empty() && text.contains(token)) =>
        {
            json!("[REDACTED]")
        }
        other => other,
    }
}

fn scope_path(root: &Path, value: &str) -> Result<PathBuf> {
    let root =
        fs::canonicalize(root).map_err(|_| StoreError::AgentDenied("invalid checkout".into()))?;
    let path = fs::canonicalize(value)
        .map_err(|_| StoreError::AgentDenied("artifact path missing".into()))?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err(StoreError::AgentDenied("artifact outside checkout".into()));
    }
    Ok(path)
}

impl Store {
    /// Scheduler/owner-only capability issuance. The raw token is returned once and never stored.
    pub fn issue_agent_grant(
        &mut self,
        project_id: &str,
        logical_task_id: &str,
        checkout: &Path,
        operations: &[String],
        ttl_seconds: i64,
    ) -> Result<AgentGrant> {
        if !(1..=MAX_TOKEN_SECONDS).contains(&ttl_seconds)
            || operations.is_empty()
            || operations
                .iter()
                .any(|op| !OPERATIONS.contains(&op.as_str()))
        {
            return Err(StoreError::AgentDenied(
                "invalid grant scope or lifetime".into(),
            ));
        }
        let project = self
            .get_project(project_id)?
            .ok_or_else(|| StoreError::NotFound(project_id.into()))?;
        let project_path = dunce::canonicalize(&project.path)
            .map_err(|_| StoreError::AgentDenied("project path missing".into()))?;
        let checkout_path = dunce::canonicalize(checkout)
            .map_err(|_| StoreError::AgentDenied("checkout path missing".into()))?;
        if !checkout_path.is_dir() {
            return Err(StoreError::AgentDenied(
                "checkout path must be a directory".into(),
            ));
        }
        let task = self
            .list_runnable_tasks(project_id)?
            .into_iter()
            .find(|task| task.logical_id == logical_task_id)
            .ok_or_else(|| StoreError::AgentDenied("task is not ready in approved scope".into()))?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let managed = checkout_path != project_path;
        if managed {
            let registered: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM managed_checkouts
                WHERE project_id=?1 AND task_id=?2 AND path=?3 AND state='prepared' AND run_id IS NULL)",
                params![project_id,task.id,checkout_path.to_string_lossy()],|row|row.get(0))?;
            if !registered {
                return Err(StoreError::AgentDenied(
                    "checkout is not a prepared managed worktree".into(),
                ));
            }
        }
        let still_ready: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
            WHERE t.id=?1 AND t.project_id=?2 AND t.scope_revision=p.active_scope_revision AND t.status='ready'
              AND NOT EXISTS(SELECT 1 FROM dependencies d JOIN tasks prerequisite ON prerequisite.id=d.to_task_id
                  WHERE d.from_task_id=t.id AND d.kind='blocks' AND prerequisite.status!='accepted'))",
            params![task.id,project_id],|row|row.get(0))?;
        if !still_ready {
            return Err(StoreError::AgentDenied("task changed before grant".into()));
        }
        let active: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_runs WHERE project_id=?1 AND state IN ('queued','starting','running','waiting_for_input','review'))",
            [project_id], |row| row.get(0))?;
        if active {
            return Err(StoreError::AgentDenied(
                "project already has an active run".into(),
            ));
        }
        if !managed {
            let pending: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM managed_checkouts WHERE project_id=?1 AND state IN ('intent','prepared','active'))",
                [project_id],|row|row.get(0))?;
            if pending {
                return Err(StoreError::AgentDenied(
                    "project has a pending managed checkout".into(),
                ));
            }
        }
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let run_id = Uuid::new_v4().to_string();
        let expiry = now() + ttl_seconds;
        tx.execute(
            "INSERT INTO agent_runs(id,project_id,task_id,harness,checkout_path,state,started_at)
                    VALUES (?1,?2,?3,'local-bridge',?4,'queued',CURRENT_TIMESTAMP)",
            params![run_id, project_id, task.id, checkout_path.to_string_lossy()],
        )?;
        tx.execute("INSERT INTO agent_capabilities(token_hash,project_id,task_id,scope_revision,run_id,operations_json,expires_at_unix)
                    VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![digest(&token),project_id,task.id,task.scope_revision,run_id,serde_json::to_string(operations)?,expiry])?;
        if managed {
            tx.execute("UPDATE managed_checkouts SET state='active',run_id=?1,updated_at=CURRENT_TIMESTAMP WHERE project_id=?2 AND task_id=?3 AND path=?4 AND state='prepared'",
                params![run_id,project_id,task.id,checkout_path.to_string_lossy()])?;
        }
        insert_event(
            &tx,
            project_id,
            "owner",
            "agent.grant",
            "run",
            &run_id,
            1,
            None,
            Some(json!({"task_id":task.id,"operations":operations,"expires_at_unix":expiry})),
            &run_id,
        )?;
        tx.commit()?;
        Ok(AgentGrant {
            run_id,
            token,
            expires_at_unix: expiry,
        })
    }

    pub fn revoke_agent_grant(&mut self, run_id: &str) -> Result<()> {
        let tx = self.connection.transaction()?;
        let project_id: String = tx
            .query_row(
                "SELECT project_id FROM agent_capabilities WHERE run_id=?1",
                [run_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| StoreError::NotFound(run_id.into()))?;
        tx.execute("UPDATE agent_capabilities SET revoked_at=CURRENT_TIMESTAMP WHERE run_id=?1 AND revoked_at IS NULL",[run_id])?;
        tx.execute("UPDATE agent_runs SET state='cancelled',ended_at=CURRENT_TIMESTAMP,revision=revision+1 WHERE id=?1 AND state NOT IN ('completed','failed','cancelled','interrupted')",[run_id])?;
        tx.execute("UPDATE managed_checkouts SET state='retained',updated_at=CURRENT_TIMESTAMP WHERE run_id=?1 AND state='active'",[run_id])?;
        let revision: i64 = tx.query_row(
            "SELECT revision FROM agent_runs WHERE id=?1",
            [run_id],
            |r| r.get(0),
        )?;
        insert_event(
            &tx,
            &project_id,
            "owner",
            "agent.revoke",
            "run",
            run_id,
            revision,
            None,
            Some(json!({"state":"cancelled"})),
            run_id,
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn agent_command(&mut self, command: &AgentCommand) -> Result<AgentReply> {
        let result = self.agent_command_inner(command);
        if let Err(error) = &result {
            let reason = match error {
                StoreError::AgentDenied(reason) => reason.as_str(),
                StoreError::RevisionConflict { .. } => "revision conflict",
                _ => "validation or persistence failure",
            };
            // Rejection telemetry is intentionally separate from project activity and contains no payload or token.
            let safe_run_id = Uuid::parse_str(&command.run_id)
                .ok()
                .map(|id| id.to_string());
            let safe_operation = if OPERATIONS.contains(&command.operation.as_str()) {
                command.operation.as_str()
            } else {
                "invalid"
            };
            let _ = self.connection.execute(
                "INSERT INTO agent_denials(run_id,operation,reason,correlation_id)
                VALUES (?1,?2,?3,?4)",
                params![
                    safe_run_id,
                    safe_operation,
                    reason,
                    digest(&command.idempotency_key)
                ],
            );
        }
        result
    }

    fn agent_command_inner(&mut self, command: &AgentCommand) -> Result<AgentReply> {
        if command.token.len() != 64 || !OPERATIONS.contains(&command.operation.as_str()) {
            return Err(StoreError::AgentDenied("invalid token or operation".into()));
        }
        let token_hash = digest(&command.token);
        let grant: Option<GrantTuple> = self
            .connection
            .query_row(
                "SELECT project_id,task_id,scope_revision,run_id,operations_json,expires_at_unix,revoked_at
             FROM agent_capabilities WHERE token_hash=?1",
                [&token_hash],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .optional()?;
        let (project_id, task_id, scope_revision, run_id, operations, expiry, revoked) =
            grant.ok_or_else(|| StoreError::AgentDenied("invalid token".into()))?;
        let current_scope: i64 = self.connection.query_row(
            "SELECT active_scope_revision FROM projects WHERE id=?1",
            [&project_id],
            |row| row.get(0),
        )?;
        let allowed: Vec<String> = serde_json::from_str(&operations)?;
        if revoked.is_some()
            || expiry <= now()
            || project_id != command.project_id
            || run_id != command.run_id
            || !allowed.contains(&command.operation)
            || current_scope != scope_revision
        {
            return Err(StoreError::AgentDenied(
                "expired, revoked, or out-of-scope capability".into(),
            ));
        }
        if command.operation.ends_with(".get")
            || command.operation == "project.context"
            || command.operation == "task.list"
        {
            return self.agent_read(command, &task_id);
        }
        if command.idempotency_key.trim().is_empty()
            || command.idempotency_key.len() > 128
            || !command
                .idempotency_key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(StoreError::AgentDenied("idempotency key required".into()));
        }
        if contains_secret(&command.payload) || command.payload.to_string().contains(&command.token)
        {
            return Err(StoreError::AgentDenied(
                "payload appears to contain a credential".into(),
            ));
        }
        let request_hash = digest(&serde_json::to_string(&json!({
            "project_id":command.project_id,"run_id":command.run_id,"operation":command.operation,
            "expected_revision":command.expected_revision,"target_id":command.target_id,"payload":command.payload
        }))?);
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let still_valid: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM agent_capabilities c JOIN projects p ON p.id=c.project_id
            WHERE c.token_hash=?1 AND c.project_id=?2 AND c.task_id=?3 AND c.scope_revision=p.active_scope_revision
              AND c.run_id=?4 AND c.revoked_at IS NULL AND c.expires_at_unix>?5 AND c.operations_json=?6)",
            params![token_hash,project_id,task_id,run_id,now(),operations],|row|row.get(0))?;
        if !still_valid {
            return Err(StoreError::AgentDenied(
                "grant changed before mutation".into(),
            ));
        }
        let prior: Option<(String,String)> = tx.query_row(
            "SELECT request_hash,response_json FROM agent_idempotency WHERE run_id=?1 AND key=?2",
            params![run_id,command.idempotency_key], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
        if let Some((hash, response)) = prior {
            if hash != request_hash {
                return Err(StoreError::AgentDenied(
                    "idempotency key reused for different request".into(),
                ));
            }
            let mut reply: AgentReply = serde_json::from_str(&response)?;
            reply.replayed = true;
            return Ok(reply);
        }
        let (result, revision, subject_type, subject_id) = mutate(&tx, command, &task_id)?;
        insert_event(
            &tx,
            &project_id,
            &format!("agent:{run_id}"),
            &command.operation,
            subject_type,
            &subject_id,
            revision,
            None,
            Some(result.clone()),
            &command.idempotency_key,
        )?;
        let activity_event_id = tx.last_insert_rowid();
        let reply = AgentReply {
            result,
            revision,
            activity_event_id: Some(activity_event_id),
            replayed: false,
        };
        tx.execute("INSERT INTO agent_idempotency(run_id,key,request_hash,response_json) VALUES (?1,?2,?3,?4)",
            params![run_id,command.idempotency_key,request_hash,serde_json::to_string(&reply)?])?;
        tx.commit()?;
        Ok(reply)
    }

    fn agent_read(&self, command: &AgentCommand, task_id: &str) -> Result<AgentReply> {
        let project = self
            .get_project(&command.project_id)?
            .ok_or_else(|| StoreError::NotFound(command.project_id.clone()))?;
        let result = match command.operation.as_str() {
            "project.get" => {
                json!({"id":project.id,"name":project.name,"path":project.path,"stage":project.stage.as_str(),"health":project.health.as_str(),"revision":project.revision,"active_scope_revision":project.active_scope_revision})
            }
            "project.context" => {
                json!({"project": {"id":project.id,"name":project.name,"path":project.path,"revision":project.revision},
                "briefs":self.list_brief_revisions(&command.project_id)?.into_iter().filter(|b| b.status=="approved").map(|b| json!({"revision":b.revision,"content":b.content})).collect::<Vec<_>>(),
                "decisions":self.list_decisions(&command.project_id)?.into_iter().filter(|d| d.status=="approved").map(|d| json!({"id":d.id,"question":d.input.question,"selected_option":d.selected_option,"revision":d.revision})).collect::<Vec<_>>() })
            }
            "task.list" => json!(
                self.list_active_tasks(&command.project_id)?
                    .into_iter()
                    .map(task_json)
                    .collect::<Vec<_>>()
            ),
            "task.get" => {
                let target = command.target_id.as_deref().unwrap_or(task_id);
                let task = self
                    .list_active_tasks(&command.project_id)?
                    .into_iter()
                    .find(|t| t.id == target || t.logical_id == target);
                if let Some(task) = task {
                    task_json(task)
                } else {
                    self.connection.query_row("SELECT id,title,outcome,revision FROM tasks
                        WHERE project_id=?1 AND parent_task_id=?2 AND status='draft' AND (id=?3 OR logical_id=?3)",
                        params![command.project_id,task_id,target],|row|Ok(json!({"id":row.get::<_,String>(0)?,
                        "title":row.get::<_,String>(1)?,"outcome":row.get::<_,String>(2)?,
                        "revision":row.get::<_,i64>(3)?,"status":"draft"}))).optional()?
                        .ok_or_else(||StoreError::NotFound(target.into()))?
                }
            }
            "decision.get" => {
                let id = command
                    .target_id
                    .as_deref()
                    .ok_or(StoreError::EmptyField("target_id"))?;
                let decision = self
                    .list_decisions(&command.project_id)?
                    .into_iter()
                    .find(|d| d.id == id)
                    .ok_or_else(|| StoreError::NotFound(id.into()))?;
                json!({"id":decision.id,"question":decision.input.question,"status":decision.status,"revision":decision.revision,
                    "alternatives":decision.input.alternatives,"recommendation":decision.input.recommendation,"impact":decision.input.impact})
            }
            _ => return Err(StoreError::AgentDenied("invalid read operation".into())),
        };
        Ok(AgentReply {
            result: redact_value(result, &command.token),
            revision: project.revision,
            activity_event_id: None,
            replayed: false,
        })
    }
}

fn task_json(task: super::TaskRecord) -> Value {
    json!({"id":task.id,"logical_id":task.logical_id,"title":task.title,"outcome":task.outcome,
        "status":task.status,"revision":task.revision,"scope_revision":task.scope_revision,
        "unresolved_blockers":task.unresolved_blockers,
        "criteria":task.criteria.into_iter().map(|c| json!({"id":c.logical_id,"assertion":c.assertion,"required_evidence":c.required_evidence})).collect::<Vec<_>>()})
}

fn checked_task(
    tx: &Connection,
    cmd: &AgentCommand,
    task_id: &str,
) -> Result<(String, i64, String)> {
    let target = cmd.target_id.as_deref().unwrap_or(task_id);
    let row: Option<(String,i64,String)> = tx.query_row(
        "SELECT t.id,t.revision,t.status FROM tasks t JOIN projects p ON p.id=t.project_id
         WHERE t.project_id=?1 AND t.scope_revision=p.active_scope_revision AND (t.id=?2 OR t.logical_id=?2)",
        params![cmd.project_id,target], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
    let (id, revision, status) = row.ok_or_else(|| StoreError::NotFound(target.into()))?;
    if id != task_id && !matches!(cmd.operation.as_str(), "task.update" | "task.get") {
        return Err(StoreError::AgentDenied(
            "target outside assigned task".into(),
        ));
    }
    if revision != cmd.expected_revision {
        return Err(StoreError::RevisionConflict {
            id,
            expected: cmd.expected_revision,
            actual: revision,
        });
    }
    Ok((id, revision, status))
}

fn mutate(
    tx: &Connection,
    cmd: &AgentCommand,
    task_id: &str,
) -> Result<(Value, i64, &'static str, String)> {
    let subject = match cmd.operation.as_str() {
        "task.block" | "task.submit" => {
            let (id, rev, status) = checked_task(tx, cmd, task_id)?;
            let next = if cmd.operation == "task.block" {
                "blocked"
            } else {
                "review"
            };
            if !matches!(status.as_str(), "ready" | "running" | "blocked")
                || (next == "review" && status == "blocked")
            {
                return Err(StoreError::AgentDenied("invalid task transition".into()));
            }
            if next == "blocked" {
                required(&cmd.payload, "reason")?;
            }
            if next == "review" {
                required(&cmd.payload, "summary")?;
                for field in [
                    "changed_files",
                    "test_results",
                    "residual_risks",
                    "criteria",
                ] {
                    if !cmd.payload.get(field).is_some_and(Value::is_array) {
                        return Err(StoreError::EmptyField(field));
                    }
                }
                let checkout: String = tx.query_row(
                    "SELECT checkout_path FROM agent_runs WHERE id=?1",
                    [&cmd.run_id],
                    |r| r.get(0),
                )?;
                for file in cmd.payload["changed_files"]
                    .as_array()
                    .expect("validated array")
                {
                    let path = file
                        .as_str()
                        .ok_or(StoreError::AgentDenied("changed file path required".into()))?;
                    scope_path(Path::new(&checkout), path)?;
                }
                for result in cmd.payload["test_results"]
                    .as_array()
                    .expect("validated array")
                {
                    let result_id = result
                        .as_str()
                        .ok_or(StoreError::AgentDenied("test result id required".into()))?;
                    let linked: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM test_results WHERE id=?1 AND task_id=?2 AND run_id=?3)",params![result_id,id,cmd.run_id],|r|r.get(0))?;
                    if !linked {
                        return Err(StoreError::AgentDenied("test result outside run".into()));
                    }
                }
            }
            tx.execute("UPDATE tasks SET status=?1,revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?2", params![next,id])?;
            (
                json!({"id":id,"status":next,"submission":cmd.payload}),
                rev + 1,
                "task",
                id,
            )
        }
        "task.create" => {
            let title = required(&cmd.payload, "title")?;
            let outcome = required(&cmd.payload, "outcome")?;
            let revision: i64 = tx.query_row(
                "SELECT revision FROM projects WHERE id=?1",
                [&cmd.project_id],
                |r| r.get(0),
            )?;
            if revision != cmd.expected_revision {
                return Err(StoreError::RevisionConflict {
                    id: cmd.project_id.clone(),
                    expected: cmd.expected_revision,
                    actual: revision,
                });
            }
            let id = Uuid::new_v4().to_string();
            tx.execute("INSERT INTO tasks(id,project_id,parent_task_id,title,outcome,status,scope_revision,logical_id)
                VALUES (?1,?2,?5,?3,?4,'draft',(SELECT active_scope_revision+1 FROM projects WHERE id=?2),?1)",params![id,cmd.project_id,title,outcome,task_id])?;
            tx.execute(
                "UPDATE projects SET revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?1",
                [&cmd.project_id],
            )?;
            (
                json!({"id":id,"status":"draft","project_revision":revision+1}),
                1,
                "task",
                id,
            )
        }
        "task.update" => {
            let target = cmd
                .target_id
                .as_deref()
                .ok_or(StoreError::EmptyField("target_id"))?;
            let row: Option<(String,i64,String)> = tx.query_row("SELECT id,revision,status FROM tasks WHERE project_id=?1 AND parent_task_id=?3 AND (id=?2 OR logical_id=?2)",params![cmd.project_id,target,task_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
            let (id, rev, status) = row.ok_or_else(|| StoreError::NotFound(target.into()))?;
            if status != "draft" {
                return Err(StoreError::AgentDenied(
                    "approved task scope is immutable".into(),
                ));
            }
            if rev != cmd.expected_revision {
                return Err(StoreError::RevisionConflict {
                    id,
                    expected: cmd.expected_revision,
                    actual: rev,
                });
            }
            let title = required(&cmd.payload, "title")?;
            let outcome = required(&cmd.payload, "outcome")?;
            tx.execute("UPDATE tasks SET title=?1,outcome=?2,revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?3",params![title,outcome,id])?;
            (
                json!({"id":id,"title":title,"outcome":outcome}),
                rev + 1,
                "task",
                id,
            )
        }
        "decision.propose" | "question.ask" => {
            let (id, _rev, status) = checked_task(tx, cmd, task_id)?;
            if cmd.operation == "question.ask"
                && !matches!(status.as_str(), "ready" | "running" | "blocked")
            {
                return Err(StoreError::AgentDenied(
                    "question requires active task".into(),
                ));
            }
            let question = required(&cmd.payload, "question")?;
            if cmd.operation == "question.ask" {
                let duplicate: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM decisions WHERE task_id=?1 AND question=?2 AND status='proposed')",params![id,question],|r|r.get(0))?;
                if duplicate {
                    return Err(StoreError::AgentDenied("question already pending".into()));
                }
            }
            let alternatives = cmd
                .payload
                .get("alternatives")
                .and_then(Value::as_array)
                .ok_or(StoreError::EmptyField("alternatives"))?;
            if alternatives.len() < 2
                || alternatives
                    .iter()
                    .any(|v| v.as_str().is_none_or(|s| s.trim().is_empty()))
            {
                return Err(StoreError::AgentDenied("two alternatives required".into()));
            }
            let recommendation = required(&cmd.payload, "recommendation")?;
            if !alternatives
                .iter()
                .any(|v| v.as_str() == Some(recommendation))
            {
                return Err(StoreError::AgentDenied(
                    "recommendation must be an alternative".into(),
                ));
            }
            let impact = if cmd.operation == "question.ask" {
                "blocking"
            } else {
                required(&cmd.payload, "impact")?
            };
            if !["low", "medium", "high", "blocking"].contains(&impact) {
                return Err(StoreError::AgentDenied("invalid impact".into()));
            }
            let decision_id = Uuid::new_v4().to_string();
            let evidence = cmd
                .payload
                .get("evidence")
                .filter(|v| v.is_array())
                .cloned()
                .unwrap_or_else(|| json!([]));
            tx.execute("INSERT INTO decisions(id,project_id,task_id,question,alternatives_json,recommendation,rationale,evidence_json,impact,status,actor,updated_at)
                VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'proposed',?10,CURRENT_TIMESTAMP)",
                params![decision_id,cmd.project_id,id,question,serde_json::to_string(alternatives)?,recommendation,
                    cmd.payload.get("rationale").and_then(Value::as_str).unwrap_or(""),evidence.to_string(),impact,format!("agent:{}",cmd.run_id)])?;
            if cmd.operation == "question.ask" {
                tx.execute(
                    "UPDATE tasks SET status='blocked',revision=revision+1 WHERE id=?1",
                    [&id],
                )?;
            }
            (
                json!({"id":decision_id,"status":"proposed","task_id":id,
                    "task_revision":if cmd.operation=="question.ask" {_rev+1} else {_rev},
                    "task_status":if cmd.operation=="question.ask" {"blocked"} else {status.as_str()}}),
                1,
                "decision",
                decision_id,
            )
        }
        "artifact.attach" => {
            let (id, _rev, _) = checked_task(tx, cmd, task_id)?;
            let uri = required(&cmd.payload, "path")?;
            let checkout: String = tx.query_row(
                "SELECT checkout_path FROM agent_runs WHERE id=?1",
                [&cmd.run_id],
                |r| r.get(0),
            )?;
            let path = scope_path(Path::new(&checkout), uri)?;
            let bytes = fs::read(&path)
                .map_err(|_| StoreError::AgentDenied("artifact unreadable".into()))?;
            let sha = format!("{:x}", Sha256::digest(&bytes));
            let supplied = required(&cmd.payload, "sha256")?;
            if sha != supplied {
                return Err(StoreError::AgentDenied("artifact hash mismatch".into()));
            }
            let artifact_id = Uuid::new_v4().to_string();
            let kind = required(&cmd.payload, "kind")?;
            tx.execute("INSERT INTO artifacts(id,project_id,task_id,run_id,kind,uri,sha256) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![artifact_id,cmd.project_id,id,cmd.run_id,kind,path.to_string_lossy(),sha])?;
            (
                json!({"id":artifact_id,"sha256":sha,"task_id":id}),
                1,
                "artifact",
                artifact_id,
            )
        }
        "research.record" => {
            let (id, _rev, _) = checked_task(tx, cmd, task_id)?;
            let claim = required(&cmd.payload, "claim")?;
            let source = required(&cmd.payload, "source_uri")?;
            let date = required(&cmd.payload, "accessed_at")?;
            let research_id = Uuid::new_v4().to_string();
            tx.execute("INSERT INTO research_findings(id,project_id,claim,source_uri,accessed_at,confidence,is_hypothesis,summary,relevance)
                VALUES (?1,?2,?3,?4,?5,?6,0,?7,?8)",params![research_id,cmd.project_id,claim,source,date,
                required(&cmd.payload,"confidence")?,cmd.payload.get("summary").and_then(Value::as_str).unwrap_or(""),format!("task:{id}")])?;
            (
                json!({"id":research_id,"task_id":id}),
                1,
                "research",
                research_id,
            )
        }
        "test.record" => {
            let (id, _rev, _) = checked_task(tx, cmd, task_id)?;
            let command = required(&cmd.payload, "command")?;
            let exit = cmd
                .payload
                .get("exit_code")
                .and_then(Value::as_i64)
                .ok_or(StoreError::EmptyField("exit_code"))?;
            let env = cmd
                .payload
                .get("environment")
                .filter(|v| v.is_object())
                .ok_or(StoreError::EmptyField("environment"))?;
            let log = required(&cmd.payload, "log_artifact_id")?;
            let linked: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM artifacts WHERE id=?1 AND task_id=?2 AND run_id=?3)",
                params![log, id, cmd.run_id],
                |r| r.get(0),
            )?;
            if !linked {
                return Err(StoreError::AgentDenied("log artifact outside run".into()));
            }
            let test_id = Uuid::new_v4().to_string();
            tx.execute("INSERT INTO test_results(id,task_id,run_id,command,exit_code,environment_json,log_artifact_id,git_commit)
                VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",params![test_id,id,cmd.run_id,command,exit,serde_json::to_string(env)?,log,
                cmd.payload.get("git_commit").and_then(Value::as_str)])?;
            (
                json!({"id":test_id,"exit_code":exit,"task_id":id}),
                1,
                "test",
                test_id,
            )
        }
        "run.progress" => {
            let (id, _rev, _) = checked_task(tx, cmd, task_id)?;
            let status = required(&cmd.payload, "status")?;
            let next = required(&cmd.payload, "next_action")?;
            let seq: i64 = tx.query_row(
                "SELECT COALESCE(MAX(sequence),0)+1 FROM run_events WHERE run_id=?1",
                [&cmd.run_id],
                |r| r.get(0),
            )?;
            tx.execute("INSERT INTO run_events(run_id,sequence,kind,payload_json) VALUES (?1,?2,'progress',?3)",
                params![cmd.run_id,seq,json!({"status":status,"next_action":next}).to_string()])?;
            (
                json!({"sequence":seq,"task_id":id}),
                seq,
                "run",
                cmd.run_id.clone(),
            )
        }
        "release.propose" => {
            let (id, _rev, _) = checked_task(tx, cmd, task_id)?;
            let version = required(&cmd.payload, "version")?;
            let commit = required(&cmd.payload, "git_commit")?;
            let target = required(&cmd.payload, "target")?;
            let rollback = required(&cmd.payload, "rollback_notes")?;
            let checklist = cmd
                .payload
                .get("checklist")
                .filter(|v| v.is_array())
                .ok_or(StoreError::EmptyField("checklist"))?;
            let release_id = Uuid::new_v4().to_string();
            tx.execute("INSERT INTO release_candidates(id,project_id,version,git_commit,target,checklist_json,rollback_notes,status)
                VALUES (?1,?2,?3,?4,?5,?6,?7,'proposed')",params![release_id,cmd.project_id,version,commit,target,checklist.to_string(),rollback])?;
            (
                json!({"id":release_id,"status":"proposed","task_id":id}),
                1,
                "release",
                release_id,
            )
        }
        _ => return Err(StoreError::AgentDenied("unsupported mutation".into())),
    };
    Ok(subject)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Store, tempfile::TempDir, AgentGrant) {
        let folder = tempfile::tempdir().unwrap();
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
            VALUES ('t','p','Task','Result','ready',1,'T')",
                [],
            )
            .unwrap();
        let grant = store
            .issue_agent_grant(
                "p",
                "T",
                folder.path(),
                &OPERATIONS.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                900,
            )
            .unwrap();
        (store, folder, grant)
    }

    fn command(grant: &AgentGrant, operation: &str, payload: Value) -> AgentCommand {
        AgentCommand {
            token: grant.token.clone(),
            project_id: "p".into(),
            run_id: grant.run_id.clone(),
            operation: operation.into(),
            idempotency_key: "key_1".into(),
            expected_revision: 1,
            target_id: None,
            payload,
        }
    }

    #[test]
    fn grant_rejects_invalid_scope_and_expiry_and_token_is_not_exported() {
        let (mut store, folder, grant) = setup();
        assert!(
            store
                .issue_agent_grant("p", "T", folder.path(), &["task.accept".into()], 900)
                .is_err()
        );
        assert!(
            store
                .issue_agent_grant("p", "T", folder.path(), &["task.submit".into()], 901)
                .is_err()
        );
        let export = store.export_json().unwrap();
        assert!(!export.contains(&grant.token));
        assert!(!export.contains(&digest(&grant.token)));
        store.revoke_agent_grant(&grant.run_id).unwrap();
        assert!(
            store
                .agent_command(&command(&grant, "task.get", json!({})))
                .is_err()
        );
    }

    #[test]
    fn replay_is_read_only_and_conflicting_replay_and_stale_revision_fail() {
        let (mut store, _folder, grant) = setup();
        let request = command(&grant, "task.block", json!({"reason":"Need input"}));
        let first = store.agent_command(&request).unwrap();
        assert_eq!(first.revision, 2);
        let count = store.activity_count("p").unwrap();
        let again = store.agent_command(&request).unwrap();
        assert!(again.replayed);
        assert_eq!(again.activity_event_id, first.activity_event_id);
        assert_eq!(store.activity_count("p").unwrap(), count);
        let mut changed = request.clone();
        changed.payload = json!({"reason":"Different"});
        assert!(store.agent_command(&changed).is_err());
        changed.idempotency_key = "key_2".into();
        assert!(matches!(
            store.agent_command(&changed),
            Err(StoreError::RevisionConflict { .. })
        ));
        assert_eq!(store.activity_count("p").unwrap(), count);
    }

    #[test]
    fn invalid_token_cross_project_and_cross_task_are_denied() {
        let (mut store, _folder, grant) = setup();
        let mut request = command(&grant, "task.submit", json!({}));
        request.token = "0".repeat(64);
        assert!(store.agent_command(&request).is_err());
        request.token = grant.token.clone();
        request.project_id = "other".into();
        assert!(store.agent_command(&request).is_err());
        request.project_id = "p".into();
        request.target_id = Some("other".into());
        store
            .connection
            .execute(
                "INSERT INTO tasks(id,project_id,title,outcome,status,scope_revision,logical_id)
            VALUES ('other','p','Other','Result','ready',1,'Other')",
                [],
            )
            .unwrap();
        assert!(store.agent_command(&request).is_err());
        assert_eq!(store.activity_count("p").unwrap(), 2); // create + grant only
    }

    #[test]
    fn artifact_path_and_hash_are_enforced_and_secrets_are_rejected() {
        let (mut store, folder, grant) = setup();
        let outside = tempfile::NamedTempFile::new().unwrap();
        let request = command(
            &grant,
            "artifact.attach",
            json!({"path":outside.path(),"kind":"log","sha256":"bad"}),
        );
        assert!(store.agent_command(&request).is_err());
        let path = folder.path().join("log.txt");
        fs::write(&path, b"log").unwrap();
        let mut request = command(
            &grant,
            "artifact.attach",
            json!({"path":path,"kind":"log","sha256":"bad"}),
        );
        assert!(store.agent_command(&request).is_err());
        request.payload["sha256"] = json!(format!("{:x}", Sha256::digest(b"log")));
        assert!(store.agent_command(&request).is_ok());
        let mut secret = command(
            &grant,
            "run.progress",
            json!({"status":"work","next_action":"go","api_key":"sk-private"}),
        );
        secret.idempotency_key = "secret_case".into();
        assert!(store.agent_command(&secret).is_err());
        let export = store.export_json().unwrap();
        assert!(!export.contains("sk-private"));
        assert!(!export.contains(&grant.token));
    }

    #[test]
    fn all_contract_operations_use_scoped_records() {
        let (mut store, folder, grant) = setup();
        for operation in ["project.get", "project.context", "task.list", "task.get"] {
            assert!(
                store
                    .agent_command(&command(&grant, operation, json!({})))
                    .is_ok(),
                "{operation}"
            );
        }
        let mut draft = command(
            &grant,
            "task.create",
            json!({"title":"Follow-up","outcome":"Done"}),
        );
        draft.idempotency_key = "create".into();
        let created = store.agent_command(&draft).unwrap();
        assert_eq!(created.revision, 1);
        assert_eq!(created.result["project_revision"], 2);
        let draft_id = created.result["id"].as_str().unwrap().to_owned();
        let mut draft_get = command(&grant, "task.get", json!({}));
        draft_get.target_id = Some(draft_id.clone());
        assert_eq!(
            store.agent_command(&draft_get).unwrap().result["status"],
            "draft"
        );
        let mut update = command(
            &grant,
            "task.update",
            json!({"title":"Edited","outcome":"Verified"}),
        );
        update.target_id = Some(draft_id);
        update.idempotency_key = "update".into();
        assert_eq!(store.agent_command(&update).unwrap().revision, 2);

        let mut decision = command(
            &grant,
            "decision.propose",
            json!({"question":"Which approach?","alternatives":["A","B"],"recommendation":"A","impact":"low","evidence":["note"]}),
        );
        decision.idempotency_key = "decision".into();
        let proposed = store.agent_command(&decision).unwrap();
        let mut get = command(&grant, "decision.get", json!({}));
        get.target_id = Some(proposed.result["id"].as_str().unwrap().to_owned());
        assert_eq!(
            store.agent_command(&get).unwrap().result["status"],
            "proposed"
        );

        let log = folder.path().join("test.log");
        fs::write(&log, b"passed").unwrap();
        let mut attach = command(
            &grant,
            "artifact.attach",
            json!({"path":log,"kind":"log","sha256":format!("{:x}",Sha256::digest(b"passed"))}),
        );
        attach.idempotency_key = "artifact".into();
        let artifact = store.agent_command(&attach).unwrap();
        let mut research = command(
            &grant,
            "research.record",
            json!({"claim":"Fact","source_uri":"https://example.org","accessed_at":"2026-10-09","confidence":"high"}),
        );
        research.idempotency_key = "research".into();
        assert!(store.agent_command(&research).is_ok());
        let mut test = command(
            &grant,
            "test.record",
            json!({"command":"cargo test","exit_code":0,"environment":{"os":"windows"},"log_artifact_id":artifact.result["id"]}),
        );
        test.idempotency_key = "test".into();
        assert!(store.agent_command(&test).is_ok());
        let mut progress = command(
            &grant,
            "run.progress",
            json!({"status":"working","next_action":"submit"}),
        );
        progress.idempotency_key = "progress".into();
        assert!(store.agent_command(&progress).is_ok());
        let mut release = command(
            &grant,
            "release.propose",
            json!({"version":"0.1","git_commit":"abc","target":"local","checklist":[],"rollback_notes":"restore"}),
        );
        release.idempotency_key = "release".into();
        assert!(store.agent_command(&release).is_ok());
        let mut submit = command(
            &grant,
            "task.submit",
            json!({"summary":"Ready","changed_files":[],"test_results":[],"residual_risks":[],"criteria":[]}),
        );
        submit.idempotency_key = "submit".into();
        assert_eq!(
            store.agent_command(&submit).unwrap().result["status"],
            "review"
        );
    }

    #[test]
    fn question_is_deduplicated_and_blocks_assigned_task() {
        let (mut store, _folder, grant) = setup();
        let payload =
            json!({"question":"Need owner choice?","alternatives":["A","B"],"recommendation":"A"});
        let mut ask = command(&grant, "question.ask", payload);
        ask.idempotency_key = "ask_1".into();
        assert!(store.agent_command(&ask).is_ok());
        ask.expected_revision = 2;
        ask.idempotency_key = "ask_2".into();
        assert!(store.agent_command(&ask).is_err());
        assert_eq!(store.list_active_tasks("p").unwrap()[0].status, "blocked");
    }

    #[test]
    fn scope_change_invalidates_run_and_denial_log_redacts_untrusted_fields() {
        let (mut store, folder, grant) = setup();
        assert!(
            store
                .issue_agent_grant("p", "T", folder.path(), &["task.get".into()], 30)
                .is_err()
        );
        let mut rogue = command(
            &grant,
            "run.progress",
            json!({"status":"work","next_action":"go"}),
        );
        rogue.idempotency_key = "sk-private".into();
        rogue.operation = "sk-private".into();
        rogue.run_id = "sk-private".into();
        assert!(store.agent_command(&rogue).is_err());
        assert!(!store.export_json().unwrap().contains("sk-private"));
        store
            .connection
            .execute(
                "UPDATE projects SET active_scope_revision=2 WHERE id='p'",
                [],
            )
            .unwrap();
        assert!(
            store
                .agent_command(&command(&grant, "task.get", json!({})))
                .is_err()
        );
    }

    #[test]
    fn expired_token_and_token_in_payload_are_denied() {
        let (mut store, folder, grant) = setup();
        let leak = command(
            &grant,
            "run.progress",
            json!({"status":"work","next_action":grant.token}),
        );
        assert!(!format!("{grant:?}").contains(&grant.token));
        assert!(!format!("{leak:?}").contains(&grant.token));
        assert!(store.agent_command(&leak).is_err());
        assert!(!store.export_json().unwrap().contains(&grant.token));
        store
            .connection
            .execute(
                "UPDATE agent_capabilities SET expires_at_unix=0 WHERE run_id=?1",
                [&grant.run_id],
            )
            .unwrap();
        assert!(
            store
                .agent_command(&command(&grant, "project.get", json!({})))
                .is_err()
        );
        assert!(
            store
                .issue_agent_grant("p", "T", folder.path(), &["task.get".into()], 30)
                .is_err()
        );
    }

    #[test]
    fn read_redaction_masks_credential_fields_and_bearer_text() {
        let value = json!({"nested":{"api_key":"plain-value","note":"Bearer abc"},"safe":"keep"});
        let redacted = redact_value(value, "abc");
        assert_eq!(redacted["nested"]["api_key"], "[REDACTED]");
        assert_eq!(redacted["nested"]["note"], "[REDACTED]");
        assert_eq!(redacted["safe"], "keep");
    }
}
