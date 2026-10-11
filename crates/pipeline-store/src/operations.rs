use super::discovery::insert_event;
use super::{Result, Store, StoreError, validate_nonempty};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const KINDS: &[&str] = &[
    "positioning",
    "launch_copy",
    "launch_asset",
    "channel_plan",
    "metric",
    "feedback",
    "bug",
    "incident",
    "maintenance",
];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleInput {
    pub kind: String,
    pub title: String,
    pub body: String,
    pub channel: String,
    pub metric_value: String,
    pub metric_unit: String,
    pub status: String,
    pub severity: String,
    pub publication_target: String,
    pub task_logical_id: Option<String>,
    pub artifact_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleRecord {
    pub id: String,
    pub project_id: String,
    pub input: LifecycleInput,
    pub publication_approval_id: Option<String>,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LifecycleBlockers {
    pub release: u32,
    pub operations: u32,
}

impl Store {
    pub fn list_lifecycle(&self, project_id: &str) -> Result<Vec<LifecycleRecord>> {
        self.get_project(project_id)?
            .ok_or_else(|| StoreError::NotFound(project_id.into()))?;
        let mut q=self.connection.prepare("SELECT id FROM lifecycle_entries WHERE project_id=?1 ORDER BY created_at DESC,rowid DESC")?;
        q.query_map([project_id], |r| r.get::<_, String>(0))?
            .map(|id| record_from(&self.connection, project_id, &id?))
            .collect()
    }

    pub fn lifecycle(&self, project_id: &str, id: &str) -> Result<LifecycleRecord> {
        record_from(&self.connection, project_id, id)
    }

    pub fn create_lifecycle(
        &mut self,
        project_id: &str,
        input: &LifecycleInput,
        actor: &str,
        correlation_id: &str,
    ) -> Result<LifecycleRecord> {
        validate_input(input)?;
        validate_nonempty("actor", actor)?;
        validate_nonempty("correlation_id", correlation_id)?;
        let tx = self.connection.transaction()?;
        let task_id = resolve_task(&tx, project_id, input.task_logical_id.as_deref())?;
        validate_artifact(&tx, project_id, input.artifact_id.as_deref())?;
        let id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO lifecycle_entries(id,project_id,task_id,artifact_id,kind,title,body,
            channel,metric_value,metric_unit,status,severity,publication_target)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                id,
                project_id,
                task_id,
                input.artifact_id,
                input.kind,
                input.title.trim(),
                input.body,
                input.channel,
                input.metric_value,
                input.metric_unit,
                input.status,
                input.severity,
                input.publication_target
            ],
        )?;
        insert_event(
            &tx,
            project_id,
            actor,
            "lifecycle.create",
            "lifecycle",
            &id,
            1,
            None,
            Some(json!(input)),
            correlation_id,
        )?;
        tx.commit()?;
        self.lifecycle(project_id, &id)
    }

    pub fn revise_lifecycle(
        &mut self,
        project_id: &str,
        id: &str,
        expected_revision: i64,
        input: &LifecycleInput,
        actor: &str,
        correlation_id: &str,
    ) -> Result<LifecycleRecord> {
        validate_input(input)?;
        let tx = self.connection.transaction()?;
        let before = record_from(&tx, project_id, id)?;
        check_revision(&before, expected_revision)?;
        if before.input.status == "published" {
            return Err(StoreError::InvalidLifecycle(
                "published record is immutable".into(),
            ));
        }
        let task_id = resolve_task(&tx, project_id, input.task_logical_id.as_deref())?;
        validate_artifact(&tx, project_id, input.artifact_id.as_deref())?;
        tx.execute(
            "UPDATE lifecycle_entries SET task_id=?1,artifact_id=?2,kind=?3,title=?4,body=?5,
            channel=?6,metric_value=?7,metric_unit=?8,status=?9,severity=?10,publication_target=?11,
            revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?12",
            params![
                task_id,
                input.artifact_id,
                input.kind,
                input.title.trim(),
                input.body,
                input.channel,
                input.metric_value,
                input.metric_unit,
                input.status,
                input.severity,
                input.publication_target,
                id
            ],
        )?;
        insert_event(
            &tx,
            project_id,
            actor,
            "lifecycle.revise",
            "lifecycle",
            id,
            expected_revision + 1,
            Some(json!(before.input)),
            Some(json!(input)),
            correlation_id,
        )?;
        tx.commit()?;
        self.lifecycle(project_id, id)
    }

    pub fn approve_publication(
        &mut self,
        project_id: &str,
        id: &str,
        expected_revision: i64,
        actor: &str,
        correlation_id: &str,
    ) -> Result<String> {
        if actor != "owner" {
            return Err(StoreError::ApprovalRequiresOwner);
        }
        let tx = self.connection.transaction()?;
        let record = record_from(&tx, project_id, id)?;
        check_revision(&record, expected_revision)?;
        if !matches!(
            record.input.kind.as_str(),
            "positioning" | "launch_copy" | "launch_asset" | "channel_plan"
        ) || record.input.publication_target.trim().is_empty()
            || record.input.status == "published"
        {
            return Err(StoreError::InvalidLifecycle(
                "publishable content and exact target required".into(),
            ));
        }
        let approval_id = Uuid::new_v4().to_string();
        let hash = content_hash(&record)?;
        tx.execute(
            "INSERT INTO approvals(id,project_id,subject_type,subject_id,subject_revision,
            subject_hash,action_class,target,approver)
            VALUES (?1,?2,'lifecycle',?3,?4,?5,'external.publish',?6,?7)",
            params![
                approval_id,
                project_id,
                id,
                expected_revision,
                hash,
                record.input.publication_target,
                actor
            ],
        )?;
        insert_event(
            &tx,
            project_id,
            actor,
            "lifecycle.publication_approve",
            "lifecycle",
            id,
            expected_revision,
            None,
            Some(
                json!({"approval_id":approval_id,"target":record.input.publication_target,"content_sha256":hash}),
            ),
            correlation_id,
        )?;
        tx.commit()?;
        Ok(approval_id)
    }

    pub fn record_manual_publication(
        &mut self,
        project_id: &str,
        id: &str,
        expected_revision: i64,
        approval_id: &str,
        actor: &str,
        correlation_id: &str,
    ) -> Result<LifecycleRecord> {
        let tx = self.connection.transaction()?;
        let record = record_from(&tx, project_id, id)?;
        check_revision(&record, expected_revision)?;
        if record.input.status == "published" {
            return Err(StoreError::InvalidLifecycle("already published".into()));
        }
        let hash = content_hash(&record)?;
        let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM approvals WHERE id=?1 AND project_id=?2
            AND subject_type='lifecycle' AND subject_id=?3 AND subject_revision=?4 AND subject_hash=?5
            AND action_class='external.publish' AND target=?6 AND revoked_at IS NULL)",
            params![approval_id,project_id,id,expected_revision,hash,record.input.publication_target],|r|r.get(0))?;
        let used: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM lifecycle_entries WHERE publication_approval_id=?1)",
            [approval_id],
            |r| r.get(0),
        )?;
        if !valid || used {
            return Err(StoreError::InvalidLifecycle(
                "unused exact-content publication approval required".into(),
            ));
        }
        tx.execute(
            "UPDATE lifecycle_entries SET status='published',publication_approval_id=?1,
            revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![approval_id, id],
        )?;
        insert_event(
            &tx,
            project_id,
            actor,
            "lifecycle.manual_publication",
            "lifecycle",
            id,
            expected_revision + 1,
            None,
            Some(json!({"approval_id":approval_id,"target":record.input.publication_target})),
            correlation_id,
        )?;
        tx.commit()?;
        self.lifecycle(project_id, id)
    }

    pub fn lifecycle_blockers(&self, project_id: &str) -> Result<LifecycleBlockers> {
        self.get_project(project_id)?
            .ok_or_else(|| StoreError::NotFound(project_id.into()))?;
        let operations: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM lifecycle_entries WHERE project_id=?1
            AND status IN ('open','in_progress','blocked') AND
            (status='blocked' OR (kind IN ('bug','incident') AND severity IN ('high','critical')))",
            [project_id],
            |r| r.get(0),
        )?;
        let release:i64=self.connection.query_row("SELECT CASE WHEN EXISTS(
            SELECT 1 FROM release_candidates rc WHERE rc.project_id=?1 AND rc.id=(
                SELECT id FROM release_candidates WHERE project_id=?1 ORDER BY created_at DESC,rowid DESC LIMIT 1)
            AND (rc.status='deployment_failed' OR EXISTS(SELECT 1 FROM release_checks c
                WHERE c.candidate_id=rc.id AND c.required=1 AND c.status='failed')
                OR EXISTS(SELECT 1 FROM release_deployments d WHERE d.candidate_id=rc.id
                    AND (d.result='failed' OR d.smoke_result='failed'))))
            THEN 1 ELSE 0 END",[project_id],|r|r.get(0))?;
        Ok(LifecycleBlockers {
            release: release as u32,
            operations: operations as u32,
        })
    }
}

