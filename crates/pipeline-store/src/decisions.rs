use super::discovery::insert_event;
use super::{Result, Store, StoreError, validate_nonempty};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionInput {
    pub task_logical_id: Option<String>,
    pub question: String,
    pub alternatives: Vec<String>,
    pub recommendation: String,
    pub rationale: String,
    pub evidence: Vec<String>,
    pub impact: String,
    pub supersedes_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionRecord {
    pub id: String,
    pub project_id: String,
    pub task_logical_id: Option<String>,
    pub input: DecisionInput,
    pub selected_option: Option<String>,
    pub status: String,
    pub actor: String,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxItem {
    pub project_id: String,
    pub kind: String,
    pub subject_id: String,
    pub title: String,
    pub detail: String,
    pub priority: u8,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityRecord {
    pub id: i64,
    pub actor: String,
    pub operation: String,
    pub subject_type: String,
    pub subject_id: String,
    pub subject_revision: i64,
    pub before: Option<Value>,
    pub after: Option<Value>,
    pub correlation_id: String,
    pub created_at: String,
}

impl Store {
    pub fn list_decisions(&self, project_id: &str) -> Result<Vec<DecisionRecord>> {
        self.get_project(project_id)?
            .ok_or_else(|| StoreError::NotFound(project_id.to_owned()))?;
        let mut statement = self.connection.prepare(
            "SELECT d.id,d.project_id,t.logical_id,d.question,d.alternatives_json,d.recommendation,
                    d.rationale,d.evidence_json,d.impact,d.supersedes_id,d.selected_option,
                    d.status,d.actor,d.revision,d.created_at,d.updated_at
             FROM decisions d LEFT JOIN tasks t ON t.id=d.task_id
             WHERE d.project_id=?1 ORDER BY d.created_at DESC,d.rowid DESC",
        )?;
        let rows = statement.query_map([project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, String>(12)?,
                row.get::<_, i64>(13)?,
                row.get::<_, String>(14)?,
                row.get::<_, String>(15)?,
            ))
        })?;
        rows.map(|row| {
            let (
                id,
                project_id,
                task_logical_id,
                question,
                alternatives,
                recommendation,
                rationale,
                evidence,
                impact,
                supersedes_id,
                selected_option,
                status,
                actor,
                revision,
                created_at,
                updated_at,
            ) = row?;
            Ok(DecisionRecord {
                id,
                project_id,
                task_logical_id: task_logical_id.clone(),
                input: DecisionInput {
                    task_logical_id,
                    question,
                    alternatives: serde_json::from_str(&alternatives)?,
                    recommendation,
                    rationale: rationale.unwrap_or_default(),
                    evidence: serde_json::from_str(&evidence)?,
                    impact,
                    supersedes_id,
                },
                selected_option,
                status,
                actor,
                revision,
                created_at,
                updated_at,
            })
        })
        .collect()
    }

    pub fn create_decision(
        &mut self,
        id: &str,
        project_id: &str,
        input: &DecisionInput,
        actor: &str,
        correlation_id: &str,
    ) -> Result<DecisionRecord> {
        validate_decision(input)?;
        validate_nonempty("id", id)?;
        validate_nonempty("actor", actor)?;
        validate_nonempty("correlation_id", correlation_id)?;
        let tx = self.connection.transaction()?;
        let task_id = resolve_task(&tx, project_id, input.task_logical_id.as_deref())?;
        check_supersedes(&tx, project_id, input.supersedes_id.as_deref())?;
        tx.execute(
            "INSERT INTO decisions(id,project_id,task_id,question,alternatives_json,recommendation,
             rationale,evidence_json,impact,status,supersedes_id,actor)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'proposed',?10,?11)",
            params![
                id,
                project_id,
                task_id,
                input.question.trim(),
                serde_json::to_string(&input.alternatives)?,
                input.recommendation.trim(),
                input.rationale.trim(),
                serde_json::to_string(&input.evidence)?,
                input.impact.trim(),
                input.supersedes_id,
                actor
            ],
        )?;
        if let Some(prior) = &input.supersedes_id {
            let previous: (i64, String) = tx.query_row(
                "SELECT revision,status FROM decisions WHERE id=?1 AND project_id=?2",
                params![prior, project_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            if previous.1 == "superseded" {
                return Err(StoreError::InvalidSupersession);
            }
            tx.execute("UPDATE decisions SET status='superseded',revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?1", [prior])?;
            insert_event(
                &tx,
                project_id,
                actor,
                "decision.supersede",
                "decision",
                prior,
                previous.0 + 1,
                Some(json!({"status":previous.1})),
                Some(json!({"status":"superseded","superseded_by":id})),
                correlation_id,
            )?;
        }
        insert_event(
            &tx,
            project_id,
            actor,
            "decision.propose",
            "decision",
            id,
            1,
            None,
            Some(
                json!({"question":input.question,"impact":input.impact,"task":input.task_logical_id,
                "alternatives":input.alternatives,"recommendation":input.recommendation,
                "rationale":input.rationale,"evidence":input.evidence,"supersedes":input.supersedes_id}),
            ),
            correlation_id,
        )?;
        tx.commit()?;
        self.decision(project_id, id)
    }

    pub fn revise_decision(
        &mut self,
        project_id: &str,
        id: &str,
        expected_revision: i64,
        input: &DecisionInput,
        actor: &str,
        correlation_id: &str,
    ) -> Result<DecisionRecord> {
        validate_decision(input)?;
        validate_nonempty("actor", actor)?;
        validate_nonempty("correlation_id", correlation_id)?;
        let tx = self.connection.transaction()?;
        let (actual, status, before) = decision_state(&tx, project_id, id)?;
        check_revision(id, expected_revision, actual)?;
        if status != "proposed" {
            return Err(StoreError::DecisionResolved);
        }
        let task_id = resolve_task(&tx, project_id, input.task_logical_id.as_deref())?;
        check_supersedes(&tx, project_id, input.supersedes_id.as_deref())?;
        if input.supersedes_id.as_deref() == Some(id) {
            return Err(StoreError::DecisionResolved);
        }
        if serde_json::from_value::<Option<String>>(before["supersedes_id"].clone())?
            != input.supersedes_id
        {
            return Err(StoreError::InvalidSupersession);
        }
        tx.execute(
            "UPDATE decisions SET task_id=?1,question=?2,alternatives_json=?3,recommendation=?4,
             rationale=?5,evidence_json=?6,impact=?7,supersedes_id=?8,
             revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?9",
            params![
                task_id,
                input.question.trim(),
                serde_json::to_string(&input.alternatives)?,
                input.recommendation.trim(),
                input.rationale.trim(),
                serde_json::to_string(&input.evidence)?,
                input.impact.trim(),
                input.supersedes_id,
                id
            ],
        )?;
        insert_event(
            &tx,
            project_id,
            actor,
            "decision.revise",
            "decision",
            id,
            actual + 1,
            Some(before),
            Some(json!(input)),
            correlation_id,
        )?;
        tx.commit()?;
        self.decision(project_id, id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn resolve_decision(
        &mut self,
        project_id: &str,
        id: &str,
        expected_revision: i64,
        selected_option: Option<&str>,
        approve: bool,
        actor: &str,
        correlation_id: &str,
    ) -> Result<DecisionRecord> {
        if actor != "owner" {
            return Err(StoreError::DecisionRequiresOwner);
        }
        validate_nonempty("correlation_id", correlation_id)?;
        let tx = self.connection.transaction()?;
        let (actual, status, before) = decision_state(&tx, project_id, id)?;
        check_revision(id, expected_revision, actual)?;
        if status != "proposed" {
            return Err(StoreError::DecisionResolved);
        }
        let alternatives: Vec<String> = serde_json::from_value(before["alternatives"].clone())?;
        let selected = if approve {
            let selected = selected_option.ok_or(StoreError::InvalidDecisionAlternative)?;
            if !alternatives.iter().any(|option| option == selected) {
                return Err(StoreError::InvalidDecisionAlternative);
            }
            Some(selected)
        } else {
            None
        };
        let new_status = if approve { "approved" } else { "rejected" };
        tx.execute(
            "UPDATE decisions SET selected_option=?1,status=?2,revision=revision+1,
                    updated_at=CURRENT_TIMESTAMP WHERE id=?3",
            params![selected, new_status, id],
        )?;
        if approve {
            let approved_snapshot = json!({"proposal":before,"selected_option":selected,"status":new_status,"revision":actual+1});
            let digest = format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&approved_snapshot)?)
            );
            tx.execute("INSERT INTO approvals(id,project_id,subject_type,subject_id,subject_revision,
                        subject_hash,action_class,target,approver) VALUES (?1,?2,'decision',?3,?4,?5,
                        'decision.resolve',?3,?6)",
                params![format!("decision:{id}:{}",actual+1),project_id,id,actual+1,digest,actor])?;
        }
        insert_event(
            &tx,
            project_id,
            actor,
            if approve {
                "decision.approve"
            } else {
                "decision.reject"
            },
            "decision",
            id,
            actual + 1,
            Some(before),
            Some(json!({"status":new_status,"selected_option":selected})),
            correlation_id,
        )?;
        tx.commit()?;
        self.decision(project_id, id)
    }

    pub fn list_inbox(&self, project_id: &str) -> Result<Vec<InboxItem>> {
        let mut inbox = Vec::new();
        for decision in self.list_decisions(project_id)? {
            if decision.status != "proposed" {
                continue;
            }
            let priority = match decision.input.impact.as_str() {
                "blocking" => 0,
                "high" => 1,
                "medium" => 2,
                _ => 3,
            };
            inbox.push(InboxItem {
                project_id: project_id.to_owned(),
                kind: "decision".into(),
                subject_id: decision.id,
                title: decision.input.question,
                detail: format!(
                    "Recommended: {} · impact: {}",
                    decision.input.recommendation, decision.input.impact
                ),
                priority,
                revision: decision.revision,
            });
        }
        let brief = self.brief_status(project_id)?;
        if brief.latest_revision > brief.approved_revision {
            inbox.push(InboxItem {
                project_id: project_id.into(),
                kind: "brief".into(),
                subject_id: project_id.into(),
                title: format!("Review brief revision {}", brief.latest_revision),
                detail: "Approve or revise the latest brief".into(),
                priority: 2,
                revision: brief.latest_revision,
            });
        }
        let plans = self.list_plan_revisions(project_id)?;
        if let Some(plan) = plans.last().filter(|plan| plan.status == "draft") {
            inbox.push(InboxItem {
                project_id: project_id.into(),
                kind: "plan".into(),
                subject_id: plan.id.clone(),
                title: format!("Review task plan revision {}", plan.revision),
                detail: "Approve or revise the latest task plan".into(),
                priority: 2,
                revision: plan.revision,
            });
        }
        for task in self
            .list_active_tasks(project_id)?
            .into_iter()
            .filter(|task| task.status == "blocked" || task.status == "review")
        {
            let blocked = task.status == "blocked";
            inbox.push(InboxItem {
                project_id: project_id.into(),
                kind: "task".into(),
                subject_id: task.logical_id,
                title: format!(
                    "{} task: {}",
                    if blocked { "Blocked" } else { "Review" },
                    task.title
                ),
                detail: if blocked {
                    task.unresolved_blockers.join(", ")
                } else {
                    "Check criteria and evidence".into()
                },
                priority: if blocked { 0 } else { 2 },
                revision: task.revision,
            });
        }
        inbox.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.title.cmp(&b.title))
        });
        Ok(inbox)
    }

    pub fn list_activity(&self, project_id: &str, limit: usize) -> Result<Vec<ActivityRecord>> {
        self.get_project(project_id)?
            .ok_or_else(|| StoreError::NotFound(project_id.into()))?;
        let mut statement = self.connection.prepare(
            "SELECT id,actor,operation,subject_type,subject_id,subject_revision,before_json,after_json,
                    correlation_id,created_at FROM activity_events WHERE project_id=?1 ORDER BY id DESC LIMIT ?2")?;
        let rows = statement.query_map(params![project_id, limit.min(500) as i64], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })?;
        rows.map(|row| {
            let (
                id,
                actor,
                operation,
                subject_type,
                subject_id,
                subject_revision,
                before,
                after,
                correlation_id,
                created_at,
            ) = row?;
            Ok(ActivityRecord {
                id,
                actor,
                operation,
                subject_type,
                subject_id,
                subject_revision,
                before: before.map(|text| serde_json::from_str(&text)).transpose()?,
                after: after.map(|text| serde_json::from_str(&text)).transpose()?,
                correlation_id,
                created_at,
            })
        })
        .collect()
    }

    fn decision(&self, project_id: &str, id: &str) -> Result<DecisionRecord> {
        self.list_decisions(project_id)?
            .into_iter()
            .find(|item| item.id == id)
            .ok_or_else(|| StoreError::DecisionNotFound(id.into()))
    }
}

