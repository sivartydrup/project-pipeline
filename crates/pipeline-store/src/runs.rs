use super::discovery::insert_event;
use super::{Result, Store, StoreError};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckoutRecord {
    pub id: String,
    pub project_id: String,
    pub task_id: String,
    pub path: String,
    pub base_commit: String,
    pub state: String,
    pub run_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRecord {
    pub id: String,
    pub project_id: String,
    pub task_id: String,
    pub task_logical_id: String,
    pub checkout_path: String,
    pub state: String,
    pub revision: i64,
    pub task_revision: i64,
    pub scope_revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCheckpoint {
    pub id: String,
    pub run_id: String,
    pub event_cursor: i64,
    pub git_head: Option<String>,
    pub diff_summary: String,
    pub task_revision: i64,
    pub run_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRequestRecord {
    pub id: String,
    pub project_id: String,
    pub task_id: String,
    pub run_id: String,
    pub action_class: String,
    pub target: String,
    pub command_digest: String,
    pub effect_summary: String,
    pub scope_revision: i64,
    pub status: String,
    pub revision: i64,
    pub expires_at_unix: Option<i64>,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn checkout_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CheckoutRecord> {
    Ok(CheckoutRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        task_id: row.get(2)?,
        path: row.get(3)?,
        base_commit: row.get(4)?,
        state: row.get(5)?,
        run_id: row.get(6)?,
    })
}

fn run_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunRecord> {
    Ok(RunRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        task_id: row.get(2)?,
        task_logical_id: row.get(3)?,
        checkout_path: row.get(4)?,
        state: row.get(5)?,
        revision: row.get(6)?,
        task_revision: row.get(7)?,
        scope_revision: row.get(8)?,
    })
}

fn policy_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PolicyRequestRecord> {
    Ok(PolicyRequestRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        task_id: row.get(2)?,
        run_id: row.get(3)?,
        action_class: row.get(4)?,
        target: row.get(5)?,
        command_digest: row.get(6)?,
        effect_summary: row.get(7)?,
        scope_revision: row.get(8)?,
        status: row.get(9)?,
        revision: row.get(10)?,
        expires_at_unix: row.get(11)?,
    })
}

fn get_checkout_from(connection: &Connection, id: &str) -> Result<CheckoutRecord> {
    connection.query_row("SELECT id,project_id,task_id,path,base_commit,state,run_id FROM managed_checkouts WHERE id=?1",
        [id],checkout_row).optional()?.ok_or_else(||StoreError::NotFound(id.into()))
}

fn get_run_from(connection: &Connection, id: &str) -> Result<RunRecord> {
    connection.query_row("SELECT r.id,r.project_id,r.task_id,t.logical_id,r.checkout_path,r.state,r.revision,t.revision,t.scope_revision
        FROM agent_runs r JOIN tasks t ON t.id=r.task_id WHERE r.id=?1",[id],run_row)
        .optional()?.ok_or_else(||StoreError::NotFound(id.into()))
}

fn get_policy_from(connection: &Connection, id: &str) -> Result<PolicyRequestRecord> {
    connection
        .query_row(
            "SELECT id,project_id,task_id,run_id,action_class,target,command_digest,effect_summary,
        scope_revision,status,revision,expires_at_unix FROM policy_requests WHERE id=?1",
            [id],
            policy_row,
        )
        .optional()?
        .ok_or_else(|| StoreError::NotFound(id.into()))
}

fn active_scope(connection: &Connection, run: &RunRecord) -> Result<()> {
    let scope: i64 = connection.query_row(
        "SELECT active_scope_revision FROM projects WHERE id=?1",
        [&run.project_id],
        |r| r.get(0),
    )?;
    if scope != run.scope_revision {
        return Err(StoreError::RunTransition("approved scope changed".into()));
    }
    Ok(())
}

fn revoke_run_policy(connection: &Connection, run_id: &str) -> Result<()> {
    connection.execute(
        "UPDATE approvals SET revoked_at=CURRENT_TIMESTAMP WHERE subject_type='policy_request'
        AND subject_id IN (SELECT id FROM policy_requests WHERE run_id=?1) AND revoked_at IS NULL",
        [run_id],
    )?;
    connection.execute("UPDATE policy_requests SET status='denied',revision=revision+1,updated_at=CURRENT_TIMESTAMP
        WHERE run_id=?1 AND status IN ('pending','approved')",[run_id])?;
    Ok(())
}

pub(crate) fn interrupt_project_on_scope_change(
    connection: &Connection,
    project_id: &str,
) -> Result<()> {
    let mut statement=connection.prepare("SELECT r.id,r.state,r.revision,r.task_id,t.revision FROM agent_runs r
        JOIN tasks t ON t.id=r.task_id WHERE r.project_id=?1 AND r.state IN ('queued','starting','running','waiting_for_input','review')")?;
    let runs = statement
        .query_map([project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (id, state, revision, task_id, task_revision) in runs {
        let cursor: i64 = connection.query_row(
            "SELECT COALESCE(MAX(sequence),0) FROM run_events WHERE run_id=?1",
            [&id],
            |r| r.get(0),
        )?;
        let checkpoint_id = Uuid::new_v4().to_string();
        connection.execute("INSERT INTO run_checkpoints(id,run_id,event_cursor,diff_summary,task_revision,run_state)
            VALUES (?1,?2,?3,'approved scope changed; inspect retained checkout',?4,?5)",
            params![checkpoint_id,id,cursor,task_revision,state])?;
        connection.execute("UPDATE agent_runs SET state='interrupted',revision=revision+1,ended_at=CURRENT_TIMESTAMP WHERE id=?1",[&id])?;
        connection.execute("UPDATE agent_capabilities SET revoked_at=CURRENT_TIMESTAMP WHERE run_id=?1 AND revoked_at IS NULL",[&id])?;
        connection.execute("UPDATE managed_checkouts SET state='retained',updated_at=CURRENT_TIMESTAMP WHERE run_id=?1 AND state='active'",[&id])?;
        connection.execute("UPDATE tasks SET status='blocked',revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?1 AND status='running'",[&task_id])?;
        revoke_run_policy(connection, &id)?;
        insert_event(
            connection,
            project_id,
            "scheduler",
            "run.interrupt",
            "run",
            &id,
            revision + 1,
            Some(json!({"state":state})),
            Some(
                json!({"state":"interrupted","reason":"scope_change","checkpoint_id":checkpoint_id}),
            ),
            &id,
        )?;
    }
    Ok(())
}

impl Store {
    pub fn get_run(&self, id: &str) -> Result<RunRecord> {
        get_run_from(&self.connection, id)
    }

    pub fn resume_blocked_task(
        &mut self,
        project_id: &str,
        logical_id: &str,
        expected_revision: i64,
        actor: &str,
    ) -> Result<()> {
        if actor != "owner" {
            return Err(StoreError::RunTransition(
                "only owner may resume blocked work".into(),
            ));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task: Option<(String, i64, String)> = tx
            .query_row(
                "SELECT t.id,t.revision,t.status FROM tasks t JOIN projects p ON p.id=t.project_id
            WHERE t.project_id=?1 AND t.logical_id=?2 AND t.scope_revision=p.active_scope_revision",
                params![project_id, logical_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let (id, revision, status) = task.ok_or_else(|| StoreError::NotFound(logical_id.into()))?;
        if revision != expected_revision {
            return Err(StoreError::RevisionConflict {
                id,
                expected: expected_revision,
                actual: revision,
            });
        }
        if status != "blocked" {
            return Err(StoreError::RunTransition("task is not blocked".into()));
        }
        let blockers: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM dependencies d JOIN tasks prerequisite ON prerequisite.id=d.to_task_id
            WHERE d.from_task_id=?1 AND d.kind='blocks' AND prerequisite.status!='accepted')",[&id],|r|r.get(0))?;
        if blockers {
            return Err(StoreError::TaskDependencyBlocked(logical_id.into()));
        }
        tx.execute("UPDATE tasks SET status='ready',revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?1",[&id])?;
        insert_event(
            &tx,
            project_id,
            "owner",
            "task.resume",
            "task",
            &id,
            revision + 1,
            Some(json!({"status":"blocked"})),
            Some(json!({"status":"ready"})),
            &id,
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn set_run_limits(&mut self, id: &str, wall_seconds: i64, token_budget: i64) -> Result<()> {
        if wall_seconds <= 0 || token_budget <= 0 {
            return Err(StoreError::RunTransition(
                "positive run limits required".into(),
            ));
        }
        let tx = self.connection.transaction()?;
        let run = get_run_from(&tx, id)?;
        if run.state != "queued" {
            return Err(StoreError::RunTransition(
                "limits must be set before start".into(),
            ));
        }
        tx.execute(
            "INSERT INTO run_limits(run_id,wall_seconds,token_budget) VALUES (?1,?2,?3)",
            params![id, wall_seconds, token_budget],
        )?;
        insert_event(
            &tx,
            &run.project_id,
            "scheduler",
            "run.limits",
            "run",
            id,
            run.revision,
            None,
            Some(json!({"wall_seconds":wall_seconds,"token_budget":token_budget})),
            id,
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn list_open_runs(&self) -> Result<Vec<RunRecord>> {
        let mut statement=self.connection.prepare("SELECT r.id,r.project_id,r.task_id,t.logical_id,r.checkout_path,r.state,r.revision,t.revision,t.scope_revision
            FROM agent_runs r JOIN tasks t ON t.id=r.task_id WHERE r.state IN ('queued','starting','running','waiting_for_input','review') ORDER BY r.rowid")?;
        statement
            .query_map([], run_row)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_checkout(&self, id: &str) -> Result<CheckoutRecord> {
        get_checkout_from(&self.connection, id)
    }

    pub fn list_pending_checkouts(&self) -> Result<Vec<CheckoutRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT id,project_id,task_id,path,base_commit,state,run_id
            FROM managed_checkouts WHERE state IN ('intent','prepared','failed') ORDER BY rowid",
        )?;
        statement
            .query_map([], checkout_row)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn record_checkout_intent(
        &mut self,
        project_id: &str,
        logical_task_id: &str,
        path: &str,
        base_commit: &str,
    ) -> Result<CheckoutRecord> {
        if path.trim().is_empty() || base_commit.trim().is_empty() {
            return Err(StoreError::EmptyField("checkout path or base commit"));
        }
        let task = self
            .list_runnable_tasks(project_id)?
            .into_iter()
            .find(|t| t.logical_id == logical_task_id)
            .ok_or_else(|| StoreError::AgentDenied("task is not ready in approved scope".into()))?;
        let id = Uuid::new_v4().to_string();
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let busy: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM agent_runs WHERE project_id=?1 AND state IN ('queued','starting','running','waiting_for_input','review'))",[project_id],|r|r.get(0))?;
        if busy {
            return Err(StoreError::RunTransition(
                "project already has an active run".into(),
            ));
        }
        let pending: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM managed_checkouts WHERE project_id=?1 AND state IN ('intent','prepared','active'))",[project_id],|r|r.get(0))?;
        if pending {
            return Err(StoreError::RunTransition(
                "project already has a pending checkout".into(),
            ));
        }
        let ready: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
            WHERE t.id=?1 AND t.scope_revision=p.active_scope_revision AND t.status='ready'
            AND NOT EXISTS(SELECT 1 FROM dependencies d JOIN tasks prerequisite ON prerequisite.id=d.to_task_id
              WHERE d.from_task_id=t.id AND d.kind='blocks' AND prerequisite.status!='accepted'))",[&task.id],|r|r.get(0))?;
        if !ready {
            return Err(StoreError::RunTransition(
                "task changed during preflight".into(),
            ));
        }
        tx.execute("INSERT INTO managed_checkouts(id,project_id,task_id,path,base_commit,state) VALUES (?1,?2,?3,?4,?5,'intent')",
            params![id,project_id,task.id,path,base_commit])?;
        insert_event(
            &tx,
            project_id,
            "scheduler",
            "checkout.intent",
            "checkout",
            &id,
            1,
            None,
            Some(json!({"task_id":task.id,"path":path,"base_commit":base_commit})),
            &id,
        )?;
        tx.commit()?;
        self.get_checkout(&id)
    }

    pub fn update_checkout_state(&mut self, id: &str, to: &str) -> Result<CheckoutRecord> {
        let tx = self.connection.transaction()?;
        let before = get_checkout_from(&tx, id)?;
        let allowed = matches!(
            (before.state.as_str(), to),
            ("intent", "prepared")
                | ("intent", "failed")
                | ("prepared", "failed")
                | ("active", "retained")
                | ("failed", "retained")
                | ("prepared", "retained")
        );
        if !allowed {
            return Err(StoreError::RunTransition(format!(
                "checkout {} -> {to}",
                before.state
            )));
        }
        let path = if to == "prepared" {
            dunce::canonicalize(&before.path)
                .map_err(|_| StoreError::RunTransition("prepared checkout missing".into()))?
                .to_string_lossy()
                .into_owned()
        } else {
            before.path.clone()
        };
        tx.execute(
            "UPDATE managed_checkouts SET state=?1,path=?2,updated_at=CURRENT_TIMESTAMP WHERE id=?3",
            params![to,path,id],
        )?;
        insert_event(
            &tx,
            &before.project_id,
            "scheduler",
            "checkout.transition",
            "checkout",
            id,
            1,
            Some(json!({"state":before.state})),
            Some(json!({"state":to,"path":path})),
            id,
        )?;
        tx.commit()?;
        self.get_checkout(id)
    }

    pub fn transition_run(
        &mut self,
        id: &str,
        expected_revision: i64,
        to: &str,
    ) -> Result<RunRecord> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let before = get_run_from(&tx, id)?;
        if before.revision != expected_revision {
            return Err(StoreError::RevisionConflict {
                id: id.into(),
                expected: expected_revision,
                actual: before.revision,
            });
        }
        active_scope(&tx, &before)?;
        let allowed = matches!(
            (before.state.as_str(), to),
            (
                "queued",
                "starting" | "cancelled" | "failed" | "interrupted"
            ) | (
                "starting",
                "running" | "failed" | "cancelled" | "interrupted"
            ) | (
                "running",
                "waiting_for_input"
                    | "review"
                    | "completed"
                    | "failed"
                    | "cancelled"
                    | "interrupted",
            ) | (
                "waiting_for_input",
                "running" | "review" | "failed" | "cancelled" | "interrupted",
            ) | (
                "review",
                "completed" | "failed" | "cancelled" | "interrupted"
            )
        );
        if !allowed {
            return Err(StoreError::RunTransition(format!(
                "{} -> {to}",
                before.state
            )));
        }
        if to == "starting" {
            let limits: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM run_limits WHERE run_id=?1)",
                [id],
                |r| r.get(0),
            )?;
            if !limits {
                return Err(StoreError::RunTransition("run limits are missing".into()));
            }
        }
        if to == "completed" {
            let status: String = tx.query_row(
                "SELECT status FROM tasks WHERE id=?1",
                [&before.task_id],
                |r| r.get(0),
            )?;
            if status != "review" {
                return Err(StoreError::RunTransition(
                    "task must be submitted for review".into(),
                ));
            }
        }
        if to == "running" {
            tx.execute("UPDATE tasks SET status='running',revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?1 AND status='ready'",[&before.task_id])?;
        }
        if matches!(to, "failed" | "cancelled" | "interrupted") {
            tx.execute("UPDATE tasks SET status='blocked',revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?1 AND status='running'",[&before.task_id])?;
        }
        tx.execute("UPDATE agent_runs SET state=?1,revision=revision+1,ended_at=CASE WHEN ?1 IN ('completed','failed','cancelled','interrupted') THEN CURRENT_TIMESTAMP ELSE ended_at END WHERE id=?2",params![to,id])?;
        if matches!(to, "completed" | "failed" | "cancelled" | "interrupted") {
            revoke_run_policy(&tx, id)?;
            tx.execute("UPDATE agent_capabilities SET revoked_at=CURRENT_TIMESTAMP WHERE run_id=?1 AND revoked_at IS NULL",[id])?;
            tx.execute("UPDATE managed_checkouts SET state='retained',updated_at=CURRENT_TIMESTAMP WHERE run_id=?1 AND state='active'",[id])?;
        }
        insert_event(
            &tx,
            &before.project_id,
            "scheduler",
            "run.transition",
            "run",
            id,
            before.revision + 1,
            Some(json!({"state":before.state})),
            Some(json!({"state":to})),
            id,
        )?;
        tx.commit()?;
        self.get_run(id)
    }

    pub fn checkpoint_and_interrupt(
        &mut self,
        id: &str,
        expected_revision: i64,
        event_cursor: i64,
        git_head: Option<&str>,
        diff_summary: &str,
    ) -> Result<RunCheckpoint> {
        if event_cursor < 0 || diff_summary.len() > 4096 {
            return Err(StoreError::RunTransition("invalid checkpoint".into()));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let before = get_run_from(&tx, id)?;
        if before.revision != expected_revision {
            return Err(StoreError::RevisionConflict {
                id: id.into(),
                expected: expected_revision,
                actual: before.revision,
            });
        }
        if !matches!(
            before.state.as_str(),
            "queued" | "starting" | "running" | "waiting_for_input" | "review"
        ) {
            return Err(StoreError::RunTransition("run is not active".into()));
        }
        let checkpoint_id = Uuid::new_v4().to_string();
        tx.execute("INSERT INTO run_checkpoints(id,run_id,event_cursor,git_head,diff_summary,task_revision,run_state)
            VALUES (?1,?2,?3,?4,?5,?6,?7)",params![checkpoint_id,id,event_cursor,git_head,diff_summary,before.task_revision,before.state])?;
        tx.execute("UPDATE agent_runs SET state='interrupted',revision=revision+1,ended_at=CURRENT_TIMESTAMP WHERE id=?1",[id])?;
        revoke_run_policy(&tx, id)?;
        tx.execute("UPDATE agent_capabilities SET revoked_at=CURRENT_TIMESTAMP WHERE run_id=?1 AND revoked_at IS NULL",[id])?;
        tx.execute("UPDATE managed_checkouts SET state='retained',updated_at=CURRENT_TIMESTAMP WHERE run_id=?1 AND state='active'",[id])?;
        tx.execute("UPDATE tasks SET status='blocked',revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?1 AND status='running'",[&before.task_id])?;
        insert_event(
            &tx,
            &before.project_id,
            "scheduler",
            "run.interrupt",
            "run",
            id,
            before.revision + 1,
            Some(json!({"state":before.state})),
            Some(
                json!({"state":"interrupted","checkpoint_id":checkpoint_id,"diff_summary":diff_summary}),
            ),
            id,
        )?;
        tx.commit()?;
        Ok(RunCheckpoint {
            id: checkpoint_id,
            run_id: id.into(),
            event_cursor,
            git_head: git_head.map(str::to_owned),
            diff_summary: diff_summary.into(),
            task_revision: before.task_revision,
            run_state: before.state,
        })
    }

    pub fn latest_run_cursor(&self, id: &str) -> Result<i64> {
        Ok(self.connection.query_row(
            "SELECT COALESCE(MAX(sequence),0) FROM run_events WHERE run_id=?1",
            [id],
            |r| r.get(0),
        )?)
    }

    pub fn record_local_action(
        &mut self,
        run_id: &str,
        action_class: &str,
        target: &str,
        command_digest: &str,
    ) -> Result<()> {
        if !["local_check", "edit_checkout"].contains(&action_class)
            || target.trim().is_empty()
            || command_digest.len() != 64
        {
            return Err(StoreError::PolicyDenied("invalid local action".into()));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = get_run_from(&tx, run_id)?;
        active_scope(&tx, &run)?;
        if !matches!(run.state.as_str(), "starting" | "running") {
            return Err(StoreError::PolicyDenied("run is not active".into()));
        }
        insert_event(
            &tx,
            &run.project_id,
            "scheduler",
            "policy.allow_local",
            "run",
            run_id,
            run.revision,
            None,
            Some(
                json!({"action_class":action_class,"target":target,"command_digest":command_digest}),
            ),
            run_id,
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_policy_request(&self, id: &str) -> Result<PolicyRequestRecord> {
        get_policy_from(&self.connection, id)
    }

    pub fn list_pending_policy_requests(
        &self,
        project_id: &str,
    ) -> Result<Vec<PolicyRequestRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT id,project_id,task_id,run_id,action_class,target,command_digest,
            effect_summary,scope_revision,status,revision,expires_at_unix FROM policy_requests
            WHERE project_id=?1 AND status='pending' ORDER BY rowid",
        )?;
        statement
            .query_map([project_id], policy_row)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn request_policy_action(
        &mut self,
        run_id: &str,
        action_class: &str,
        target: &str,
        command_digest: &str,
        effect_summary: &str,
    ) -> Result<PolicyRequestRecord> {
        if ![
            "destructive_files",
            "rewrite_git",
            "credential_access",
            "spend",
            "external_publish",
            "production",
            "unknown_external",
        ]
        .contains(&action_class)
            || target.trim().is_empty()
            || command_digest.len() != 64
            || !command_digest.chars().all(|c| c.is_ascii_hexdigit())
            || effect_summary.trim().is_empty()
            || effect_summary.len() > 1000
        {
            return Err(StoreError::PolicyDenied("invalid action request".into()));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = get_run_from(&tx, run_id)?;
        active_scope(&tx, &run)?;
        if !matches!(
            run.state.as_str(),
            "starting" | "running" | "waiting_for_input"
        ) {
            return Err(StoreError::PolicyDenied("run is not active".into()));
        }
        let prior: Option<String>=tx.query_row("SELECT id FROM policy_requests WHERE run_id=?1 AND action_class=?2 AND target=?3 AND command_digest=?4",
            params![run_id,action_class,target,command_digest],|r|r.get(0)).optional()?;
        if let Some(id) = prior {
            return get_policy_from(&tx, &id);
        }
        let id = Uuid::new_v4().to_string();
        tx.execute("INSERT INTO policy_requests(id,project_id,task_id,run_id,action_class,target,command_digest,effect_summary,scope_revision,status)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'pending')",params![id,run.project_id,run.task_id,run_id,action_class,target,command_digest,effect_summary,run.scope_revision])?;
        insert_event(
            &tx,
            &run.project_id,
            "scheduler",
            "policy.request",
            "policy",
            &id,
            1,
            None,
            Some(
                json!({"action_class":action_class,"target":target,"effect_summary":effect_summary}),
            ),
            &id,
        )?;
        tx.commit()?;
        self.get_policy_request(&id)
    }

    pub fn resolve_policy_request(
        &mut self,
        id: &str,
        expected_revision: i64,
        approve: bool,
        expires_at_unix: Option<i64>,
        actor: &str,
    ) -> Result<PolicyRequestRecord> {
        if actor != "owner" {
            return Err(StoreError::PolicyDenied(
                "only owner may resolve policy requests".into(),
            ));
        }
        if approve
            && expires_at_unix.is_none_or(|expiry| expiry <= now() || expiry > now() + 86_400)
        {
            return Err(StoreError::PolicyDenied(
                "approval expiry must be within 24 hours".into(),
            ));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let before = get_policy_from(&tx, id)?;
        if before.revision != expected_revision {
            return Err(StoreError::RevisionConflict {
                id: id.into(),
                expected: expected_revision,
                actual: before.revision,
            });
        }
        if before.status != "pending" {
            return Err(StoreError::PolicyDenied(
                "request is already resolved".into(),
            ));
        }
        let run = get_run_from(&tx, &before.run_id)?;
        active_scope(&tx, &run)?;
        if !matches!(
            run.state.as_str(),
            "starting" | "running" | "waiting_for_input"
        ) {
            return Err(StoreError::PolicyDenied(
                "run ended before owner decision".into(),
            ));
        }
        let status = if approve { "approved" } else { "denied" };
        tx.execute("UPDATE policy_requests SET status=?1,revision=revision+1,expires_at_unix=?2,updated_at=CURRENT_TIMESTAMP WHERE id=?3",
            params![status,if approve {expires_at_unix} else {None},id])?;
        if approve {
            let hash = format!(
                "{:x}",
                Sha256::digest(
                    format!(
                        "{}:{}:{}:{}:{}",
                        before.run_id,
                        before.action_class,
                        before.target,
                        before.command_digest,
                        before.scope_revision
                    )
                    .as_bytes()
                )
            );
            tx.execute("INSERT INTO approvals(id,project_id,subject_type,subject_id,subject_revision,subject_hash,action_class,target,approver,expires_at)
                VALUES (?1,?2,'policy_request',?3,2,?4,?5,?6,'owner',?7)",
                params![Uuid::new_v4().to_string(),before.project_id,id,hash,before.action_class,before.target,expires_at_unix.map(|v|v.to_string())])?;
        }
        insert_event(
            &tx,
            &before.project_id,
            "owner",
            "policy.resolve",
            "policy",
            id,
            before.revision + 1,
            Some(json!({"status":before.status})),
            Some(json!({"status":status,"expires_at_unix":expires_at_unix})),
            id,
        )?;
        tx.commit()?;
        self.get_policy_request(id)
    }

    pub fn consume_policy_action(
        &mut self,
        id: &str,
        run_id: &str,
        action_class: &str,
        target: &str,
        command_digest: &str,
    ) -> Result<PolicyRequestRecord> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let before = get_policy_from(&tx, id)?;
        let run = get_run_from(&tx, run_id)?;
        active_scope(&tx, &run)?;
        if before.run_id != run_id
            || before.action_class != action_class
            || before.target != target
            || before.command_digest != command_digest
            || before.scope_revision != run.scope_revision
        {
            return Err(StoreError::PolicyDenied("approval scope mismatch".into()));
        }
        if before.status != "approved"
            || before.expires_at_unix.is_none_or(|expiry| expiry <= now())
        {
            return Err(StoreError::PolicyDenied(format!(
                "action remains {}",
                before.status
            )));
        }
        if !matches!(
            run.state.as_str(),
            "starting" | "running" | "waiting_for_input"
        ) {
            return Err(StoreError::PolicyDenied("run is not active".into()));
        }
        tx.execute("UPDATE policy_requests SET status='consumed',revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?1",[id])?;
        insert_event(
            &tx,
            &before.project_id,
            "scheduler",
            "policy.consume",
            "policy",
            id,
            before.revision + 1,
            Some(json!({"status":"approved"})),
            Some(json!({"status":"consumed","target":target})),
            id,
        )?;
        tx.commit()?;
        self.get_policy_request(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AGENT_BRIDGE_SCHEMA, DECISIONS_SCHEMA, DISCOVERY_SCHEMA, INITIAL_SCHEMA, PLAN_SCHEMA,
        PROJECT_GIT_SCHEMA, apply_migration,
    };

    #[test]
    fn v6_runs_migrate_to_state_machine_and_keep_backup() {
        let folder = tempfile::tempdir().unwrap();
        let database = folder.path().join("pipeline.sqlite");
        let mut connection = Connection::open(&database).unwrap();
        for (version, sql) in [
            (1, INITIAL_SCHEMA),
            (2, PROJECT_GIT_SCHEMA),
            (3, DISCOVERY_SCHEMA),
            (4, PLAN_SCHEMA),
            (5, DECISIONS_SCHEMA),
            (6, AGENT_BRIDGE_SCHEMA),
        ] {
            apply_migration(&mut connection, version, sql).unwrap();
        }
        connection
            .execute(
                "INSERT INTO projects(id,name,path) VALUES ('p','Project','C:/p')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO tasks(id,project_id,title,outcome,status,scope_revision,logical_id)
            VALUES ('t','p','Task','Done','ready',1,'T')",
                [],
            )
            .unwrap();
        for (id, state) in [("r1", "ready"), ("r2", "closed")] {
            connection
                .execute(
                    "INSERT INTO agent_runs(id,project_id,task_id,harness,checkout_path,state)
                VALUES (?1,'p','t','bridge','C:/p',?2)",
                    params![id, state],
                )
                .unwrap();
        }
        drop(connection);
        let store = Store::open(&database).unwrap();
        assert_eq!(store.schema_version().unwrap(), 7);
        assert_eq!(store.get_run("r1").unwrap().state, "queued");
        assert_eq!(store.get_run("r2").unwrap().state, "cancelled");
        assert!(database.with_extension("pre-v6.sqlite").exists());
        assert!(store.list_pending_policy_requests("p").unwrap().is_empty());
    }
}
