use super::discovery::insert_event;
use super::runs::interrupt_project_on_scope_change;
use super::{Result, Store, StoreError, validate_nonempty};
use pipeline_domain::{DependencyKind, PlanContent, TaskSpec};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRecord {
    pub id: String,
    pub project_id: String,
    pub revision: i64,
    pub status: String,
    pub content_hash: String,
    pub scope_revision: Option<i64>,
    pub content: PlanContent,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanStatus {
    pub latest_revision: i64,
    pub active_scope_revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriterionRecord {
    pub logical_id: String,
    pub assertion: String,
    pub verifier: String,
    pub required_evidence: Vec<String>,
    pub evidence: BTreeMap<String, String>,
    pub verified_by: Option<String>,
    pub accepted_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRecord {
    pub id: String,
    pub logical_id: String,
    pub title: String,
    pub outcome: String,
    pub status: String,
    pub weight: i64,
    pub revision: i64,
    pub scope_revision: i64,
    pub criteria: Vec<CriterionRecord>,
    pub unresolved_blockers: Vec<String>,
}

impl Store {
    pub fn plan_status(&self, project_id: &str) -> Result<PlanStatus> {
        let active: Option<i64> = self
            .connection
            .query_row(
                "SELECT active_scope_revision FROM projects WHERE id=?1",
                [project_id],
                |row| row.get(0),
            )
            .optional()?;
        let active_scope_revision =
            active.ok_or_else(|| StoreError::NotFound(project_id.to_owned()))?;
        let latest_revision = self.connection.query_row(
            "SELECT COALESCE(MAX(revision),0) FROM plan_revisions WHERE project_id=?1",
            [project_id],
            |row| row.get(0),
        )?;
        Ok(PlanStatus {
            latest_revision,
            active_scope_revision,
        })
    }

    pub fn list_plan_revisions(&self, project_id: &str) -> Result<Vec<PlanRecord>> {
        self.plan_status(project_id)?;
        let mut statement = self.connection.prepare(
            "SELECT id,revision,status,content_json,content_hash,scope_revision,created_at
             FROM plan_revisions WHERE project_id=?1 ORDER BY revision",
        )?;
        let rows = statement.query_map([project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, String>(6)?,
            ))
        })?;
        rows.map(|row| {
            let (id, revision, status, content_json, content_hash, scope_revision, created_at) =
                row?;
            Ok(PlanRecord {
                id,
                project_id: project_id.to_owned(),
                revision,
                status,
                content_hash,
                scope_revision,
                content: serde_json::from_str(&content_json)?,
                created_at,
            })
        })
        .collect()
    }

    pub fn save_plan_revision(
        &mut self,
        id: &str,
        project_id: &str,
        expected_latest_revision: i64,
        content: &PlanContent,
        actor: &str,
        correlation_id: &str,
    ) -> Result<PlanRecord> {
        for (field, value) in [
            ("id", id),
            ("actor", actor),
            ("correlation_id", correlation_id),
        ] {
            validate_nonempty(field, value)?;
        }
        let transaction = self.connection.transaction()?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
            [project_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(StoreError::NotFound(project_id.to_owned()));
        }
        let actual: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(revision),0) FROM plan_revisions WHERE project_id=?1",
            [project_id],
            |row| row.get(0),
        )?;
        if actual != expected_latest_revision {
            return Err(StoreError::PlanRevisionConflict {
                id: project_id.to_owned(),
                expected: expected_latest_revision,
                actual,
            });
        }
        let revision = actual + 1;
        let content_json = serde_json::to_string(content)?;
        let content_hash = hash(&content_json);
        transaction.execute(
            "INSERT INTO plan_revisions(id,project_id,revision,status,content_json,content_hash)
             VALUES (?1,?2,?3,'draft',?4,?5)",
            params![id, project_id, revision, content_json, content_hash],
        )?;
        transaction.execute(
            "UPDATE projects SET revision=revision+1,health='needs_input',updated_at=CURRENT_TIMESTAMP WHERE id=?1",
            [project_id],
        )?;
        insert_event(
            &transaction,
            project_id,
            actor,
            "plan.revise",
            "plan",
            id,
            revision,
            Some(json!({"previous_revision": actual})),
            Some(json!({"revision": revision, "content_hash": content_hash})),
            correlation_id,
        )?;
        let created_at: String = transaction.query_row(
            "SELECT created_at FROM plan_revisions WHERE id=?1",
            [id],
            |row| row.get(0),
        )?;
        transaction.commit()?;
        Ok(PlanRecord {
            id: id.to_owned(),
            project_id: project_id.to_owned(),
            revision,
            status: "draft".to_owned(),
            content_hash,
            scope_revision: None,
            content: content.clone(),
            created_at,
        })
    }

    pub fn approve_plan_revision(
        &mut self,
        project_id: &str,
        expected_latest_revision: i64,
        approval_id: &str,
        actor: &str,
        correlation_id: &str,
    ) -> Result<PlanRecord> {
        if actor != "owner" {
            return Err(StoreError::PlanRequiresOwner);
        }
        validate_nonempty("approval_id", approval_id)?;
        validate_nonempty("correlation_id", correlation_id)?;
        let transaction = self.connection.transaction()?;
        let project: Option<(i64, i64)> = transaction
            .query_row(
                "SELECT active_scope_revision,approved_brief_revision FROM projects WHERE id=?1",
                [project_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (active_scope, approved_brief) =
            project.ok_or_else(|| StoreError::NotFound(project_id.to_owned()))?;
        if approved_brief == 0 {
            return Err(StoreError::PlanRequiresBrief);
        }
        let latest_brief: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(revision),0) FROM brief_revisions WHERE project_id=?1",
            [project_id],
            |row| row.get(0),
        )?;
        if approved_brief != latest_brief {
            return Err(StoreError::PlanRequiresCurrentBrief);
        }
        let actual: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(revision),0) FROM plan_revisions WHERE project_id=?1",
            [project_id],
            |row| row.get(0),
        )?;
        if actual != expected_latest_revision || actual == 0 {
            return Err(StoreError::PlanRevisionConflict {
                id: project_id.to_owned(),
                expected: expected_latest_revision,
                actual,
            });
        }
        let (plan_id, status, content_json, content_hash, created_at): (
            String,
            String,
            String,
            String,
            String,
        ) = transaction.query_row(
            "SELECT id,status,content_json,content_hash,created_at FROM plan_revisions
                 WHERE project_id=?1 AND revision=?2",
            params![project_id, actual],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )?;
        if status == "approved" {
            return Err(StoreError::PlanAlreadyApproved);
        }
        let content: PlanContent = serde_json::from_str(&content_json)?;
        content.validate()?;
        let previous = if active_scope > 0 {
            transaction
                .query_row(
                    "SELECT content_json FROM plan_revisions WHERE project_id=?1 AND scope_revision=?2",
                    params![project_id, active_scope],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .map(|json| serde_json::from_str::<PlanContent>(&json))
                .transpose()?
        } else {
            None
        };
        let scope_revision = active_scope + 1;
        materialize_plan(
            &transaction,
            project_id,
            &plan_id,
            scope_revision,
            &content,
            previous.as_ref(),
            active_scope,
        )?;
        transaction.execute(
            "UPDATE plan_revisions SET status='approved',scope_revision=?1 WHERE id=?2",
            params![scope_revision, plan_id],
        )?;
        transaction.execute(
            "INSERT INTO approvals(id,project_id,subject_type,subject_id,subject_revision,subject_hash,
             action_class,target,approver) VALUES (?1,?2,'plan',?3,?4,?5,'approve_scope',?2,?6)",
            params![approval_id, project_id, plan_id, actual, content_hash, actor],
        )?;
        transaction.execute(
            "UPDATE projects SET active_scope_revision=?1,stage=CASE WHEN stage IN
             ('idea','discovery','planning') THEN 'design' ELSE stage END,
             revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![scope_revision, project_id],
        )?;
        interrupt_project_on_scope_change(&transaction, project_id)?;
        insert_event(
            &transaction,
            project_id,
            actor,
            "plan.approve",
            "plan",
            &plan_id,
            actual,
            Some(json!({"active_scope_revision": active_scope})),
            Some(
                json!({"scope_revision": scope_revision, "content_hash": content_hash,
                "approval_id": approval_id}),
            ),
            correlation_id,
        )?;
        transaction.commit()?;
        Ok(PlanRecord {
            id: plan_id,
            project_id: project_id.to_owned(),
            revision: actual,
            status: "approved".to_owned(),
            content_hash,
            scope_revision: Some(scope_revision),
            content,
            created_at,
        })
    }

    pub fn list_active_tasks(&self, project_id: &str) -> Result<Vec<TaskRecord>> {
        let scope = self.plan_status(project_id)?.active_scope_revision;
        if scope == 0 {
            return Ok(Vec::new());
        }
        let mut statement = self.connection.prepare(
            "SELECT id,logical_id,title,outcome,status,weight,revision FROM tasks
             WHERE project_id=?1 AND scope_revision=?2 ORDER BY rowid",
        )?;
        let rows = statement.query_map(params![project_id, scope], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })?;
        rows.map(|row| {
            let (id, logical_id, title, outcome, status, weight, revision) = row?;
            Ok(TaskRecord {
                criteria: criteria_for(&self.connection, &id)?,
                unresolved_blockers: blockers_for(&self.connection, &id)?,
                id,
                logical_id,
                title,
                outcome,
                status,
                weight,
                revision,
                scope_revision: scope,
            })
        })
        .collect()
    }

    pub fn list_runnable_tasks(&self, project_id: &str) -> Result<Vec<TaskRecord>> {
        Ok(self
            .list_active_tasks(project_id)?
            .into_iter()
            .filter(|task| task.status == "ready" && task.unresolved_blockers.is_empty())
            .collect())
    }

    pub fn submit_task_for_review(
        &mut self,
        project_id: &str,
        logical_id: &str,
        expected_revision: i64,
        actor: &str,
        correlation_id: &str,
    ) -> Result<()> {
        self.transition_task(
            project_id,
            logical_id,
            expected_revision,
            "review",
            actor,
            correlation_id,
        )
    }

    pub fn accept_task(
        &mut self,
        project_id: &str,
        logical_id: &str,
        expected_revision: i64,
        actor: &str,
        correlation_id: &str,
    ) -> Result<()> {
        if actor != "owner" {
            return Err(StoreError::PlanRequiresOwner);
        }
        self.transition_task(
            project_id,
            logical_id,
            expected_revision,
            "accepted",
            actor,
            correlation_id,
        )
    }

    fn transition_task(
        &mut self,
        project_id: &str,
        logical_id: &str,
        expected_revision: i64,
        to: &'static str,
        actor: &str,
        correlation_id: &str,
    ) -> Result<()> {
        validate_nonempty("actor", actor)?;
        validate_nonempty("correlation_id", correlation_id)?;
        let transaction = self.connection.transaction()?;
        let (id, status, revision) = active_task(&transaction, project_id, logical_id)?;
        if revision != expected_revision {
            return Err(StoreError::RevisionConflict {
                id,
                expected: expected_revision,
                actual: revision,
            });
        }
        let allowed = match to {
            "review" => status == "ready" || status == "running",
            "accepted" => status == "review",
            _ => false,
        };
        if !allowed {
            return Err(StoreError::InvalidTaskTransition { from: status, to });
        }
        if to == "review" && !blockers_for(&transaction, &id)?.is_empty() {
            return Err(StoreError::TaskDependencyBlocked(logical_id.to_owned()));
        }
        if to == "accepted" {
            super::review::require_agent_review_evidence(&transaction, &id)?;
            let criteria = criteria_for(&transaction, &id)?;
            if criteria.is_empty()
                || criteria.iter().any(|criterion| {
                    criterion.accepted_at.is_none()
                        || criterion.verified_by.is_none()
                        || criterion.required_evidence.iter().any(|kind| {
                            criterion
                                .evidence
                                .get(kind)
                                .is_none_or(|reference| reference.trim().is_empty())
                        })
                })
            {
                return Err(StoreError::CriteriaNotVerified(logical_id.to_owned()));
            }
        }
        transaction.execute(
            "UPDATE tasks SET status=?1,revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![to, id],
        )?;
        insert_event(
            &transaction,
            project_id,
            actor,
            "task.transition",
            "task",
            &id,
            revision + 1,
            Some(json!({"status": status})),
            Some(json!({"status": to})),
            correlation_id,
        )?;
        transaction.commit()?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn verify_criterion(
        &mut self,
        project_id: &str,
        logical_task_id: &str,
        logical_criterion_id: &str,
        expected_task_revision: i64,
        evidence: &BTreeMap<String, String>,
        verifier: &str,
        correlation_id: &str,
    ) -> Result<()> {
        validate_nonempty("verifier", verifier)?;
        validate_nonempty("correlation_id", correlation_id)?;
        let transaction = self.connection.transaction()?;
        let (task_id, status, revision) = active_task(&transaction, project_id, logical_task_id)?;
        if revision != expected_task_revision {
            return Err(StoreError::RevisionConflict {
                id: task_id,
                expected: expected_task_revision,
                actual: revision,
            });
        }
        if status != "review" {
            return Err(StoreError::InvalidTaskTransition {
                from: status,
                to: "verify criterion",
            });
        }
        let (criterion_id, required_json): (String, String) = transaction
            .query_row(
                "SELECT id,required_evidence_json FROM criteria WHERE task_id=?1 AND logical_id=?2",
                params![task_id, logical_criterion_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| StoreError::NotFound(logical_criterion_id.to_owned()))?;
        let required: Vec<String> = serde_json::from_str(&required_json)?;
        if required.is_empty() {
            return Err(StoreError::CriteriaNotVerified(logical_task_id.to_owned()));
        }
        for kind in &required {
            if evidence
                .get(kind)
                .is_none_or(|reference| reference.trim().is_empty())
            {
                return Err(StoreError::MissingEvidence(kind.clone()));
            }
        }
        transaction.execute(
            "UPDATE criteria SET evidence_json=?1,verified_by=?2,accepted_at=CURRENT_TIMESTAMP WHERE id=?3",
            params![serde_json::to_string(evidence)?, verifier, criterion_id],
        )?;
        transaction.execute(
            "UPDATE tasks SET revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?1",
            [&task_id],
        )?;
        insert_event(
            &transaction,
            project_id,
            verifier,
            "criterion.verify",
            "criterion",
            &criterion_id,
            revision + 1,
            None,
            Some(json!({"evidence": evidence, "verified_by": verifier})),
            correlation_id,
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn export_active_plan_json(&self, project_id: &str) -> Result<String> {
        let status = self.plan_status(project_id)?;
        let revisions = self.list_plan_revisions(project_id)?;
        let active = revisions
            .into_iter()
            .find(|revision| revision.scope_revision == Some(status.active_scope_revision))
            .ok_or_else(|| StoreError::NotFound("approved plan".to_owned()))?;
        Ok(serde_json::to_string_pretty(&json!({
            "project_id": project_id,
            "plan_revision": active.revision,
            "scope_revision": status.active_scope_revision,
            "content_hash": active.content_hash,
            "plan": active.content,
            "tasks": self.list_active_tasks(project_id)?.iter().map(|task| json!({
                "logical_id": task.logical_id,
                "status": task.status,
                "weight": task.weight,
                "criteria": task.criteria.iter().map(|criterion| json!({
                    "logical_id": criterion.logical_id,
                    "verified_by": criterion.verified_by,
                    "evidence": criterion.evidence,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        }))?)
    }
}

fn materialize_plan(
    connection: &Connection,
    project_id: &str,
    plan_id: &str,
    scope: i64,
    content: &PlanContent,
    previous: Option<&PlanContent>,
    previous_scope: i64,
) -> Result<()> {
    for (position, milestone) in content.milestones.iter().enumerate() {
        connection.execute(
            "INSERT INTO milestones(id,project_id,title,outcome,position,scope_revision)
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                format!("{plan_id}:{}", milestone.id),
                project_id,
                milestone.title,
                milestone.outcome,
                position as i64,
                scope
            ],
        )?;
    }
    for (position, epic) in content.epics.iter().enumerate() {
        connection.execute(
            "INSERT INTO epics(id,project_id,title,outcome,position) VALUES (?1,?2,?3,?4,?5)",
            params![
                format!("{plan_id}:{}", epic.id),
                project_id,
                epic.title,
                epic.outcome,
                position as i64
            ],
        )?;
    }
    for task in &content.tasks {
        let id = format!("{plan_id}:{}", task.id);
        let carry = previous
            .and_then(|plan| {
                plan.tasks
                    .iter()
                    .find(|old| old.id == task.id)
                    .map(|old| (plan, old))
            })
            .is_some_and(|(plan, old)| {
                old == task
                    && blockers_for_spec(plan, &task.id) == blockers_for_spec(content, &task.id)
            });
        let old_id = if carry && previous_scope > 0 {
            connection
                .query_row(
                    "SELECT id FROM tasks WHERE project_id=?1 AND scope_revision=?2 AND logical_id=?3 AND status='accepted'",
                    params![project_id, previous_scope, task.id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
        } else {
            None
        };
        let status = if old_id.is_some() {
            "accepted"
        } else {
            "ready"
        };
        connection.execute(
            "INSERT INTO tasks(id,project_id,epic_id,title,outcome,status,weight,risk,scope_revision,logical_id,plan_revision_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![id,project_id,task.epic_id.as_ref().map(|epic| format!("{plan_id}:{epic}")),
                task.title,task.outcome,status,task.weight,task.risk,scope,task.id,plan_id],
        )?;
        materialize_criteria(connection, &id, task, old_id.as_deref())?;
    }
    for dependency in &content.dependencies {
        connection.execute(
            "INSERT INTO dependencies(from_task_id,to_task_id,kind) VALUES (?1,?2,?3)",
            params![
                format!("{plan_id}:{}", dependency.from_task_id),
                format!("{plan_id}:{}", dependency.to_task_id),
                dependency.kind.as_str()
            ],
        )?;
    }
    Ok(())
}

fn materialize_criteria(
    connection: &Connection,
    task_id: &str,
    task: &TaskSpec,
    old_task_id: Option<&str>,
) -> Result<()> {
    for criterion in &task.criteria {
        let prior: Option<(String, Option<String>, Option<String>)> = match old_task_id {
            Some(old_id) => connection
                .query_row(
                    "SELECT evidence_json,verified_by,accepted_at FROM criteria WHERE task_id=?1 AND logical_id=?2",
                    params![old_id, criterion.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?,
            None => None,
        };
        let (evidence, verified_by, accepted_at) = prior.unwrap_or(("{}".to_owned(), None, None));
        connection.execute(
            "INSERT INTO criteria(id,task_id,assertion,verifier,required_evidence_json,logical_id,
             evidence_json,verified_by,accepted_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                format!("{task_id}:{}", criterion.id),
                task_id,
                criterion.assertion,
                criterion.verifier,
                serde_json::to_string(&criterion.required_evidence)?,
                criterion.id,
                evidence,
                verified_by,
                accepted_at
            ],
        )?;
    }
    Ok(())
}

fn blockers_for_spec(plan: &PlanContent, task_id: &str) -> Vec<String> {
    let mut blockers: Vec<_> = plan
        .dependencies
        .iter()
        .filter(|dependency| {
            dependency.kind == DependencyKind::Blocks && dependency.from_task_id == task_id
        })
        .map(|dependency| dependency.to_task_id.clone())
        .collect();
    blockers.sort();
    blockers
}

fn active_task(
    connection: &Connection,
    project_id: &str,
    logical_id: &str,
) -> Result<(String, String, i64)> {
    connection
        .query_row(
            "SELECT id,status,revision FROM tasks WHERE project_id=?1 AND logical_id=?2
             AND scope_revision=(SELECT active_scope_revision FROM projects WHERE id=?1)",
            params![project_id, logical_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| StoreError::TaskNotInActiveScope(logical_id.to_owned()))
}

fn criteria_for(connection: &Connection, task_id: &str) -> Result<Vec<CriterionRecord>> {
    let mut statement = connection.prepare(
        "SELECT logical_id,assertion,verifier,required_evidence_json,evidence_json,verified_by,accepted_at
         FROM criteria WHERE task_id=?1 ORDER BY rowid",
    )?;
    let rows = statement.query_map([task_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
        ))
    })?;
    rows.map(|row| {
        let (logical_id, assertion, verifier, required, evidence, verified_by, accepted_at) = row?;
        Ok(CriterionRecord {
            logical_id,
            assertion,
            verifier,
            required_evidence: serde_json::from_str(&required)?,
            evidence: serde_json::from_str(&evidence)?,
            verified_by,
            accepted_at,
        })
    })
    .collect()
}

fn blockers_for(connection: &Connection, task_id: &str) -> Result<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT prerequisite.logical_id FROM dependencies edge
         JOIN tasks prerequisite ON prerequisite.id=edge.to_task_id
         WHERE edge.from_task_id=?1 AND edge.kind='blocks' AND prerequisite.status<>'accepted'
         ORDER BY prerequisite.title",
    )?;
    Ok(statement
        .query_map([task_id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?)
}

fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pipeline_domain::{CriterionSpec, DependencySpec, EpicSpec, MilestoneSpec, PlanError};

    fn plan() -> PlanContent {
        PlanContent {
            milestones: vec![MilestoneSpec {
                id: "milestone".into(),
                title: "First release".into(),
                outcome: "Working feature".into(),
                task_ids: vec!["a".into(), "b".into()],
            }],
            epics: vec![EpicSpec {
                id: "e".into(),
                title: "Build".into(),
                outcome: "Usable app".into(),
            }],
            tasks: ["a", "b"]
                .into_iter()
                .map(|id| TaskSpec {
                    id: id.into(),
                    epic_id: Some("e".into()),
                    title: id.into(),
                    outcome: "done".into(),
                    weight: if id == "a" { 2 } else { 3 },
                    risk: "low".into(),
                    criteria: vec![CriterionSpec {
                        id: format!("c-{id}"),
                        assertion: "works".into(),
                        verifier: "owner".into(),
                        required_evidence: vec!["test-log".into()],
                    }],
                    verification_commands: vec!["cargo test".into()],
                    deliverables: Vec::new(),
                    context_links: Vec::new(),
                    estimate_band: "small".into(),
                    owner_decision_triggers: Vec::new(),
                })
                .collect(),
            dependencies: vec![DependencySpec {
                from_task_id: "b".into(),
                to_task_id: "a".into(),
                kind: DependencyKind::Blocks,
            }],
        }
    }

    fn setup() -> Store {
        let mut store = Store::open_in_memory().unwrap();
        store
            .create_project("p", "Project", "C:/p", "owner", "create")
            .unwrap();
        let brief = pipeline_domain::BriefContent {
            idea: "idea".into(),
            audience: "owner".into(),
            problem: "problem".into(),
            desired_outcome: "outcome".into(),
            ..Default::default()
        };
        store
            .save_brief_revision("brief", "p", 0, &brief, "owner", "brief")
            .unwrap();
        store
            .approve_brief_revision("p", 1, "brief-approval", "owner", "brief-approve")
            .unwrap();
        store
    }

    #[test]
    fn graph_and_completion_follow_approved_scope_and_evidence() {
        let mut store = setup();
        let mut content = plan();
        store
            .save_plan_revision("v1", "p", 0, &content, "owner", "save-1")
            .unwrap();
        assert_eq!(
            store
                .project_metrics("p")
                .unwrap()
                .verified_completion_basis_points,
            0
        );
        store
            .approve_plan_revision("p", 1, "approval-1", "owner", "approve-1")
            .unwrap();
        let materialized: i64 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM milestones WHERE project_id='p' AND scope_revision=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(materialized, 1);
        assert_eq!(
            store
                .list_runnable_tasks("p")
                .unwrap()
                .iter()
                .map(|t| t.logical_id.as_str())
                .collect::<Vec<_>>(),
            vec!["a"]
        );
        let a = store.list_active_tasks("p").unwrap().remove(0);
        assert!(matches!(
            store.submit_task_for_review("p", "b", 1, "owner", "premature"),
            Err(StoreError::TaskDependencyBlocked(_))
        ));
        store
            .submit_task_for_review("p", "a", a.revision, "owner", "review-a")
            .unwrap();
        assert!(matches!(
            store.accept_task("p", "a", 2, "owner", "early"),
            Err(StoreError::CriteriaNotVerified(_))
        ));
        let evidence = BTreeMap::from([("test-log".into(), "artifact://test-a".into())]);
        store
            .verify_criterion("p", "a", "c-a", 2, &evidence, "owner", "verify-a")
            .unwrap();
        store.accept_task("p", "a", 3, "owner", "accept-a").unwrap();
        assert_eq!(
            store
                .project_metrics("p")
                .unwrap()
                .verified_completion_basis_points,
            4000
        );
        assert_eq!(
            store
                .list_runnable_tasks("p")
                .unwrap()
                .iter()
                .map(|t| t.logical_id.as_str())
                .collect::<Vec<_>>(),
            vec!["b"]
        );
        content.tasks.push(TaskSpec {
            id: "c".into(),
            epic_id: None,
            title: "c".into(),
            outcome: "done".into(),
            weight: 5,
            risk: "low".into(),
            criteria: vec![pipeline_domain::CriterionSpec {
                id: "c-c".into(),
                assertion: "works".into(),
                verifier: "owner".into(),
                required_evidence: vec!["test-log".into()],
            }],
            verification_commands: Vec::new(),
            deliverables: Vec::new(),
            context_links: Vec::new(),
            estimate_band: String::new(),
            owner_decision_triggers: Vec::new(),
        });
        store
            .save_plan_revision("v2", "p", 1, &content, "owner", "save-2")
            .unwrap();
        assert_eq!(
            store
                .project_metrics("p")
                .unwrap()
                .verified_completion_basis_points,
            4000
        );
        store
            .approve_plan_revision("p", 2, "approval-2", "owner", "approve-2")
            .unwrap();
        assert_eq!(
            store
                .project_metrics("p")
                .unwrap()
                .verified_completion_basis_points,
            2000
        );
        assert_eq!(store.list_active_tasks("p").unwrap()[0].status, "accepted");
        assert_eq!(
            store.list_active_tasks("p").unwrap()[0].criteria[0].evidence,
            evidence
        );
        assert!(
            store
                .export_active_plan_json("p")
                .unwrap()
                .contains("\"scope_revision\": 2")
        );
    }

    #[test]
    fn invalid_graph_conflict_and_audit_failure_do_not_mutate_scope() {
        let mut store = setup();
        let mut content = plan();
        content.dependencies.push(pipeline_domain::DependencySpec {
            from_task_id: "a".into(),
            to_task_id: "b".into(),
            kind: DependencyKind::Blocks,
        });
        store
            .save_plan_revision("v1", "p", 0, &content, "owner", "save-1")
            .unwrap();
        assert!(matches!(
            store.approve_plan_revision("p", 1, "approval", "owner", "approve"),
            Err(StoreError::InvalidPlan(PlanError::Cycle))
        ));
        assert_eq!(store.plan_status("p").unwrap().active_scope_revision, 0);
        content.dependencies.pop();
        assert!(matches!(
            store.save_plan_revision("stale", "p", 0, &content, "owner", "stale"),
            Err(StoreError::PlanRevisionConflict { .. })
        ));
        store
            .save_plan_revision("v2", "p", 1, &content, "owner", "save-2")
            .unwrap();
        store
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_plan_activity BEFORE INSERT ON activity_events
            WHEN NEW.operation='plan.approve' BEGIN SELECT RAISE(ABORT,'test failure'); END;",
            )
            .unwrap();
        assert!(
            store
                .approve_plan_revision("p", 2, "approval", "owner", "approve")
                .is_err()
        );
        assert_eq!(store.plan_status("p").unwrap().active_scope_revision, 0);
        assert!(store.list_active_tasks("p").unwrap().is_empty());
    }

    #[test]
    fn plan_approval_waits_for_latest_brief() {
        let mut store = setup();
        let mut brief = pipeline_domain::BriefContent {
            idea: "idea".into(),
            audience: "owner".into(),
            problem: "problem".into(),
            desired_outcome: "outcome".into(),
            ..Default::default()
        };
        brief.scope = "expanded".into();
        store
            .save_brief_revision("brief-v2", "p", 1, &brief, "owner", "brief-2")
            .unwrap();
        store
            .save_plan_revision("plan", "p", 0, &plan(), "owner", "save")
            .unwrap();
        assert!(matches!(
            store.approve_plan_revision("p", 1, "approval", "owner", "approve"),
            Err(StoreError::PlanRequiresCurrentBrief)
        ));
        assert_eq!(store.plan_status("p").unwrap().active_scope_revision, 0);
    }

    #[test]
    fn weighted_completion_matches_every_accepted_subset() {
        for weights in [[1, 2, 3], [3, 5, 7], [2, 2, 9]] {
            for mask in 0..8_u8 {
                let mut store = setup();
                let mut content = plan();
                content.epics.clear();
                content.milestones.clear();
                content.dependencies.clear();
                content.tasks = (0..3)
                    .map(|i| {
                        let mut task = content.tasks[0].clone();
                        task.id = format!("task-{i}");
                        task.epic_id = None;
                        task.title = task.id.clone();
                        task.weight = weights[i];
                        task.criteria[0].id = format!("criterion-{i}");
                        task
                    })
                    .collect();
                store
                    .save_plan_revision("plan", "p", 0, &content, "owner", "save")
                    .unwrap();
                store
                    .approve_plan_revision("p", 1, "approval", "owner", "approve")
                    .unwrap();
                let mut accepted_weight = 0;
                for (i, weight) in weights.iter().enumerate() {
                    if mask & (1 << i) == 0 {
                        continue;
                    }
                    let id = format!("task-{i}");
                    let criterion = format!("criterion-{i}");
                    store
                        .submit_task_for_review("p", &id, 1, "owner", "review")
                        .unwrap();
                    store
                        .verify_criterion(
                            "p",
                            &id,
                            &criterion,
                            2,
                            &BTreeMap::from([("test-log".into(), format!("artifact://{i}"))]),
                            "owner",
                            "verify",
                        )
                        .unwrap();
                    store.accept_task("p", &id, 3, "owner", "accept").unwrap();
                    accepted_weight += weight;
                }
                let total: i64 = weights.iter().sum();
                assert_eq!(
                    store
                        .project_metrics("p")
                        .unwrap()
                        .verified_completion_basis_points,
                    (accepted_weight * 10_000 / total) as u16,
                    "weights {weights:?}, mask {mask}"
                );
            }
        }
    }
}