fn validate_decision(input: &DecisionInput) -> Result<()> {
    validate_nonempty("question", &input.question)?;
    validate_nonempty("impact", &input.impact)?;
    validate_nonempty("rationale", &input.rationale)?;
    if !matches!(
        input.impact.as_str(),
        "low" | "medium" | "high" | "blocking"
    ) {
        return Err(StoreError::InvalidDecisionImpact);
    }
    if input.alternatives.len() < 2
        || input
            .alternatives
            .iter()
            .any(|value| value.trim().is_empty())
        || input.alternatives.iter().any(|value| {
            input
                .alternatives
                .iter()
                .filter(|other| *other == value)
                .count()
                > 1
        })
        || !input.alternatives.contains(&input.recommendation)
    {
        return Err(StoreError::InvalidDecisionAlternative);
    }
    Ok(())
}

fn resolve_task(
    connection: &Connection,
    project_id: &str,
    logical_id: Option<&str>,
) -> Result<Option<String>> {
    if !connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
        [project_id],
        |row| row.get::<_, bool>(0),
    )? {
        return Err(StoreError::NotFound(project_id.into()));
    }
    let Some(logical_id) = logical_id.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    connection.query_row("SELECT id FROM tasks WHERE project_id=?1 AND logical_id=?2 AND scope_revision=(SELECT active_scope_revision FROM projects WHERE id=?1)",
        params![project_id,logical_id],|row|row.get(0)).optional()?.map(Some)
        .ok_or_else(||StoreError::TaskNotInActiveScope(logical_id.into()))
}

