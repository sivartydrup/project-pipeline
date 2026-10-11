use super::discovery::insert_event;
use super::{Result, Store, StoreError, reject_sensitive_text, validate_nonempty};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignArtifactInput {
    pub task_logical_id: Option<String>,
    pub decision_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub uri: Option<String>,
    pub mime_type: Option<String>,
    pub source_run_id: Option<String>,
    pub git_commit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignArtifactRecord {
    pub id: String,
    pub root_id: String,
    pub project_id: String,
    pub input: DesignArtifactInput,
    pub sha256: String,
    pub version: i64,
    pub status: String,
    pub review_note: Option<String>,
    pub reviewed_by: Option<String>,
    pub actor: String,
    pub created_at: String,
}

impl Store {
    pub fn list_design_artifacts(&self, project_id: &str) -> Result<Vec<DesignArtifactRecord>> {
        self.get_project(project_id)?
            .ok_or_else(|| StoreError::NotFound(project_id.into()))?;
        let mut query = self.connection.prepare(
            "SELECT a.id,a.root_id,a.project_id,t.logical_id,a.decision_id,a.kind,a.title,a.body,
                    a.uri,a.sha256,a.mime_type,a.source_run_id,a.git_commit,a.version,a.status,
                    a.review_note,a.reviewed_by,a.actor,a.created_at
             FROM design_artifacts a LEFT JOIN tasks t ON t.id=a.task_id
             WHERE a.project_id=?1 ORDER BY a.created_at DESC,a.rowid DESC",
        )?;
        query
            .query_map([project_id], record_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn create_design_artifact(
        &mut self,
        project_id: &str,
        input: &DesignArtifactInput,
        actor: &str,
        correlation_id: &str,
    ) -> Result<DesignArtifactRecord> {
        reject_sensitive_text(&serde_json::to_string(input)?)?;
        validate_nonempty("actor", actor)?;
        validate_nonempty("correlation_id", correlation_id)?;
        let normalized = normalize_input(input);
        let input = &normalized;
        let digest = validate_and_hash(&self.connection, project_id, input)?;
        let tx = self.connection.transaction()?;
        let task_id = resolve_links(&tx, project_id, input)?;
        let id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO design_artifacts(id,root_id,project_id,task_id,decision_id,kind,title,body,
             uri,sha256,mime_type,source_run_id,git_commit,version,status,actor)
             VALUES (?1,?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,1,'in_review',?13)",
            params![id,project_id,task_id,input.decision_id,input.kind,input.title.trim(),
                input.body,input.uri,digest,input.mime_type,input.source_run_id,input.git_commit,actor],
        )?;
        insert_event(
            &tx,
            project_id,
            actor,
            "design.create",
            "design_artifact",
            &id,
            1,
            None,
            Some(
                json!({"kind":input.kind,"title":input.title,"sha256":digest,
                "task":input.task_logical_id,"decision":input.decision_id}),
            ),
            correlation_id,
        )?;
        tx.commit()?;
        self.design_version(project_id, &id, 1)
    }

    pub fn revise_design_artifact(
        &mut self,
        project_id: &str,
        root_id: &str,
        expected_version: i64,
        input: &DesignArtifactInput,
        actor: &str,
        correlation_id: &str,
    ) -> Result<DesignArtifactRecord> {
        reject_sensitive_text(&serde_json::to_string(input)?)?;
        validate_nonempty("actor", actor)?;
        validate_nonempty("correlation_id", correlation_id)?;
        let normalized = normalize_input(input);
        let input = &normalized;
        let digest = validate_and_hash(&self.connection, project_id, input)?;
        let tx = self.connection.transaction()?;
        let latest = latest_version(&tx, project_id, root_id)?;
        if latest != expected_version {
            return Err(StoreError::RevisionConflict {
                id: root_id.into(),
                expected: expected_version,
                actual: latest,
            });
        }
        let task_id = resolve_links(&tx, project_id, input)?;
        let id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO design_artifacts(id,root_id,project_id,task_id,decision_id,kind,title,body,
             uri,sha256,mime_type,source_run_id,git_commit,version,status,actor)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,'in_review',?15)",
            params![id,root_id,project_id,task_id,input.decision_id,input.kind,input.title.trim(),
                input.body,input.uri,digest,input.mime_type,input.source_run_id,input.git_commit,
                latest+1,actor],
        )?;
        insert_event(
            &tx,
            project_id,
            actor,
            "design.revise",
            "design_artifact",
            root_id,
            latest + 1,
            Some(json!({"version":latest})),
            Some(json!({"version":latest+1,"sha256":digest,"title":input.title})),
            correlation_id,
        )?;
        tx.commit()?;
        self.design_version(project_id, root_id, latest + 1)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn review_design_artifact(
        &mut self,
        project_id: &str,
        root_id: &str,
        expected_version: i64,
        approve: bool,
        note: &str,
        actor: &str,
        correlation_id: &str,
    ) -> Result<DesignArtifactRecord> {
        if actor != "owner" {
            return Err(StoreError::ApprovalRequiresOwner);
        }
        validate_nonempty("correlation_id", correlation_id)?;
        if !approve {
            validate_nonempty("review note", note)?;
        }
        let tx = self.connection.transaction()?;
        let latest = latest_version(&tx, project_id, root_id)?;
        if latest != expected_version {
            return Err(StoreError::RevisionConflict {
                id: root_id.into(),
                expected: expected_version,
                actual: latest,
            });
        }
        let prior: String = tx.query_row(
            "SELECT status FROM design_artifacts WHERE project_id=?1 AND root_id=?2 AND version=?3",
            params![project_id, root_id, latest],
            |row| row.get(0),
        )?;
        if prior != "in_review" {
            return Err(StoreError::InvalidDesign(
                "latest version is already reviewed".into(),
            ));
        }
        if approve {
            let (kind,title,body,uri,stored_hash):(String,String,String,Option<String>,String)=tx.query_row(
                "SELECT kind,title,body,uri,sha256 FROM design_artifacts WHERE project_id=?1 AND root_id=?2 AND version=?3",
                params![project_id,root_id,latest],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
            let current_hash = validate_and_hash(
                &tx,
                project_id,
                &DesignArtifactInput {
                    kind,
                    title,
                    body,
                    uri,
                    ..Default::default()
                },
            )?;
            if current_hash != stored_hash {
                return Err(StoreError::InvalidDesign(
                    "artifact bytes changed since review submission".into(),
                ));
            }
        }
        let status = if approve {
            "approved"
        } else {
            "changes_requested"
        };
        tx.execute("UPDATE design_artifacts SET status=?1,review_note=?2,reviewed_by=?3,
                    reviewed_at=CURRENT_TIMESTAMP WHERE project_id=?4 AND root_id=?5 AND version=?6",
            params![status,note.trim(),actor,project_id,root_id,latest])?;
        insert_event(
            &tx,
            project_id,
            actor,
            "design.review",
            "design_artifact",
            root_id,
            latest,
            Some(json!({"status":prior})),
            Some(json!({"status":status,"note":note.trim()})),
            correlation_id,
        )?;
        tx.commit()?;
        self.design_version(project_id, root_id, latest)
    }

    fn design_version(
        &self,
        project_id: &str,
        root_id: &str,
        version: i64,
    ) -> Result<DesignArtifactRecord> {
        self.list_design_artifacts(project_id)?
            .into_iter()
            .find(|a| a.root_id == root_id && a.version == version)
            .ok_or_else(|| StoreError::InvalidDesign("version disappeared".into()))
    }
}

fn record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DesignArtifactRecord> {
    Ok(DesignArtifactRecord {
        id: row.get(0)?,
        root_id: row.get(1)?,
        project_id: row.get(2)?,
        input: DesignArtifactInput {
            task_logical_id: row.get(3)?,
            decision_id: row.get(4)?,
            kind: row.get(5)?,
            title: row.get(6)?,
            body: row.get(7)?,
            uri: row.get(8)?,
            mime_type: row.get(10)?,
            source_run_id: row.get(11)?,
            git_commit: row.get(12)?,
        },
        sha256: row.get(9)?,
        version: row.get(13)?,
        status: row.get(14)?,
        review_note: row.get(15)?,
        reviewed_by: row.get(16)?,
        actor: row.get(17)?,
        created_at: row.get(18)?,
    })
}

fn normalize_input(input: &DesignArtifactInput) -> DesignArtifactInput {
    let mut normalized = input.clone();
    for field in [
        &mut normalized.task_logical_id,
        &mut normalized.decision_id,
        &mut normalized.uri,
        &mut normalized.mime_type,
        &mut normalized.source_run_id,
        &mut normalized.git_commit,
    ] {
        if field
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            *field = None;
        }
    }
    normalized
}

fn latest_version(connection: &Connection, project_id: &str, root_id: &str) -> Result<i64> {
    connection
        .query_row(
            "SELECT max(version) FROM design_artifacts WHERE project_id=?1 AND root_id=?2",
            params![project_id, root_id],
            |row| row.get::<_, Option<i64>>(0),
        )?
        .ok_or_else(|| StoreError::InvalidDesign("artifact not found in project".into()))
}

fn resolve_links(
    connection: &Connection,
    project_id: &str,
    input: &DesignArtifactInput,
) -> Result<Option<String>> {
    let task_id = if let Some(logical_id) = input
        .task_logical_id
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        Some(connection.query_row("SELECT t.id FROM tasks t JOIN projects p ON p.id=t.project_id
                WHERE t.project_id=?1 AND t.logical_id=?2 AND t.scope_revision=p.active_scope_revision",
            params![project_id,logical_id],|row|row.get(0)).optional()?
            .ok_or_else(|| StoreError::TaskNotInActiveScope(logical_id.into()))?)
    } else {
        None
    };
    if let Some(id) = input
        .decision_id
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        let valid: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM decisions WHERE id=?1 AND project_id=?2)",
            params![id, project_id],
            |row| row.get(0),
        )?;
        if !valid {
            return Err(StoreError::DecisionNotFound(id.into()));
        }
    }
    if let Some(id) = input
        .source_run_id
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        let valid: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_runs WHERE id=?1 AND project_id=?2)",
            params![id, project_id],
            |row| row.get(0),
        )?;
        if !valid {
            return Err(StoreError::InvalidDesign(
                "source run is outside project".into(),
            ));
        }
    }
    Ok(task_id)
}

