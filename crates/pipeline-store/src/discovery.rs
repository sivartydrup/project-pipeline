use super::{Result, Store, StoreError, validate_nonempty};
use pipeline_domain::BriefContent;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::json;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BriefRecord {
    pub id: String,
    pub project_id: String,
    pub revision: i64,
    pub status: String,
    pub content: BriefContent,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BriefStatus {
    pub latest_revision: i64,
    pub approved_revision: i64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ResearchInput {
    pub claim: String,
    pub summary: String,
    pub relevance: String,
    pub source_uri: String,
    pub accessed_at: String,
    pub confidence: String,
    pub is_hypothesis: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchRecord {
    pub id: String,
    pub project_id: String,
    pub brief_revision_id: Option<String>,
    pub input: ResearchInput,
    pub created_at: String,
}

impl Store {
    pub fn brief_status(&self, project_id: &str) -> Result<BriefStatus> {
        let approved: Option<i64> = self
            .connection
            .query_row(
                "SELECT approved_brief_revision FROM projects WHERE id=?1",
                [project_id],
                |row| row.get(0),
            )
            .optional()?;
        let approved_revision =
            approved.ok_or_else(|| StoreError::NotFound(project_id.to_owned()))?;
        let latest_revision: i64 = self.connection.query_row(
            "SELECT COALESCE(MAX(revision),0) FROM brief_revisions WHERE project_id=?1",
            [project_id],
            |row| row.get(0),
        )?;
        Ok(BriefStatus {
            latest_revision,
            approved_revision,
        })
    }

    pub fn list_brief_revisions(&self, project_id: &str) -> Result<Vec<BriefRecord>> {
        self.brief_status(project_id)?;
        let mut statement = self.connection.prepare(
            "SELECT id, revision, status, content_json, created_at FROM brief_revisions WHERE project_id=?1 ORDER BY revision",
        )?;
        let rows = statement.query_map([project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        rows.map(|row| {
            let (id, revision, status, content_json, created_at) = row?;
            Ok(BriefRecord {
                id,
                project_id: project_id.to_owned(),
                revision,
                status,
                content: serde_json::from_str(&content_json)?,
                created_at,
            })
        })
        .collect()
    }

    pub fn save_brief_revision(
        &mut self,
        id: &str,
        project_id: &str,
        expected_latest_revision: i64,
        content: &BriefContent,
        actor: &str,
        correlation_id: &str,
    ) -> Result<BriefRecord> {
        for (field, value) in [
            ("id", id),
            ("actor", actor),
            ("correlation_id", correlation_id),
        ] {
            validate_nonempty(field, value)?;
        }
        let transaction = self.connection.transaction()?;
        let project_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
            [project_id],
            |row| row.get(0),
        )?;
        if !project_exists {
            return Err(StoreError::NotFound(project_id.to_owned()));
        }
        let actual: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(revision),0) FROM brief_revisions WHERE project_id=?1",
            [project_id],
            |row| row.get(0),
        )?;
        if actual != expected_latest_revision {
            return Err(StoreError::BriefRevisionConflict {
                id: project_id.to_owned(),
                expected: expected_latest_revision,
                actual,
            });
        }
        let revision = actual + 1;
        let content_json = serde_json::to_string(content)?;
        transaction.execute(
            "INSERT INTO brief_revisions(id,project_id,revision,status,content_json) VALUES (?1,?2,?3,'draft',?4)",
            params![id, project_id, revision, content_json],
        )?;
        transaction.execute(
            "UPDATE projects SET stage=CASE WHEN approved_brief_revision=0 THEN 'discovery' ELSE stage END,
                health='needs_input', revision=revision+1, updated_at=CURRENT_TIMESTAMP WHERE id=?1",
            [project_id],
        )?;
        insert_event(
            &transaction,
            project_id,
            actor,
            "brief.revise",
            "brief",
            id,
            revision,
            Some(json!({"previous_revision": actual})),
            Some(json!({"revision": revision, "content_hash": content_hash(&content_json)})),
            correlation_id,
        )?;
        let created_at: String = transaction.query_row(
            "SELECT created_at FROM brief_revisions WHERE id=?1",
            [id],
            |row| row.get(0),
        )?;
        transaction.commit()?;
        Ok(BriefRecord {
            id: id.to_owned(),
            project_id: project_id.to_owned(),
            revision,
            status: "draft".to_owned(),
            content: content.clone(),
            created_at,
        })
    }

    pub fn approve_brief_revision(
        &mut self,
        project_id: &str,
        expected_latest_revision: i64,
        approval_id: &str,
        actor: &str,
        correlation_id: &str,
    ) -> Result<BriefRecord> {
        if actor != "owner" {
            return Err(StoreError::ApprovalRequiresOwner);
        }
        validate_nonempty("approval_id", approval_id)?;
        validate_nonempty("correlation_id", correlation_id)?;
        let transaction = self.connection.transaction()?;
        let approved: Option<i64> = transaction
            .query_row(
                "SELECT approved_brief_revision FROM projects WHERE id=?1",
                [project_id],
                |row| row.get(0),
            )
            .optional()?;
        if approved.is_none() {
            return Err(StoreError::NotFound(project_id.to_owned()));
        }
        let actual: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(revision),0) FROM brief_revisions WHERE project_id=?1",
            [project_id],
            |row| row.get(0),
        )?;
        if actual != expected_latest_revision || actual == 0 {
            return Err(StoreError::BriefRevisionConflict {
                id: project_id.to_owned(),
                expected: expected_latest_revision,
                actual,
            });
        }
        let (brief_id, status, content_json): (String,String,String) = transaction.query_row(
            "SELECT id,status,content_json FROM brief_revisions WHERE project_id=?1 AND revision=?2",
            params![project_id, actual], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        )?;
        if status == "approved" {
            return Err(StoreError::BriefAlreadyApproved);
        }
        let content: BriefContent = serde_json::from_str(&content_json)?;
        if !content.has_required_intake() {
            return Err(StoreError::IncompleteBrief);
        }
        let hash = content_hash(&content_json);
        transaction.execute(
            "UPDATE brief_revisions SET status='approved' WHERE id=?1",
            [&brief_id],
        )?;
        transaction.execute(
            "INSERT INTO approvals(id,project_id,subject_type,subject_id,subject_revision,subject_hash,
                action_class,target,approver) VALUES (?1,?2,'brief',?3,?4,?5,'approve_brief',?2,?6)",
            params![approval_id,project_id,brief_id,actual,hash,actor],
        )?;
        transaction.execute(
            "UPDATE projects SET approved_brief_revision=?1,
                stage=CASE WHEN stage IN ('idea','discovery','planning') THEN 'planning' ELSE stage END,
                revision=revision+1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![actual,project_id],
        )?;
        insert_event(
            &transaction,
            project_id,
            actor,
            "brief.approve",
            "brief",
            &brief_id,
            actual,
            Some(json!({"status": "draft"})),
            Some(json!({"status": "approved", "content_hash": hash, "approval_id": approval_id})),
            correlation_id,
        )?;
        let created_at: String = transaction.query_row(
            "SELECT created_at FROM brief_revisions WHERE id=?1",
            [&brief_id],
            |row| row.get(0),
        )?;
        transaction.commit()?;
        Ok(BriefRecord {
            id: brief_id,
            project_id: project_id.to_owned(),
            revision: actual,
            status: "approved".to_owned(),
            content,
            created_at,
        })
    }

    pub fn add_research_finding(
        &mut self,
        id: &str,
        project_id: &str,
        brief_revision_id: Option<&str>,
        input: &ResearchInput,
        actor: &str,
        correlation_id: &str,
    ) -> Result<ResearchRecord> {
        for (field, value) in [
            ("id", id),
            ("claim", &input.claim),
            ("summary", &input.summary),
            ("relevance", &input.relevance),
            ("confidence", &input.confidence),
            ("actor", actor),
            ("correlation_id", correlation_id),
        ] {
            validate_nonempty(field, value)?;
        }
        if !input.is_hypothesis
            && (input.source_uri.trim().is_empty() || input.accessed_at.trim().is_empty())
        {
            return Err(StoreError::MissingResearchSource);
        }
        let transaction = self.connection.transaction()?;
        let project_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
            [project_id],
            |row| row.get(0),
        )?;
        if !project_exists {
            return Err(StoreError::NotFound(project_id.to_owned()));
        }
        if let Some(brief_id) = brief_revision_id {
            let linked: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM brief_revisions WHERE id=?1 AND project_id=?2)",
                params![brief_id, project_id],
                |row| row.get(0),
            )?;
            if !linked {
                return Err(StoreError::NotFound(brief_id.to_owned()));
            }
        }
        transaction.execute(
            "INSERT INTO research_findings(id,project_id,brief_revision_id,claim,source_uri,accessed_at,
                confidence,is_hypothesis,summary,relevance)
                VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![id,project_id,brief_revision_id,input.claim.trim(),
                if input.source_uri.trim().is_empty() {None} else {Some(input.source_uri.trim())},
                if input.accessed_at.trim().is_empty() {None} else {Some(input.accessed_at.trim())},
                input.confidence.trim(),input.is_hypothesis,input.summary.trim(),input.relevance.trim()],
        )?;
        insert_event(
            &transaction,
            project_id,
            actor,
            "research.record",
            "research",
            id,
            1,
            None,
            Some(
                json!({"kind": if input.is_hypothesis {"hypothesis"} else {"sourced_fact"},
                "source_uri": input.source_uri}),
            ),
            correlation_id,
        )?;
        let created_at: String = transaction.query_row(
            "SELECT created_at FROM research_findings WHERE id=?1",
            [id],
            |row| row.get(0),
        )?;
        transaction.commit()?;
        Ok(ResearchRecord {
            id: id.to_owned(),
            project_id: project_id.to_owned(),
            brief_revision_id: brief_revision_id.map(str::to_owned),
            input: input.clone(),
            created_at,
        })
    }

    pub fn list_research_findings(&self, project_id: &str) -> Result<Vec<ResearchRecord>> {
        self.brief_status(project_id)?;
        let mut statement = self.connection.prepare(
            "SELECT id,brief_revision_id,claim,summary,relevance,source_uri,accessed_at,
                confidence,is_hypothesis,created_at FROM research_findings
                WHERE project_id=?1 ORDER BY created_at,id",
        )?;
        let rows = statement.query_map([project_id], |row| {
            Ok(ResearchRecord {
                id: row.get(0)?,
                project_id: project_id.to_owned(),
                brief_revision_id: row.get(1)?,
                input: ResearchInput {
                    claim: row.get(2)?,
                    summary: row.get(3)?,
                    relevance: row.get(4)?,
                    source_uri: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                    accessed_at: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
                    confidence: row.get(7)?,
                    is_hypothesis: row.get(8)?,
                },
                created_at: row.get(9)?,
            })
        })?;
        rows.map(|row| row.map_err(StoreError::from)).collect()
    }
}