fn check_supersedes(connection: &Connection, project_id: &str, id: Option<&str>) -> Result<()> {
    if let Some(id) = id {
        let found: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM decisions WHERE id=?1 AND project_id=?2)",
            params![id, project_id],
            |row| row.get(0),
        )?;
        if !found {
            return Err(StoreError::DecisionNotFound(id.into()));
        }
    }
    Ok(())
}

fn decision_state(
    connection: &Connection,
    project_id: &str,
    id: &str,
) -> Result<(i64, String, Value)> {
    type StateRow = (
        i64,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
    );
    let row: Option<StateRow> = connection.query_row(
        "SELECT revision,status,question,alternatives_json,recommendation,rationale,evidence_json,impact,supersedes_id
         FROM decisions WHERE id=?1 AND project_id=?2",params![id,project_id],|row|Ok((
            row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?))).optional()?;
    let (
        revision,
        status,
        question,
        alternatives,
        recommendation,
        rationale,
        evidence,
        impact,
        supersedes_id,
    ) = row.ok_or_else(|| StoreError::DecisionNotFound(id.into()))?;
    let task_id: Option<String> = connection.query_row(
        "SELECT task_id FROM decisions WHERE id=?1 AND project_id=?2",
        params![id, project_id],
        |row| row.get(0),
    )?;
    Ok((
        revision,
        status.clone(),
        json!({"revision":revision,"status":status,"question":question,
        "alternatives":serde_json::from_str::<Value>(&alternatives)?,"recommendation":recommendation,
        "rationale":rationale,"evidence":serde_json::from_str::<Value>(&evidence)?,"impact":impact,
        "supersedes_id":supersedes_id,"task_id":task_id}),
    ))
}