fn validate_input(i: &LifecycleInput) -> Result<()> {
    if !KINDS.contains(&i.kind.as_str()) {
        return Err(StoreError::InvalidLifecycle("invalid kind".into()));
    }
    validate_nonempty("title", &i.title)?;
    if !matches!(
        i.status.as_str(),
        "draft" | "open" | "in_progress" | "blocked" | "resolved"
    ) {
        return Err(StoreError::InvalidLifecycle("invalid status".into()));
    }
    if !matches!(i.severity.as_str(), "low" | "medium" | "high" | "critical") {
        return Err(StoreError::InvalidLifecycle("invalid severity".into()));
    }
    if i.kind == "metric" && (i.metric_value.trim().is_empty() || i.metric_unit.trim().is_empty()) {
        return Err(StoreError::InvalidLifecycle(
            "metric value and unit required".into(),
        ));
    }
    if i.kind == "channel_plan" && i.channel.trim().is_empty() {
        return Err(StoreError::InvalidLifecycle("channel required".into()));
    }
    if i.kind == "launch_asset" && i.artifact_id.is_none() {
        return Err(StoreError::InvalidLifecycle(
            "launch asset requires artifact".into(),
        ));
    }
    Ok(())
}

fn resolve_task(c: &Connection, project_id: &str, logical: Option<&str>) -> Result<Option<String>> {
    logical
        .map(|id| {
            c.query_row(
                "SELECT t.id FROM tasks t JOIN projects p ON p.id=t.project_id
        WHERE t.project_id=?1 AND t.logical_id=?2 AND t.scope_revision=p.active_scope_revision",
                params![project_id, id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| {
                StoreError::InvalidLifecycle("task must belong to active project scope".into())
            })
        })
        .transpose()
}

fn validate_artifact(c: &Connection, project_id: &str, id: Option<&str>) -> Result<()> {
    if let Some(id) = id {
        let exists: bool = c.query_row(
            "SELECT EXISTS(SELECT 1 FROM artifacts WHERE id=?1 AND project_id=?2)",
            params![id, project_id],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(StoreError::InvalidLifecycle(
                "artifact must belong to project".into(),
            ));
        }
    }
    Ok(())
}

fn record_from(c: &Connection, project_id: &str, id: &str) -> Result<LifecycleRecord> {
    c.query_row("SELECT e.kind,e.title,e.body,e.channel,e.metric_value,e.metric_unit,e.status,e.severity,
        e.publication_target,t.logical_id,e.artifact_id,e.publication_approval_id,e.revision,e.created_at,e.updated_at
        FROM lifecycle_entries e LEFT JOIN tasks t ON t.id=e.task_id WHERE e.id=?1 AND e.project_id=?2",
        params![id,project_id],|r|Ok(LifecycleRecord{id:id.into(),project_id:project_id.into(),input:LifecycleInput{
            kind:r.get(0)?,title:r.get(1)?,body:r.get(2)?,channel:r.get(3)?,metric_value:r.get(4)?,
            metric_unit:r.get(5)?,status:r.get(6)?,severity:r.get(7)?,publication_target:r.get(8)?,
            task_logical_id:r.get(9)?,artifact_id:r.get(10)?},publication_approval_id:r.get(11)?,
            revision:r.get(12)?,created_at:r.get(13)?,updated_at:r.get(14)?}))
        .optional()?.ok_or_else(||StoreError::NotFound(id.into()))
}

fn check_revision(r: &LifecycleRecord, expected: i64) -> Result<()> {
    if r.revision != expected {
        return Err(StoreError::RevisionConflict {
            id: r.id.clone(),
            expected,
            actual: r.revision,
        });
    }
    Ok(())
}

fn content_hash(r: &LifecycleRecord) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(
            &json!({"id":r.id,"project_id":r.project_id,
        "revision":r.revision,"input":r.input})
        )?)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(kind: &str) -> LifecycleInput {
        LifecycleInput {
            kind: kind.into(),
            title: "Record".into(),
            body: "Detail".into(),
            status: "open".into(),
            severity: "low".into(),
            ..Default::default()
        }
    }

    #[test]
    fn publication_is_exact_revision_target_and_one_use() {
        let mut s = Store::open_in_memory().unwrap();
        s.create_project("p", "Project", "C:/p", "owner", "p")
            .unwrap();
        let mut draft = input("launch_copy");
        draft.publication_target = "example.com".into();
        let mut r = s.create_lifecycle("p", &draft, "owner", "create").unwrap();
        assert!(
            s.record_manual_publication("p", &r.id, r.revision, "missing", "owner", "no")
                .is_err()
        );
        let approval = s
            .approve_publication("p", &r.id, r.revision, "owner", "approve")
            .unwrap();
        draft.body = "Revised copy".into();
        r = s
            .revise_lifecycle("p", &r.id, r.revision, &draft, "owner", "revise")
            .unwrap();
        assert!(
            s.record_manual_publication("p", &r.id, r.revision, &approval, "owner", "stale")
                .is_err()
        );
        let approval = s
            .approve_publication("p", &r.id, r.revision, "owner", "approve2")
            .unwrap();
        r = s
            .record_manual_publication("p", &r.id, r.revision, &approval, "owner", "published")
            .unwrap();
        assert_eq!(r.input.status, "published");
        assert!(
            s.record_manual_publication("p", &r.id, r.revision, &approval, "owner", "replay")
                .is_err()
        );
    }

    #[test]
    fn latest_release_and_operations_blockers_are_derived() {
        let folder = tempfile::tempdir().unwrap();
        let check_file = folder.path().join("check.log");
        std::fs::write(&check_file, b"passed").unwrap();
        let mut s = Store::open_in_memory().unwrap();
        s.create_project(
            "p",
            "Project",
            folder.path().to_str().unwrap(),
            "owner",
            "p",
        )
        .unwrap();
        let mut incident = input("incident");
        incident.severity = "critical".into();
        let mut r = s
            .create_lifecycle("p", &incident, "owner", "incident")
            .unwrap();
        assert_eq!(s.lifecycle_blockers("p").unwrap().operations, 1);
        assert_eq!(s.project_metrics("p").unwrap().blocked_count, 1);
        incident.status = "resolved".into();
        r = s
            .revise_lifecycle("p", &r.id, r.revision, &incident, "owner", "resolve")
            .unwrap();
        assert_eq!(r.input.status, "resolved");
        assert_eq!(s.lifecycle_blockers("p").unwrap().operations, 0);
        assert_eq!(s.project_metrics("p").unwrap().blocked_count, 0);
        let asset_file = folder.path().join("launch.png");
        std::fs::write(&asset_file, b"asset").unwrap();
        let asset_id = s
            .attach_release_artifact("p", &asset_file, "launch-asset", None, "owner", "asset")
            .unwrap();
        let mut asset = input("launch_asset");
        asset.artifact_id = Some(asset_id.clone());
        let asset = s
            .create_lifecycle("p", &asset, "owner", "launch-asset")
            .unwrap();
        assert_eq!(asset.input.artifact_id.as_deref(), Some(asset_id.as_str()));
        let commit = "a".repeat(40);
        let mut release = s
            .create_release(
                "p",
                "1.0",
                &commit,
                "production",
                "Restore backup",
                "owner",
                "release1",
            )
            .unwrap();
        release = s
            .set_release_check(
                "p",
                &release.id,
                release.revision,
                "quality",
                true,
                "failed",
                None,
                "owner",
                "failed-check",
            )
            .unwrap();
        assert_eq!(s.lifecycle_blockers("p").unwrap().release, 1);
        let evidence = s
            .attach_release_artifact(
                "p",
                &check_file,
                "check-log",
                Some(&commit),
                "owner",
                "evidence",
            )
            .unwrap();
        release = s
            .set_release_check(
                "p",
                &release.id,
                release.revision,
                "quality",
                true,
                "passed",
                Some(&evidence),
                "owner",
                "pass-check",
            )
            .unwrap();
        assert_eq!(s.lifecycle_blockers("p").unwrap().release, 0);
        let failed = s
            .set_release_check(
                "p",
                &release.id,
                release.revision,
                "quality",
                true,
                "failed",
                None,
                "owner",
                "failed-again",
            )
            .unwrap();
        assert_eq!(s.lifecycle_blockers("p").unwrap().release, 1);
        let replacement = s
            .create_release(
                "p",
                "1.1",
                &commit,
                "production",
                "Restore backup",
                "owner",
                "replacement",
            )
            .unwrap();
        assert_ne!(failed.id, replacement.id);
        assert_eq!(s.lifecycle_blockers("p").unwrap().release, 0);
    }
}