fn content_hash(content_json: &str) -> String {
    format!("{:x}", Sha256::digest(content_json.as_bytes()))
}

#[allow(clippy::too_many_arguments)]
fn insert_event(
    connection: &Connection,
    project_id: &str,
    actor: &str,
    operation: &str,
    subject_type: &str,
    subject_id: &str,
    revision: i64,
    before: Option<serde_json::Value>,
    after: Option<serde_json::Value>,
    correlation_id: &str,
) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO activity_events(project_id,actor,operation,subject_type,subject_id,
            subject_revision,before_json,after_json,correlation_id)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            project_id,
            actor,
            operation,
            subject_type,
            subject_id,
            revision,
            before.map(|value| value.to_string()),
            after.map(|value| value.to_string()),
            correlation_id
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_brief() -> BriefContent {
        BriefContent {
            idea: "Tool for makers".to_owned(),
            audience: "Solo makers".to_owned(),
            problem: "Scattered decisions".to_owned(),
            desired_outcome: "One clear plan".to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn approval_is_revision_bound_hashed_and_atomic_with_audit() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .create_project("p1", "First", "C:/first", "owner", "create")
            .unwrap();
        let content = complete_brief();
        store
            .save_brief_revision("b1", "p1", 0, &content, "owner", "save")
            .unwrap();
        assert!(matches!(
            store.approve_brief_revision("p1", 1, "a0", "agent", "deny"),
            Err(StoreError::ApprovalRequiresOwner)
        ));
        store
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_brief_approval BEFORE INSERT ON activity_events
             WHEN NEW.operation='brief.approve' BEGIN SELECT RAISE(ABORT, 'test failure'); END;",
            )
            .unwrap();
        assert!(
            store
                .approve_brief_revision("p1", 1, "a1", "owner", "approve")
                .is_err()
        );
        assert_eq!(store.brief_status("p1").unwrap().approved_revision, 0);
        assert_eq!(store.list_brief_revisions("p1").unwrap()[0].status, "draft");
        let count: i64 = store
            .connection
            .query_row("SELECT count(*) FROM approvals", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
        store
            .connection
            .execute_batch("DROP TRIGGER reject_brief_approval")
            .unwrap();
        store
            .approve_brief_revision("p1", 1, "a1", "owner", "approve")
            .unwrap();
        let (subject_id, revision, hash): (String, i64, String) = store
            .connection
            .query_row(
                "SELECT subject_id,subject_revision,subject_hash FROM approvals WHERE id='a1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(subject_id, "b1");
        assert_eq!(revision, 1);
        assert_eq!(
            hash,
            content_hash(&serde_json::to_string(&content).unwrap())
        );
        assert_eq!(store.activity_count("p1").unwrap(), 3);
    }

    #[test]
    fn incomplete_brief_cannot_be_approved() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .create_project("p1", "First", "C:/first", "owner", "create")
            .unwrap();
        store
            .save_brief_revision("b1", "p1", 0, &BriefContent::default(), "owner", "save")
            .unwrap();
        assert!(matches!(
            store.approve_brief_revision("p1", 1, "a1", "owner", "approve"),
            Err(StoreError::IncompleteBrief)
        ));
        assert_eq!(store.brief_status("p1").unwrap().approved_revision, 0);
    }
}