fn check_revision(id: &str, expected: i64, actual: i64) -> Result<()> {
    if expected == actual {
        Ok(())
    } else {
        Err(StoreError::DecisionRevisionConflict {
            id: id.into(),
            expected,
            actual,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Store {
        let mut store = Store::open_in_memory().unwrap();
        store
            .create_project("p", "Project", "C:/p", "owner", "create")
            .unwrap();
        store
    }

    fn input(impact: &str) -> DecisionInput {
        DecisionInput {
            question: "Which approach?".into(),
            alternatives: vec!["A".into(), "B".into()],
            recommendation: "A".into(),
            rationale: "Lower risk".into(),
            evidence: vec!["artifact://comparison".into()],
            impact: impact.into(),
            ..Default::default()
        }
    }

    #[test]
    fn decision_audit_and_revision_bound_owner_review() {
        let mut store = setup();
        let baseline = store.activity_count("p").unwrap();
        let mut proposal = store
            .create_decision("d", "p", &input("high"), "agent", "propose")
            .unwrap();
        assert_eq!(proposal.status, "proposed");
        assert_eq!(store.list_inbox("p").unwrap()[0].subject_id, "d");
        assert_eq!(store.activity_count("p").unwrap(), baseline + 1);
        let mut revised = input("blocking");
        revised.rationale = "Measured latency".into();
        proposal = store
            .revise_decision("p", "d", 1, &revised, "owner", "edit")
            .unwrap();
        assert_eq!(proposal.revision, 2);
        assert_eq!(proposal.actor, "agent");
        assert!(matches!(
            store.resolve_decision("p", "d", 1, Some("A"), true, "owner", "stale"),
            Err(StoreError::DecisionRevisionConflict { .. })
        ));
        assert!(matches!(
            store.resolve_decision("p", "d", 2, Some("A"), true, "agent", "not-owner"),
            Err(StoreError::DecisionRequiresOwner)
        ));
        assert!(matches!(
            store.resolve_decision("p", "d", 2, Some("C"), true, "owner", "invalid"),
            Err(StoreError::InvalidDecisionAlternative)
        ));
        assert_eq!(store.activity_count("p").unwrap(), baseline + 2);
        let approved = store
            .resolve_decision("p", "d", 2, Some("B"), true, "owner", "approve")
            .unwrap();
        assert_eq!(approved.status, "approved");
        assert_eq!(approved.selected_option.as_deref(), Some("B"));
        assert!(
            store
                .list_inbox("p")
                .unwrap()
                .iter()
                .all(|item| item.subject_id != "d")
        );
        let approval: (i64, String) = store
            .connection
            .query_row(
                "SELECT subject_revision,subject_hash FROM approvals WHERE subject_id='d'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(approval.0, 3);
        assert_eq!(approval.1.len(), 64);
        assert_eq!(
            store.list_activity("p", 10).unwrap()[0].operation,
            "decision.approve"
        );
        assert!(matches!(
            store.revise_decision("p", "d", 3, &revised, "owner", "late"),
            Err(StoreError::DecisionResolved)
        ));
    }

    #[test]
    fn inbox_priority_project_scope_rejection_and_transactional_audit() {
        let mut store = setup();
        store
            .create_project("other", "Other", "C:/other", "owner", "create-other")
            .unwrap();
        store
            .create_decision("low", "p", &input("low"), "agent", "low")
            .unwrap();
        store
            .create_decision("block", "p", &input("blocking"), "agent", "block")
            .unwrap();
        assert_eq!(store.list_inbox("p").unwrap()[0].subject_id, "block");
        assert!(matches!(
            store.resolve_decision("other", "block", 1, Some("A"), true, "owner", "cross"),
            Err(StoreError::DecisionNotFound(_))
        ));
        let baseline = store.activity_count("p").unwrap();
        store.connection.execute_batch("CREATE TRIGGER fail_decision_audit BEFORE INSERT ON activity_events
            WHEN NEW.operation='decision.reject' BEGIN SELECT RAISE(ABORT,'audit unavailable'); END;").unwrap();
        assert!(
            store
                .resolve_decision("p", "low", 1, None, false, "owner", "reject")
                .is_err()
        );
        assert_eq!(store.activity_count("p").unwrap(), baseline);
        assert_eq!(store.decision("p", "low").unwrap().status, "proposed");
    }

    #[test]
    fn rejection_and_supersession_keep_prior_versions_and_audit() {
        let mut store = setup();
        store
            .create_decision("first", "p", &input("medium"), "agent", "first")
            .unwrap();
        let rejected = store
            .resolve_decision("p", "first", 1, None, false, "owner", "reject")
            .unwrap();
        assert_eq!(rejected.status, "rejected");
        assert!(matches!(
            store.resolve_decision("p", "first", 2, Some("A"), true, "owner", "retry"),
            Err(StoreError::DecisionResolved)
        ));
        let mut replacement = input("high");
        replacement.supersedes_id = Some("first".into());
        let next = store
            .create_decision("next", "p", &replacement, "agent", "replace")
            .unwrap();
        assert_eq!(next.input.supersedes_id.as_deref(), Some("first"));
        assert_eq!(store.decision("p", "first").unwrap().status, "superseded");
        let operations: Vec<_> = store
            .list_activity("p", 5)
            .unwrap()
            .into_iter()
            .map(|event| event.operation)
            .collect();
        assert!(operations.contains(&"decision.supersede".into()));
        assert!(operations.contains(&"decision.reject".into()));
        assert!(matches!(
            store.create_decision("third", "p", &replacement, "agent", "repeat"),
            Err(StoreError::InvalidSupersession)
        ));
    }
}