fn validate_and_hash(
    connection: &Connection,
    project_id: &str,
    input: &DesignArtifactInput,
) -> Result<String> {
    validate_nonempty("title", &input.title)?;
    if !matches!(
        input.kind.as_str(),
        "flow" | "wireframe" | "screenshot" | "component" | "copy" | "asset" | "accessibility"
    ) {
        return Err(StoreError::InvalidDesign("unsupported kind".into()));
    }
    if input.body.trim().is_empty() && input.uri.as_deref().is_none_or(|s| s.trim().is_empty()) {
        return Err(StoreError::InvalidDesign(
            "body or project file required".into(),
        ));
    }
    let project_path: String = connection
        .query_row(
            "SELECT path FROM projects WHERE id=?1",
            [project_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| StoreError::NotFound(project_id.into()))?;
    let mut hasher = Sha256::new();
    hasher.update(input.body.as_bytes());
    if let Some(uri) = input.uri.as_deref().filter(|s| !s.trim().is_empty()) {
        let root = dunce::canonicalize(&project_path)
            .map_err(|e| StoreError::InvalidDesign(format!("project path: {e}")))?;
        let file = dunce::canonicalize(uri)
            .map_err(|e| StoreError::InvalidDesign(format!("artifact file: {e}")))?;
        if !file.starts_with(&root) || !file.is_file() {
            return Err(StoreError::InvalidDesign(
                "file must be inside project folder".into(),
            ));
        }
        hasher.update([0]);
        let mut reader =
            File::open(Path::new(&file)).map_err(|e| StoreError::InvalidDesign(e.to_string()))?;
        let mut block = [0_u8; 8192];
        loop {
            let count = reader
                .read(&mut block)
                .map_err(|e| StoreError::InvalidDesign(e.to_string()))?;
            if count == 0 {
                break;
            }
            hasher.update(&block[..count]);
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn design_to_implementation_journey_preserves_reviewed_versions_and_links() {
        let folder = tempdir().unwrap();
        let screenshot = folder.path().join("screen.png");
        fs::write(&screenshot, b"screen one").unwrap();
        let mut store = Store::open_in_memory().unwrap();
        store
            .create_project(
                "p",
                "Sample",
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
            VALUES ('task','p','Build screen','Done','ready',1,'UX-1')",
                [],
            )
            .unwrap();
        store.connection.execute("INSERT INTO decisions(id,project_id,question,alternatives_json,impact,status,actor)
            VALUES ('decision','p','Which layout?','[\"A\",\"B\"]','low','proposed','owner')",[]).unwrap();
        let input = DesignArtifactInput {
            task_logical_id: Some("UX-1".into()),
            decision_id: Some("decision".into()),
            kind: "screenshot".into(),
            title: "Home screen".into(),
            body: "Desktop layout".into(),
            uri: Some(screenshot.to_string_lossy().into_owned()),
            mime_type: Some("image/png".into()),
            git_commit: Some("abc123".into()),
            ..Default::default()
        };
        let first = store
            .create_design_artifact("p", &input, "owner", "draft")
            .unwrap();
        assert_eq!(first.version, 1);
        assert_eq!(first.status, "in_review");
        assert_eq!(first.input.task_logical_id.as_deref(), Some("UX-1"));
        assert_eq!(first.input.decision_id.as_deref(), Some("decision"));
        assert_eq!(first.sha256.len(), 64);
        assert!(matches!(
            store.review_design_artifact("p", &first.root_id, 1, false, "", "owner", "review"),
            Err(StoreError::EmptyField(_))
        ));
        let requested = store
            .review_design_artifact(
                "p",
                &first.root_id,
                1,
                false,
                "increase contrast",
                "owner",
                "review",
            )
            .unwrap();
        assert_eq!(requested.status, "changes_requested");
        fs::write(&screenshot, b"screen two").unwrap();
        let mut revised = input.clone();
        revised.body = "Contrast updated".into();
        let second = store
            .revise_design_artifact("p", &first.root_id, 1, &revised, "owner", "revision")
            .unwrap();
        assert_eq!(second.version, 2);
        assert_ne!(first.sha256, second.sha256);
        assert!(matches!(
            store.revise_design_artifact("p", &first.root_id, 1, &revised, "owner", "stale"),
            Err(StoreError::RevisionConflict { .. })
        ));
        assert!(matches!(
            store.review_design_artifact("p", &first.root_id, 1, true, "", "owner", "stale"),
            Err(StoreError::RevisionConflict { .. })
        ));
        assert!(matches!(
            store.review_design_artifact("p", &first.root_id, 2, true, "", "agent:r", "agent"),
            Err(StoreError::ApprovalRequiresOwner)
        ));
        fs::write(&screenshot, b"modified after submission").unwrap();
        assert!(matches!(
            store.review_design_artifact("p", &first.root_id, 2, true, "", "owner", "tampered"),
            Err(StoreError::InvalidDesign(_))
        ));
        fs::write(&screenshot, b"screen two").unwrap();
        store
            .review_design_artifact(
                "p",
                &first.root_id,
                2,
                true,
                "Looks good",
                "owner",
                "approve",
            )
            .unwrap();
        let versions = store.list_design_artifacts("p").unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].status, "approved");
        assert_eq!(versions[1].status, "changes_requested");
        assert_eq!(store.activity_count("p").unwrap(), 5);
        let export = store.export_json().unwrap();
        assert!(export.contains("design_artifacts"));
        assert!(export.contains("increase contrast"));
    }

    #[test]
    fn design_file_and_links_are_project_scoped() {
        let first = tempdir().unwrap();
        let other = tempdir().unwrap();
        let outside = other.path().join("outside.png");
        fs::write(&outside, b"image").unwrap();
        let mut store = Store::open_in_memory().unwrap();
        store
            .create_project(
                "p",
                "Sample",
                first.path().to_str().unwrap(),
                "owner",
                "create",
            )
            .unwrap();
        let input = DesignArtifactInput {
            kind: "asset".into(),
            title: "Logo".into(),
            uri: Some(outside.to_string_lossy().into_owned()),
            ..Default::default()
        };
        assert!(matches!(
            store.create_design_artifact("p", &input, "owner", "outside"),
            Err(StoreError::InvalidDesign(_))
        ));
        let input = DesignArtifactInput {
            kind: "copy".into(),
            title: "Header".into(),
            body: "Hello".into(),
            decision_id: Some("other-project-decision".into()),
            ..Default::default()
        };
        assert!(matches!(
            store.create_design_artifact("p", &input, "owner", "bad-link"),
            Err(StoreError::DecisionNotFound(_))
        ));
        assert!(store.list_design_artifacts("p").unwrap().is_empty());
    }

    #[test]
    fn all_design_workflow_kinds_are_recordable() {
        let folder = tempdir().unwrap();
        let mut store = Store::open_in_memory().unwrap();
        store
            .create_project(
                "p",
                "Sample",
                folder.path().to_str().unwrap(),
                "owner",
                "create",
            )
            .unwrap();
        for kind in [
            "flow",
            "wireframe",
            "screenshot",
            "component",
            "copy",
            "asset",
            "accessibility",
        ] {
            let input = DesignArtifactInput {
                kind: kind.into(),
                title: kind.into(),
                body: format!("{kind} review"),
                decision_id: Some(String::new()),
                source_run_id: Some(String::new()),
                ..Default::default()
            };
            let saved = store
                .create_design_artifact("p", &input, "owner", kind)
                .unwrap();
            assert_eq!(saved.input.kind, kind);
        }
        assert_eq!(store.list_design_artifacts("p").unwrap().len(), 7);
    }
}
