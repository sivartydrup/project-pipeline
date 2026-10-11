use super::discovery::insert_event;
use super::{Result, Store, StoreError, validate_nonempty};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseBuild {
    pub id: String,
    pub artifact_id: String,
    pub platform: String,
    pub uri: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseCheck {
    pub id: String,
    pub name: String,
    pub required: bool,
    pub status: String,
    pub evidence_artifact_id: Option<String>,
    pub evidence_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentRecord {
    pub id: String,
    pub execution_approval_id: String,
    pub result: String,
    pub smoke_result: String,
    pub notes: String,
    pub actor: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseRecord {
    pub id: String,
    pub project_id: String,
    pub version: String,
    pub git_commit: String,
    pub target: String,
    pub rollback_notes: String,
    pub status: String,
    pub revision: i64,
    pub builds: Vec<ReleaseBuild>,
    pub checks: Vec<ReleaseCheck>,
    pub deployments: Vec<DeploymentRecord>,
}

impl Store {
    pub fn attach_release_artifact(
        &mut self,
        project_id: &str,
        path: &Path,
        kind: &str,
        git_commit: Option<&str>,
        actor: &str,
        correlation_id: &str,
    ) -> Result<String> {
        if !matches!(
            kind,
            "build" | "check-log" | "smoke-log" | "release-note" | "launch-asset"
        ) {
            return Err(StoreError::InvalidRelease(
                "invalid release artifact kind".into(),
            ));
        }
        let project = self
            .get_project(project_id)?
            .ok_or_else(|| StoreError::NotFound(project_id.into()))?;
        let root = dunce::canonicalize(&project.path)
            .map_err(|e| StoreError::InvalidRelease(e.to_string()))?;
        let file =
            dunce::canonicalize(path).map_err(|e| StoreError::InvalidRelease(e.to_string()))?;
        if !file.starts_with(&root) || !file.is_file() {
            return Err(StoreError::InvalidRelease(
                "artifact must be a file inside project root".into(),
            ));
        }
        let bytes = fs::read(&file).map_err(|e| StoreError::InvalidRelease(e.to_string()))?;
        let digest = format!("{:x}", Sha256::digest(&bytes));
        let id = Uuid::new_v4().to_string();
        let tx = self.connection.transaction()?;
        tx.execute(
            "INSERT INTO artifacts(id,project_id,kind,uri,sha256,git_commit)
                    VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                id,
                project_id,
                kind,
                file.to_string_lossy(),
                digest,
                git_commit
            ],
        )?;
        insert_event(
            &tx,
            project_id,
            actor,
            "release.artifact_attach",
            "artifact",
            &id,
            1,
            None,
            Some(json!({"kind":kind,"uri":file,"sha256":digest,"git_commit":git_commit})),
            correlation_id,
        )?;
        tx.commit()?;
        Ok(id)
    }

    pub fn list_releases(&self, project_id: &str) -> Result<Vec<ReleaseRecord>> {
        self.get_project(project_id)?
            .ok_or_else(|| StoreError::NotFound(project_id.into()))?;
        let mut statement = self.connection.prepare(
            "SELECT id FROM release_candidates WHERE project_id=?1 ORDER BY created_at DESC,rowid DESC",
        )?;
        let ids = statement.query_map([project_id], |row| row.get::<_, String>(0))?;
        ids.map(|id| self.release(project_id, &id?)).collect()
    }

    pub fn release(&self, project_id: &str, id: &str) -> Result<ReleaseRecord> {
        release_from(&self.connection, project_id, id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_release(
        &mut self,
        project_id: &str,
        version: &str,
        git_commit: &str,
        target: &str,
        rollback_notes: &str,
        actor: &str,
        correlation_id: &str,
    ) -> Result<ReleaseRecord> {
        for (field, value) in [
            ("version", version),
            ("target", target),
            ("rollback_notes", rollback_notes),
            ("actor", actor),
            ("correlation_id", correlation_id),
        ] {
            validate_nonempty(field, value)?;
        }
        if !matches!(git_commit.len(), 40 | 64)
            || !git_commit.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(StoreError::InvalidRelease(
                "full Git commit hash required".into(),
            ));
        }
        let id = Uuid::new_v4().to_string();
        let tx = self.connection.transaction()?;
        tx.execute("INSERT INTO release_candidates(id,project_id,version,git_commit,target,checklist_json,rollback_notes,status)
                    VALUES (?1,?2,?3,?4,?5,'[]',?6,'draft')",
            params![id,project_id,version.trim(),git_commit,target.trim(),rollback_notes.trim()])?;
        insert_event(
            &tx,
            project_id,
            actor,
            "release.create",
            "release",
            &id,
            1,
            None,
            Some(json!({"version":version,"git_commit":git_commit,"target":target})),
            correlation_id,
        )?;
        tx.commit()?;
        self.release(project_id, &id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_release_build(
        &mut self,
        project_id: &str,
        id: &str,
        expected_revision: i64,
        artifact_id: &str,
        platform: &str,
        actor: &str,
        correlation_id: &str,
    ) -> Result<ReleaseRecord> {
        validate_nonempty("platform", platform)?;
        let tx = self.connection.transaction()?;
        require_draft(&tx, project_id, id, expected_revision)?;
        let artifact: Option<(String, Option<String>)> = tx
            .query_row(
                "SELECT kind,git_commit FROM artifacts WHERE id=?1 AND project_id=?2",
                params![artifact_id, project_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let commit: String = tx.query_row(
            "SELECT git_commit FROM release_candidates WHERE id=?1",
            [id],
            |r| r.get(0),
        )?;
        if !matches!(artifact,Some((ref kind,Some(ref artifact_commit)))
            if kind=="build" && artifact_commit==&commit)
        {
            return Err(StoreError::InvalidRelease(
                "build artifact must belong to project and exact commit".into(),
            ));
        }
        let build_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO release_builds(id,candidate_id,artifact_id,platform) VALUES (?1,?2,?3,?4)",
            params![build_id, id, artifact_id, platform.trim()],
        )?;
        bump(&tx, id)?;
        insert_event(
            &tx,
            project_id,
            actor,
            "release.build_add",
            "release",
            id,
            expected_revision + 1,
            None,
            Some(json!({"artifact_id":artifact_id,"platform":platform})),
            correlation_id,
        )?;
        tx.commit()?;
        self.release(project_id, id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_release_check(
        &mut self,
        project_id: &str,
        id: &str,
        expected_revision: i64,
        name: &str,
        required: bool,
        status: &str,
        evidence_artifact_id: Option<&str>,
        actor: &str,
        correlation_id: &str,
    ) -> Result<ReleaseRecord> {
        validate_nonempty("check name", name)?;
        if !matches!(status, "pending" | "passed" | "failed") {
            return Err(StoreError::InvalidRelease("invalid check status".into()));
        }
        if status == "passed" && evidence_artifact_id.is_none() {
            return Err(StoreError::InvalidRelease(
                "passing check requires evidence artifact".into(),
            ));
        }
        let tx = self.connection.transaction()?;
        require_draft(&tx, project_id, id, expected_revision)?;
        if let Some(artifact_id) = evidence_artifact_id {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM artifacts WHERE id=?1 AND project_id=?2)",
                params![artifact_id, project_id],
                |r| r.get(0),
            )?;
            if !exists {
                return Err(StoreError::InvalidRelease(
                    "check evidence must belong to project".into(),
                ));
            }
        }
        tx.execute("INSERT INTO release_checks(id,candidate_id,name,required,status,evidence_artifact_id)
                    VALUES (?1,?2,?3,?4,?5,?6)
                    ON CONFLICT(candidate_id,name) DO UPDATE SET required=excluded.required,status=excluded.status,
                    evidence_artifact_id=excluded.evidence_artifact_id,updated_at=CURRENT_TIMESTAMP",
            params![Uuid::new_v4().to_string(),id,name.trim(),required,status,evidence_artifact_id])?;
        bump(&tx, id)?;
        insert_event(
            &tx,
            project_id,
            actor,
            "release.check_set",
            "release",
            id,
            expected_revision + 1,
            None,
            Some(
                json!({"name":name,"required":required,"status":status,"evidence_artifact_id":evidence_artifact_id}),
            ),
            correlation_id,
        )?;
        tx.commit()?;
        self.release(project_id, id)
    }

    pub fn approve_release(
        &mut self,
        project_id: &str,
        id: &str,
        expected_revision: i64,
        actor: &str,
        correlation_id: &str,
    ) -> Result<ReleaseRecord> {
        if actor != "owner" {
            return Err(StoreError::ApprovalRequiresOwner);
        }
        let tx = self.connection.transaction()?;
        require_draft(&tx, project_id, id, expected_revision)?;
        let release = release_from(&tx, project_id, id)?;
        if release.builds.is_empty()
            || release.checks.is_empty()
            || release
                .checks
                .iter()
                .any(|c| c.required && c.status != "passed")
        {
            return Err(StoreError::InvalidRelease(
                "build and passing required checks needed".into(),
            ));
        }
        for build in &release.builds {
            verify_artifact(&tx, project_id, &build.artifact_id)?;
        }
        for check in &release.checks {
            if let Some(artifact_id) = &check.evidence_artifact_id {
                verify_artifact(&tx, project_id, artifact_id)?;
            }
        }
        let hash = release_hash(&release)?;
        tx.execute("INSERT INTO approvals(id,project_id,subject_type,subject_id,subject_revision,subject_hash,
                    action_class,target,approver) VALUES (?1,?2,'release',?3,?4,?5,'release.approve',?6,?7)",
            params![Uuid::new_v4().to_string(),project_id,id,expected_revision,hash,release.target,actor])?;
        tx.execute("UPDATE release_candidates SET status='approved',revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?1",[id])?;
        insert_event(
            &tx,
            project_id,
            actor,
            "release.approve",
            "release",
            id,
            expected_revision + 1,
            None,
            Some(json!({"subject_hash":hash,"target":release.target})),
            correlation_id,
        )?;
        tx.commit()?;
        self.release(project_id, id)
    }

    pub fn approve_release_execution(
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
        let release = release_from(&tx, project_id, id)?;
        if release.revision != expected_revision {
            return Err(StoreError::RevisionConflict {
                id: id.into(),
                expected: expected_revision,
                actual: release.revision,
            });
        }
        if release.status != "approved" {
            return Err(StoreError::InvalidRelease(
                "candidate must be approved first".into(),
            ));
        }
        let approval_id = Uuid::new_v4().to_string();
        let hash = release_hash(&release)?;
        tx.execute("INSERT INTO approvals(id,project_id,subject_type,subject_id,subject_revision,subject_hash,
                    action_class,target,approver) VALUES (?1,?2,'release',?3,?4,?5,'release.execute',?6,?7)",
            params![approval_id,project_id,id,expected_revision,hash,release.target,actor])?;
        insert_event(
            &tx,
            project_id,
            actor,
            "release.execution_approve",
            "release",
            id,
            expected_revision,
            None,
            Some(json!({"approval_id":approval_id,"target":release.target})),
            correlation_id,
        )?;
        tx.commit()?;
        Ok(approval_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_manual_deployment(
        &mut self,
        project_id: &str,
        id: &str,
        expected_revision: i64,
        approval_id: &str,
        result: &str,
        smoke_result: &str,
        notes: &str,
        actor: &str,
        correlation_id: &str,
    ) -> Result<ReleaseRecord> {
        if !matches!(result, "succeeded" | "failed")
            || !matches!(smoke_result, "pending" | "passed" | "failed")
        {
            return Err(StoreError::InvalidRelease(
                "invalid deployment or smoke result".into(),
            ));
        }
        if result == "failed" && smoke_result != "pending" {
            return Err(StoreError::InvalidRelease(
                "failed deployment cannot claim a smoke result".into(),
            ));
        }
        validate_nonempty("deployment notes", notes)?;
        let tx = self.connection.transaction()?;
        let release = release_from(&tx, project_id, id)?;
        if release.revision != expected_revision {
            return Err(StoreError::RevisionConflict {
                id: id.into(),
                expected: expected_revision,
                actual: release.revision,
            });
        }
        if release.status != "approved" {
            return Err(StoreError::InvalidRelease(
                "candidate must be approved first".into(),
            ));
        }
        let hash = release_hash(&release)?;
        let valid: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM approvals WHERE id=?1 AND project_id=?2
            AND subject_type='release' AND subject_id=?3 AND subject_revision=?4 AND subject_hash=?5
            AND action_class='release.execute' AND target=?6 AND revoked_at IS NULL)",
            params![
                approval_id,
                project_id,
                id,
                expected_revision,
                hash,
                release.target
            ],
            |r| r.get(0),
        )?;
        let used: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM release_deployments WHERE execution_approval_id=?1)",
            [approval_id],
            |r| r.get(0),
        )?;
        if !valid || used {
            return Err(StoreError::InvalidRelease(
                "unused exact-target execution approval required".into(),
            ));
        }
        tx.execute("INSERT INTO release_deployments(id,candidate_id,execution_approval_id,result,smoke_result,notes,actor)
                    VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![Uuid::new_v4().to_string(),id,approval_id,result,smoke_result,notes.trim(),actor])?;
        let candidate_status = if result == "succeeded" {
            "deployed"
        } else {
            "deployment_failed"
        };
        tx.execute("UPDATE release_candidates SET status=?1,revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![candidate_status,id])?;
        insert_event(
            &tx,
            project_id,
            actor,
            "release.manual_deployment",
            "release",
            id,
            expected_revision + 1,
            None,
            Some(
                json!({"approval_id":approval_id,"result":result,"smoke_result":smoke_result,"notes":notes}),
            ),
            correlation_id,
        )?;
        tx.commit()?;
        self.release(project_id, id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_release_smoke(
        &mut self,
        project_id: &str,
        id: &str,
        expected_revision: i64,
        deployment_id: &str,
        smoke_result: &str,
        notes: &str,
        actor: &str,
        correlation_id: &str,
    ) -> Result<ReleaseRecord> {
        if !matches!(smoke_result, "passed" | "failed") {
            return Err(StoreError::InvalidRelease(
                "smoke result must pass or fail".into(),
            ));
        }
        validate_nonempty("smoke notes", notes)?;
        let tx = self.connection.transaction()?;
        let release = release_from(&tx, project_id, id)?;
        if release.revision != expected_revision {
            return Err(StoreError::RevisionConflict {
                id: id.into(),
                expected: expected_revision,
                actual: release.revision,
            });
        }
        if release.status != "deployed" {
            return Err(StoreError::InvalidRelease(
                "deployment record required".into(),
            ));
        }
        let changed = tx.execute(
            "UPDATE release_deployments SET smoke_result=?1,notes=notes || '\nSmoke: ' || ?2
            WHERE id=?3 AND candidate_id=?4 AND smoke_result='pending'",
            params![smoke_result, notes.trim(), deployment_id, id],
        )?;
        if changed != 1 {
            return Err(StoreError::InvalidRelease(
                "pending smoke record required".into(),
            ));
        }
        bump(&tx, id)?;
        insert_event(
            &tx,
            project_id,
            actor,
            "release.smoke_result",
            "release",
            id,
            expected_revision + 1,
            None,
            Some(json!({"deployment_id":deployment_id,"smoke_result":smoke_result,"notes":notes})),
            correlation_id,
        )?;
        tx.commit()?;
        self.release(project_id, id)
    }
}

fn require_draft(c: &Connection, project_id: &str, id: &str, revision: i64) -> Result<()> {
    let r = release_from(c, project_id, id)?;
    if r.revision != revision {
        return Err(StoreError::RevisionConflict {
            id: id.into(),
            expected: revision,
            actual: r.revision,
        });
    }
    if r.status != "draft" {
        return Err(StoreError::InvalidRelease(
            "approved candidate is immutable".into(),
        ));
    }
    Ok(())
}

fn bump(c: &Connection, id: &str) -> Result<()> {
    c.execute("UPDATE release_candidates SET revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?1",[id])?;
    Ok(())
}

fn release_hash(r: &ReleaseRecord) -> Result<String> {
    let snapshot = json!({"id":r.id,"project_id":r.project_id,"version":r.version,
        "git_commit":r.git_commit,"target":r.target,"rollback_notes":r.rollback_notes,
        "builds":r.builds,"checks":r.checks});
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&snapshot)?)
    ))
}

fn verify_artifact(c: &Connection, project_id: &str, id: &str) -> Result<()> {
    let (uri, digest): (String, String) = c.query_row(
        "SELECT uri,sha256 FROM artifacts WHERE id=?1 AND project_id=?2",
        params![id, project_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let bytes = fs::read(&uri)
        .map_err(|_| StoreError::InvalidRelease("release evidence is unreadable".into()))?;
    if format!("{:x}", Sha256::digest(bytes)) != digest {
        return Err(StoreError::InvalidRelease(
            "release evidence hash changed".into(),
        ));
    }
    Ok(())
}

fn release_from(c: &Connection, project_id: &str, id: &str) -> Result<ReleaseRecord> {
    let base: Option<(String,String,String,String,String,i64)>=c.query_row(
        "SELECT version,git_commit,target,rollback_notes,status,revision FROM release_candidates WHERE id=?1 AND project_id=?2",
        params![id,project_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?;
    let (version, git_commit, target, rollback_notes, status, revision) =
        base.ok_or_else(|| StoreError::NotFound(id.into()))?;
    let builds = c
        .prepare(
            "SELECT b.id,b.artifact_id,b.platform,a.uri,a.sha256 FROM release_builds b
        JOIN artifacts a ON a.id=b.artifact_id WHERE b.candidate_id=?1 ORDER BY b.rowid",
        )?
        .query_map([id], |r| {
            Ok(ReleaseBuild {
                id: r.get(0)?,
                artifact_id: r.get(1)?,
                platform: r.get(2)?,
                uri: r.get(3)?,
                sha256: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let checks=c.prepare("SELECT c.id,c.name,c.required,c.status,c.evidence_artifact_id,a.sha256 FROM release_checks c
        LEFT JOIN artifacts a ON a.id=c.evidence_artifact_id WHERE c.candidate_id=?1 ORDER BY c.name")?
        .query_map([id],|r|Ok(ReleaseCheck{id:r.get(0)?,name:r.get(1)?,required:r.get(2)?,status:r.get(3)?,evidence_artifact_id:r.get(4)?,evidence_sha256:r.get(5)?}))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let deployments=c.prepare("SELECT id,execution_approval_id,result,smoke_result,notes,actor,created_at FROM release_deployments WHERE candidate_id=?1 ORDER BY rowid")?
        .query_map([id],|r|Ok(DeploymentRecord{id:r.get(0)?,execution_approval_id:r.get(1)?,result:r.get(2)?,smoke_result:r.get(3)?,notes:r.get(4)?,actor:r.get(5)?,created_at:r.get(6)?}))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ReleaseRecord {
        id: id.into(),
        project_id: project_id.into(),
        version,
        git_commit,
        target,
        rollback_notes,
        status,
        revision,
        builds,
        checks,
        deployments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Store, String, tempfile::TempDir) {
        let folder = tempfile::tempdir().unwrap();
        let build = folder.path().join("build.exe");
        let check = folder.path().join("check.log");
        fs::write(&build, b"binary").unwrap();
        fs::write(&check, b"passing checks").unwrap();
        let mut store = Store::open_in_memory().unwrap();
        store
            .create_project(
                "p",
                "Project",
                folder.path().to_str().unwrap(),
                "owner",
                "project",
            )
            .unwrap();
        let commit = "a".repeat(40);
        store
            .connection
            .execute(
                "INSERT INTO artifacts(id,project_id,kind,uri,sha256,git_commit)
            VALUES ('build','p','build',?1,?2,?3),('check','p','test-log',?4,?5,?3)",
                params![
                    build.to_str(),
                    format!("{:x}", Sha256::digest(b"binary")),
                    commit,
                    check.to_str(),
                    format!("{:x}", Sha256::digest(b"passing checks"))
                ],
            )
            .unwrap();
        (store, commit, folder)
    }

    #[test]
    fn failed_then_passing_check_controls_release_approval() {
        let (mut store, commit, _folder) = fixture();
        let mut r = store
            .create_release(
                "p",
                "1.0.0",
                &commit,
                "production",
                "Restore 0.9 backup",
                "owner",
                "create",
            )
            .unwrap();
        assert!(
            store
                .approve_release("p", &r.id, r.revision, "owner", "approve")
                .is_err()
        );
        r = store
            .add_release_build(
                "p",
                &r.id,
                r.revision,
                "build",
                "windows-x86_64",
                "owner",
                "build",
            )
            .unwrap();
        r = store
            .set_release_check(
                "p",
                &r.id,
                r.revision,
                "smoke",
                true,
                "failed",
                Some("check"),
                "owner",
                "failed",
            )
            .unwrap();
        assert!(matches!(
            store.approve_release("p", &r.id, r.revision, "owner", "approve"),
            Err(StoreError::InvalidRelease(_))
        ));
        let failed_revision = r.revision;
        r = store
            .set_release_check(
                "p",
                &r.id,
                r.revision,
                "smoke",
                true,
                "passed",
                Some("check"),
                "owner",
                "passed",
            )
            .unwrap();
        assert!(matches!(
            store.approve_release("p", &r.id, failed_revision, "owner", "stale"),
            Err(StoreError::RevisionConflict { .. })
        ));
        assert!(matches!(
            store.approve_release("p", &r.id, r.revision, "agent", "agent"),
            Err(StoreError::ApprovalRequiresOwner)
        ));
        r = store
            .approve_release("p", &r.id, r.revision, "owner", "approved")
            .unwrap();
        assert_eq!(r.status, "approved");
        let approval_hash: String = store
            .connection
            .query_row(
                "SELECT subject_hash FROM approvals
            WHERE subject_id=?1 AND action_class='release.approve'",
                [&r.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(approval_hash, release_hash(&r).unwrap());
        assert_eq!(store.activity_count("p").unwrap(), 6);
    }

    #[test]
    fn deployment_requires_separate_exact_target_one_use_approval() {
        let (mut store, commit, _folder) = fixture();
        let mut r = store
            .create_release(
                "p",
                "1.0.0",
                &commit,
                "production",
                "Restore backup",
                "owner",
                "create",
            )
            .unwrap();
        r = store
            .add_release_build("p", &r.id, r.revision, "build", "windows", "owner", "build")
            .unwrap();
        r = store
            .set_release_check(
                "p",
                &r.id,
                r.revision,
                "security",
                true,
                "passed",
                Some("check"),
                "owner",
                "check",
            )
            .unwrap();
        r = store
            .approve_release("p", &r.id, r.revision, "owner", "approve")
            .unwrap();
        assert!(
            store
                .record_manual_deployment(
                    "p",
                    &r.id,
                    r.revision,
                    "missing",
                    "succeeded",
                    "passed",
                    "manual deploy",
                    "owner",
                    "deploy"
                )
                .is_err()
        );
        assert!(matches!(
            store.approve_release_execution("p", &r.id, r.revision, "agent", "agent"),
            Err(StoreError::ApprovalRequiresOwner)
        ));
        let approval = store
            .approve_release_execution("p", &r.id, r.revision, "owner", "execution")
            .unwrap();
        store
            .connection
            .execute(
                "UPDATE approvals SET target='staging' WHERE id=?1",
                [&approval],
            )
            .unwrap();
        assert!(
            store
                .record_manual_deployment(
                    "p",
                    &r.id,
                    r.revision,
                    &approval,
                    "succeeded",
                    "passed",
                    "manual deploy",
                    "owner",
                    "wrong-target"
                )
                .is_err()
        );
        store
            .connection
            .execute(
                "UPDATE approvals SET target='production' WHERE id=?1",
                [&approval],
            )
            .unwrap();
        r = store
            .record_manual_deployment(
                "p",
                &r.id,
                r.revision,
                &approval,
                "succeeded",
                "passed",
                "manual deploy",
                "owner",
                "deploy",
            )
            .unwrap();
        assert_eq!(r.status, "deployed");
        assert_eq!(r.deployments.len(), 1);
        assert_eq!(r.deployments[0].smoke_result, "passed");
        assert!(
            store
                .record_manual_deployment(
                    "p",
                    &r.id,
                    r.revision,
                    &approval,
                    "succeeded",
                    "passed",
                    "again",
                    "owner",
                    "replay"
                )
                .is_err()
        );
    }
}
