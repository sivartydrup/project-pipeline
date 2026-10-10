use super::{EngineError, ProjectEngine, Result};
use pipeline_store::{AgentGrant, CheckoutRecord, PolicyRequestRecord, RunCheckpoint, RunRecord};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunLimits {
    pub wall_seconds: i64,
    pub token_budget: i64,
    pub token_ttl_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacketCriterion {
    pub id: String,
    pub assertion: String,
    pub verifier: String,
    pub required_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPacket {
    pub schema_version: u8,
    pub project_id: String,
    pub task_id: String,
    pub task_revision: i64,
    pub scope_revision: i64,
    pub checkout_path: String,
    pub goal: String,
    #[serde(default)]
    pub review_feedback: Option<String>,
    pub acceptance_criteria: Vec<PacketCriterion>,
    pub dependencies: Vec<String>,
    pub approved_decision_ids: Vec<String>,
    pub allowed_operations: Vec<String>,
    pub stop_conditions: Vec<String>,
    pub limits: RunLimits,
}

#[derive(Debug, Clone)]
pub struct RunStart {
    pub run: RunRecord,
    pub checkout: CheckoutRecord,
    pub grant: AgentGrant,
    pub packet: TaskPacket,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryRecord {
    pub run_id: Option<String>,
    pub checkout_id: Option<String>,
    pub checkout_path: String,
    pub state: String,
    pub changed_entries: Option<usize>,
    pub checkpoint: Option<RunCheckpoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionClass {
    LocalCheck,
    EditCheckout,
    DestructiveFiles,
    RewriteGit,
    CredentialAccess,
    Spend,
    ExternalPublish,
    Production,
    UnknownExternal,
}

impl ActionClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalCheck => "local_check",
            Self::EditCheckout => "edit_checkout",
            Self::DestructiveFiles => "destructive_files",
            Self::RewriteGit => "rewrite_git",
            Self::CredentialAccess => "credential_access",
            Self::Spend => "spend",
            Self::ExternalPublish => "external_publish",
            Self::Production => "production",
            Self::UnknownExternal => "unknown_external",
        }
    }

    fn requires_approval(self) -> bool {
        !matches!(self, Self::LocalCheck | Self::EditCheckout)
    }
}

#[derive(Debug, Clone)]
pub struct ActionSpec {
    pub class: ActionClass,
    pub target: String,
    /// Exact proposed action, hashed before persistence. Never put raw credentials here.
    pub command_spec: String,
    pub effect_summary: String,
}

#[derive(Debug, Clone)]
pub enum PolicyOutcome {
    Allowed,
    Pending(PolicyRequestRecord),
    Denied(PolicyRequestRecord),
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let disabled_hooks =
        std::env::temp_dir().join(format!("project-pipeline-no-hooks-{}", Uuid::new_v4()));
    let output = Command::new("git")
        .arg("-c")
        .arg(format!("core.hooksPath={}", disabled_hooks.display()))
        .args(["-c", "filter.lfs.process=", "-c", "filter.lfs.smudge="])
        .env("GIT_LFS_SKIP_SMUDGE", "1")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        return Err(EngineError::Git(message.chars().take(500).collect()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn reject_custom_checkout_filters(root: &Path) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "config",
            "--name-only",
            "--get-regexp",
            r"^filter\..*\.(process|smudge)$",
        ])
        .output()?;
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(EngineError::Git(
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(500)
                .collect(),
        ));
    }
    let configured = String::from_utf8_lossy(&output.stdout);
    if configured
        .lines()
        .any(|name| name != "filter.lfs.process" && name != "filter.lfs.smudge")
    {
        return Err(EngineError::RunPreflight(
            "custom Git checkout filter requires isolation review".into(),
        ));
    }
    Ok(())
}

fn safe_checkout_target(root: &Path, target: &str) -> Result<PathBuf> {
    let root = dunce::canonicalize(root)
        .map_err(|_| EngineError::RunPreflight("checkout missing".into()))?;
    let path = Path::new(target);
    if !path.is_absolute() {
        return Err(EngineError::RunPreflight(
            "action target must be absolute".into(),
        ));
    }
    let candidate = if path.exists() {
        dunce::canonicalize(path)?
    } else {
        let parent = path
            .parent()
            .ok_or_else(|| EngineError::RunPreflight("target parent missing".into()))?;
        dunce::canonicalize(parent)?.join(
            path.file_name()
                .ok_or_else(|| EngineError::RunPreflight("target filename missing".into()))?,
        )
    };
    if !candidate.starts_with(root) {
        return Err(EngineError::RunPreflight(
            "action target outside checkout".into(),
        ));
    }
    Ok(candidate)
}

impl ProjectEngine {
    pub fn prepare_agent_run(
        &mut self,
        project_id: &str,
        logical_task_id: &str,
        harness_available: bool,
        allowed_operations: &[String],
        limits: RunLimits,
    ) -> Result<RunStart> {
        let base = std::env::temp_dir().join("project-pipeline-worktrees");
        self.prepare_agent_run_at(
            project_id,
            logical_task_id,
            harness_available,
            allowed_operations,
            limits,
            &base,
        )
    }

    fn prepare_agent_run_at(
        &mut self,
        project_id: &str,
        logical_task_id: &str,
        harness_available: bool,
        allowed_operations: &[String],
        limits: RunLimits,
        worktree_base: &Path,
    ) -> Result<RunStart> {
        if !harness_available {
            return Err(EngineError::RunPreflight("harness unavailable".into()));
        }
        if limits.wall_seconds <= 0
            || limits.token_budget <= 0
            || !(1..=900).contains(&limits.token_ttl_seconds)
        {
            return Err(EngineError::RunPreflight(
                "positive time and token limits required".into(),
            ));
        }
        let project = self
            .store
            .get_project(project_id)?
            .ok_or_else(|| EngineError::RunPreflight("project missing".into()))?;
        let discovery = self.load_discovery(project_id)?;
        if discovery.approved_revision == 0
            || discovery.approved_revision != discovery.latest_revision()
        {
            return Err(EngineError::RunPreflight(
                "latest brief must be approved".into(),
            ));
        }
        let plan = self.load_plan(project_id)?;
        if plan.active_scope_revision == 0 {
            return Err(EngineError::RunPreflight("approved plan required".into()));
        }
        let task = plan
            .active_tasks
            .iter()
            .find(|task| task.logical_id == logical_task_id)
            .ok_or_else(|| EngineError::RunPreflight("task outside active plan".into()))?;
        if task.status != "ready" || !task.unresolved_blockers.is_empty() {
            return Err(EngineError::RunPreflight(
                "task is not ready or has blockers".into(),
            ));
        }
        let project_path = dunce::canonicalize(&project.path)?;
        let git_root = PathBuf::from(git(&project_path, &["rev-parse", "--show-toplevel"])?);
        let git_root = dunce::canonicalize(git_root)?;
        reject_custom_checkout_filters(&git_root)?;
        let subdir = project_path
            .strip_prefix(&git_root)
            .map_err(|_| EngineError::RunPreflight("project folder is outside Git root".into()))?;
        let base_commit = git(&git_root, &["rev-parse", "HEAD"])?;
        if base_commit.len() != 40 && base_commit.len() != 64 {
            return Err(EngineError::Git("invalid HEAD revision".into()));
        }
        let group = format!("{:x}", Sha256::digest(project_id.as_bytes()));
        let worktree_root = worktree_base
            .join(&group[..16])
            .join(Uuid::new_v4().to_string());
        let checkout_path = worktree_root.join(subdir);
        let intent = self.store.record_checkout_intent(
            project_id,
            logical_task_id,
            &checkout_path.to_string_lossy(),
            &base_commit,
        )?;
        if let Some(parent) = worktree_root.parent() {
            fs::create_dir_all(parent)?;
        }
        let result = git(
            &git_root,
            &[
                "worktree",
                "add",
                "--detach",
                worktree_root.to_string_lossy().as_ref(),
                &base_commit,
            ],
        );
        if let Err(error) = result {
            self.store.update_checkout_state(&intent.id, "failed")?;
            return Err(error);
        }
        let actual_root =
            dunce::canonicalize(git(&checkout_path, &["rev-parse", "--show-toplevel"])?)?;
        if actual_root != dunce::canonicalize(&worktree_root)?
            || git(&checkout_path, &["rev-parse", "HEAD"])? != base_commit
        {
            self.store.update_checkout_state(&intent.id, "failed")?;
            return Err(EngineError::RunPreflight(
                "worktree identity check failed".into(),
            ));
        }
        self.store.update_checkout_state(&intent.id, "prepared")?;
        let grant = self.store.issue_agent_grant(
            project_id,
            logical_task_id,
            &checkout_path,
            allowed_operations,
            limits.token_ttl_seconds,
        )?;
        self.store
            .set_run_limits(&grant.run_id, limits.wall_seconds, limits.token_budget)?;
        let run = self.store.transition_run(&grant.run_id, 1, "starting")?;
        let checkout = self.store.get_checkout(&intent.id)?;
        let decisions = self.store.list_decisions(project_id)?;
        let packet = TaskPacket {
            schema_version: 1,
            project_id: project_id.into(),
            task_id: logical_task_id.into(),
            task_revision: task.revision,
            scope_revision: task.scope_revision,
            checkout_path: checkout_path.to_string_lossy().into_owned(),
            goal: task.outcome.clone(),
            review_feedback: self
                .store
                .latest_task_feedback(project_id, logical_task_id)?,
            acceptance_criteria: task
                .criteria
                .iter()
                .map(|criterion| PacketCriterion {
                    id: criterion.logical_id.clone(),
                    assertion: criterion.assertion.clone(),
                    verifier: criterion.verifier.clone(),
                    required_evidence: criterion.required_evidence.clone(),
                })
                .collect(),
            dependencies: plan
                .revisions
                .iter()
                .find(|revision| revision.scope_revision == Some(plan.active_scope_revision))
                .map(|revision| {
                    revision
                        .content
                        .dependencies
                        .iter()
                        .filter(|edge| {
                            edge.from_task_id == logical_task_id && edge.kind.as_str() == "blocks"
                        })
                        .map(|edge| edge.to_task_id.clone())
                        .collect()
                })
                .unwrap_or_default(),
            approved_decision_ids: decisions
                .into_iter()
                .filter(|decision| {
                    decision.status == "approved"
                        && decision
                            .task_logical_id
                            .as_deref()
                            .is_none_or(|id| id == logical_task_id)
                })
                .map(|decision| decision.id)
                .collect(),
            allowed_operations: allowed_operations.to_vec(),
            stop_conditions: vec![
                "scope_change".into(),
                "external_publish".into(),
                "secret_request".into(),
            ],
            limits,
        };
        self.store.save_run_packet(
            &run.id,
            &serde_json::to_value(&packet)
                .map_err(|error| EngineError::RunPreflight(error.to_string()))?,
        )?;
        Ok(RunStart {
            run,
            checkout,
            grant,
            packet,
        })
    }

    pub fn transition_agent_run(
        &mut self,
        run_id: &str,
        expected_revision: i64,
        to: &str,
    ) -> Result<RunRecord> {
        Ok(self.store.transition_run(run_id, expected_revision, to)?)
    }

    pub fn resume_blocked_task(
        &mut self,
        project_id: &str,
        logical_id: &str,
        expected_revision: i64,
    ) -> Result<()> {
        Ok(self
            .store
            .resume_blocked_task(project_id, logical_id, expected_revision, "owner")?)
    }

    /// Called once at desktop startup, never by the per-request API opener.
    pub fn reconcile_interrupted_runs(&mut self) -> Result<Vec<RecoveryRecord>> {
        let mut recovery = Vec::new();
        for run in self.store.list_open_runs()? {
            let path = Path::new(&run.checkout_path);
            let (head, changed) = if path.exists() {
                let head = git(path, &["rev-parse", "HEAD"]).ok();
                let count = git(path, &["status", "--porcelain=v1", "--untracked-files=all"])
                    .ok()
                    .map(|status| status.lines().filter(|line| !line.is_empty()).count());
                (head, count)
            } else {
                (None, None)
            };
            let summary = match changed {
                Some(count) => format!("changed entries: {count}"),
                None => "checkout missing or unreadable".into(),
            };
            let cursor = self.store.latest_run_cursor(&run.id)?;
            let checkpoint = self.store.checkpoint_and_interrupt(
                &run.id,
                run.revision,
                cursor,
                head.as_deref(),
                &summary,
            )?;
            recovery.push(RecoveryRecord {
                run_id: Some(run.id),
                checkout_id: None,
                checkout_path: run.checkout_path,
                state: "interrupted".into(),
                changed_entries: changed,
                checkpoint: Some(checkpoint),
            });
        }
        for checkout in self.store.list_pending_checkouts()? {
            let changed = if Path::new(&checkout.path).exists() {
                git(
                    Path::new(&checkout.path),
                    &["status", "--porcelain=v1", "--untracked-files=all"],
                )
                .ok()
                .map(|status| status.lines().filter(|line| !line.is_empty()).count())
            } else {
                None
            };
            recovery.push(RecoveryRecord {
                run_id: None,
                checkout_id: Some(checkout.id),
                checkout_path: checkout.path,
                state: checkout.state,
                changed_entries: changed,
                checkpoint: None,
            });
        }
        Ok(recovery)
    }

    pub fn evaluate_action(&mut self, run_id: &str, action: &ActionSpec) -> Result<PolicyOutcome> {
        if action.command_spec.trim().is_empty()
            || action.target.trim().is_empty()
            || action.effect_summary.trim().is_empty()
        {
            return Err(EngineError::RunPreflight(
                "complete action specification required".into(),
            ));
        }
        let digest = format!("{:x}", Sha256::digest(action.command_spec.as_bytes()));
        if !action.class.requires_approval() {
            let run = self.store.get_run(run_id)?;
            let target = safe_checkout_target(Path::new(&run.checkout_path), &action.target)?;
            self.store.record_local_action(
                run_id,
                action.class.as_str(),
                &target.to_string_lossy(),
                &digest,
            )?;
            return Ok(PolicyOutcome::Allowed);
        }
        let request = self.store.request_policy_action(
            run_id,
            action.class.as_str(),
            &action.target,
            &digest,
            &action.effect_summary,
        )?;
        match request.status.as_str() {
            "pending" => Ok(PolicyOutcome::Pending(request)),
            "denied" | "consumed" => Ok(PolicyOutcome::Denied(request)),
            "approved" => {
                self.store.consume_policy_action(
                    &request.id,
                    run_id,
                    action.class.as_str(),
                    &action.target,
                    &digest,
                )?;
                Ok(PolicyOutcome::Allowed)
            }
            _ => Err(EngineError::RunPreflight("invalid policy state".into())),
        }
    }

    pub fn resolve_action_request(
        &mut self,
        id: &str,
        expected_revision: i64,
        approve: bool,
        expires_at_unix: Option<i64>,
    ) -> Result<PolicyRequestRecord> {
        Ok(self.store.resolve_policy_request(
            id,
            expected_revision,
            approve,
            expires_at_unix,
            "owner",
        )?)
    }

    pub fn get_action_request(&self, id: &str) -> Result<PolicyRequestRecord> {
        Ok(self.store.get_policy_request(id)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BriefContent, CriterionSpec, PlanContent, TaskSpec};
    use tempfile::TempDir;

    fn run_git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn fixture() -> (TempDir, PathBuf, String) {
        let folder = tempfile::tempdir().unwrap();
        let repo = folder.path().join("repo");
        fs::create_dir(&repo).unwrap();
        run_git(&repo, &["init"]);
        fs::write(repo.join("README.md"), "base\n").unwrap();
        run_git(&repo, &["add", "README.md"]);
        run_git(
            &repo,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-m",
                "base",
            ],
        );
        let database = folder.path().join("portfolio.sqlite");
        let mut engine = ProjectEngine::open(&database).unwrap();
        let project = engine.import_existing(&repo, "Project").unwrap();
        let brief = BriefContent {
            idea: "App".into(),
            audience: "Owner".into(),
            problem: "Work scattered".into(),
            desired_outcome: "Working app".into(),
            ..Default::default()
        };
        engine.save_brief(&project.id, 0, &brief).unwrap();
        engine.approve_latest_brief(&project.id, 1).unwrap();
        let plan = PlanContent {
            tasks: vec![TaskSpec {
                id: "task".into(),
                epic_id: None,
                title: "Build".into(),
                outcome: "Feature works".into(),
                weight: 1,
                risk: "low".into(),
                criteria: vec![CriterionSpec {
                    id: "criterion".into(),
                    assertion: "Works".into(),
                    verifier: "owner".into(),
                    required_evidence: vec!["test".into()],
                }],
                verification_commands: vec!["cargo test".into()],
                deliverables: vec!["code".into()],
                context_links: vec![],
                estimate_band: "small".into(),
                owner_decision_triggers: vec![],
            }],
            ..Default::default()
        };
        engine.save_plan(&project.id, 0, &plan).unwrap();
        engine.approve_latest_plan(&project.id, 1).unwrap();
        (folder, database, project.id)
    }

    fn limits() -> RunLimits {
        RunLimits {
            wall_seconds: 3600,
            token_budget: 10_000,
            token_ttl_seconds: 900,
        }
    }
    fn ops() -> Vec<String> {
        vec![
            "task.get".into(),
            "task.submit".into(),
            "run.progress".into(),
        ]
    }

    #[test]
    fn preflight_requires_harness_approved_scope_and_limits() {
        let (folder, database, project_id) = fixture();
        let mut engine = ProjectEngine::open(&database).unwrap();
        let base = folder.path().join("worktrees");
        assert!(
            engine
                .prepare_agent_run_at(&project_id, "task", false, &ops(), limits(), &base)
                .is_err()
        );
        assert!(
            engine
                .prepare_agent_run_at(
                    &project_id,
                    "task",
                    true,
                    &ops(),
                    RunLimits {
                        wall_seconds: 0,
                        ..limits()
                    },
                    &base
                )
                .is_err()
        );
        assert!(
            engine
                .prepare_agent_run_at(&project_id, "missing", true, &ops(), limits(), &base)
                .is_err()
        );
        assert!(engine.store.list_open_runs().unwrap().is_empty());
        assert!(engine.store.list_pending_checkouts().unwrap().is_empty());
    }

    #[test]
    fn preflight_rejects_custom_checkout_filter_before_recording_intent() {
        let (folder, database, project_id) = fixture();
        run_git(
            &folder.path().join("repo"),
            &["config", "filter.hostile.smudge", "untrusted-command"],
        );
        let mut engine = ProjectEngine::open(&database).unwrap();
        let error = engine
            .prepare_agent_run_at(
                &project_id,
                "task",
                true,
                &ops(),
                limits(),
                &folder.path().join("worktrees"),
            )
            .unwrap_err();
        assert!(error.to_string().contains("custom Git checkout filter"));
        assert!(engine.store.list_pending_checkouts().unwrap().is_empty());
    }

    #[test]
    fn worktree_run_reconciles_partial_work_without_replaying_actions() {
        let (folder, database, project_id) = fixture();
        let mut engine = ProjectEngine::open(&database).unwrap();
        let start = engine
            .prepare_agent_run_at(
                &project_id,
                "task",
                true,
                &ops(),
                limits(),
                &folder.path().join("worktrees"),
            )
            .unwrap();
        assert_eq!(start.run.state, "starting");
        assert_eq!(start.checkout.state, "active");
        assert_eq!(start.packet.task_revision, 1);
        assert_eq!(start.packet.scope_revision, 1);
        assert_eq!(start.packet.acceptance_criteria.len(), 1);
        assert!(
            Path::new(&start.packet.checkout_path)
                .join("README.md")
                .exists()
        );
        assert!(
            engine
                .prepare_agent_run_at(
                    &project_id,
                    "task",
                    true,
                    &ops(),
                    limits(),
                    &folder.path().join("worktrees")
                )
                .is_err()
        );
        engine
            .transition_agent_run(&start.run.id, 2, "running")
            .unwrap();
        let progress = crate::AgentCommand {
            token: start.grant.token.clone(),
            project_id: project_id.clone(),
            run_id: start.run.id.clone(),
            operation: "run.progress".into(),
            idempotency_key: "progress1".into(),
            expected_revision: 2,
            target_id: None,
            payload: serde_json::json!({"status":"working","next_action":"checkpoint"}),
        };
        engine.agent_command(&progress).unwrap();
        fs::write(
            Path::new(&start.packet.checkout_path).join("partial.txt"),
            "unfinished",
        )
        .unwrap();
        drop(engine);
        let mut reopened = ProjectEngine::open(&database).unwrap();
        let records = reopened.reconcile_interrupted_runs().unwrap();
        assert_eq!(records.len(), 1);
        eprintln!(
            "RECOVERY_RECORD={}",
            serde_json::json!({"state":records[0].state,
            "changed_entries":records[0].changed_entries,
            "checkpoint_state":records[0].checkpoint.as_ref().unwrap().run_state,
            "event_cursor":records[0].checkpoint.as_ref().unwrap().event_cursor,
            "task_revision":records[0].checkpoint.as_ref().unwrap().task_revision})
        );
        assert_eq!(records[0].changed_entries, Some(1));
        assert_eq!(records[0].checkpoint.as_ref().unwrap().run_state, "running");
        assert_eq!(records[0].checkpoint.as_ref().unwrap().event_cursor, 1);
        assert_eq!(
            reopened.store.get_run(&start.run.id).unwrap().state,
            "interrupted"
        );
        assert!(
            Path::new(&start.packet.checkout_path)
                .join("partial.txt")
                .exists()
        );
        assert!(reopened.reconcile_interrupted_runs().unwrap().is_empty());
        assert!(reopened.agent_command(&progress).is_err());
        assert_eq!(
            reopened.store.latest_run_cursor(&progress.run_id).unwrap(),
            1
        );
        assert_eq!(
            reopened.load_plan(&project_id).unwrap().active_tasks[0].status,
            "blocked"
        );
        reopened
            .resume_blocked_task(&project_id, "task", 3)
            .unwrap();
        assert_eq!(
            reopened.load_plan(&project_id).unwrap().active_tasks[0].status,
            "ready"
        );
    }

    #[test]
    fn policy_denial_survives_restart_and_approval_is_exact_and_single_use() {
        let (folder, database, project_id) = fixture();
        let mut engine = ProjectEngine::open(&database).unwrap();
        let start = engine
            .prepare_agent_run_at(
                &project_id,
                "task",
                true,
                &ops(),
                limits(),
                &folder.path().join("worktrees"),
            )
            .unwrap();
        engine
            .transition_agent_run(&start.run.id, 2, "running")
            .unwrap();
        let risky = ActionSpec {
            class: ActionClass::RewriteGit,
            target: "repo history".into(),
            command_spec: "git reset --hard abc".into(),
            effect_summary: "Rewrite repository history".into(),
        };
        let request = match engine.evaluate_action(&start.run.id, &risky).unwrap() {
            PolicyOutcome::Pending(request) => request,
            _ => panic!("expected pending"),
        };
        assert_eq!(engine.global_inbox().unwrap()[0].kind, "policy");
        engine
            .resolve_action_request(&request.id, 1, false, None)
            .unwrap();
        assert!(matches!(
            engine.evaluate_action(&start.run.id, &risky).unwrap(),
            PolicyOutcome::Denied(_)
        ));
        drop(engine);
        let mut reopened = ProjectEngine::open(&database).unwrap();
        assert!(matches!(
            reopened.evaluate_action(&start.run.id, &risky).unwrap(),
            PolicyOutcome::Denied(_)
        ));
        let publish = ActionSpec {
            class: ActionClass::ExternalPublish,
            target: "https://example.invalid/app".into(),
            command_spec: "publish artifact abc".into(),
            effect_summary: "Publish a build".into(),
        };
        let request = match reopened.evaluate_action(&start.run.id, &publish).unwrap() {
            PolicyOutcome::Pending(request) => request,
            _ => panic!("expected pending"),
        };
        let expiry = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            + 3600;
        reopened
            .resolve_action_request(&request.id, 1, true, Some(expiry))
            .unwrap();
        assert!(matches!(
            reopened.evaluate_action(&start.run.id, &publish).unwrap(),
            PolicyOutcome::Allowed
        ));
        assert!(matches!(
            reopened.evaluate_action(&start.run.id, &publish).unwrap(),
            PolicyOutcome::Denied(_)
        ));
        let altered = ActionSpec {
            command_spec: "publish artifact different".into(),
            ..publish
        };
        assert!(matches!(
            reopened.evaluate_action(&start.run.id, &altered).unwrap(),
            PolicyOutcome::Pending(_)
        ));
    }

    #[test]
    fn local_actions_are_checkout_scoped_and_run_state_is_revision_checked() {
        let (folder, database, project_id) = fixture();
        let mut engine = ProjectEngine::open(&database).unwrap();
        let start = engine
            .prepare_agent_run_at(
                &project_id,
                "task",
                true,
                &ops(),
                limits(),
                &folder.path().join("worktrees"),
            )
            .unwrap();
        assert!(
            engine
                .transition_agent_run(&start.run.id, 1, "running")
                .is_err()
        );
        assert!(
            engine
                .transition_agent_run(&start.run.id, 2, "completed")
                .is_err()
        );
        engine
            .transition_agent_run(&start.run.id, 2, "running")
            .unwrap();
        let local = ActionSpec {
            class: ActionClass::EditCheckout,
            target: Path::new(&start.packet.checkout_path)
                .join("new.txt")
                .to_string_lossy()
                .into_owned(),
            command_spec: "write new.txt".into(),
            effect_summary: "Edit task file".into(),
        };
        assert!(matches!(
            engine.evaluate_action(&start.run.id, &local).unwrap(),
            PolicyOutcome::Allowed
        ));
        let outside = ActionSpec {
            target: folder
                .path()
                .join("outside.txt")
                .to_string_lossy()
                .into_owned(),
            ..local
        };
        assert!(engine.evaluate_action(&start.run.id, &outside).is_err());
        assert!(
            engine
                .transition_agent_run(&start.run.id, 3, "queued")
                .is_err()
        );
        let partial = Path::new(&start.packet.checkout_path).join("failed-run.txt");
        fs::write(&partial, "keep this work").unwrap();
        engine
            .transition_agent_run(&start.run.id, 3, "failed")
            .unwrap();
        assert_eq!(fs::read_to_string(partial).unwrap(), "keep this work");
    }

    #[test]
    fn approving_new_scope_interrupts_run_and_revokes_pending_actions() {
        let (folder, database, project_id) = fixture();
        let mut engine = ProjectEngine::open(&database).unwrap();
        let start = engine
            .prepare_agent_run_at(
                &project_id,
                "task",
                true,
                &ops(),
                limits(),
                &folder.path().join("worktrees"),
            )
            .unwrap();
        engine
            .transition_agent_run(&start.run.id, 2, "running")
            .unwrap();
        let action = ActionSpec {
            class: ActionClass::Production,
            target: "production service".into(),
            command_spec: "deploy commit abc".into(),
            effect_summary: "Deploy a build".into(),
        };
        let request = match engine.evaluate_action(&start.run.id, &action).unwrap() {
            PolicyOutcome::Pending(request) => request,
            _ => panic!("expected pending"),
        };
        let plan = engine.load_plan(&project_id).unwrap().latest_content();
        engine.save_plan(&project_id, 1, &plan).unwrap();
        engine.approve_latest_plan(&project_id, 2).unwrap();
        assert_eq!(
            engine.store.get_run(&start.run.id).unwrap().state,
            "interrupted"
        );
        assert_eq!(
            engine.store.get_policy_request(&request.id).unwrap().status,
            "denied"
        );
        let expiry = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            + 900;
        assert!(
            engine
                .resolve_action_request(&request.id, 2, true, Some(expiry))
                .is_err()
        );
        assert!(
            engine
                .agent_command(&crate::AgentCommand {
                    token: start.grant.token,
                    project_id: project_id.clone(),
                    run_id: start.run.id,
                    operation: "task.get".into(),
                    idempotency_key: String::new(),
                    expected_revision: 0,
                    target_id: None,
                    payload: serde_json::json!({})
                })
                .is_err()
        );
        assert!(Path::new(&start.packet.checkout_path).exists());
    }

    #[test]
    fn opencode_permission_event_is_deduplicated_and_waits_for_owner() {
        use pipeline_adapters::{EventKind, NormalizedEvent, OpenCodeAdapter, RunHandle};

        let (folder, database, project_id) = fixture();
        let mut engine = ProjectEngine::open(&database).unwrap();
        let start = engine
            .prepare_agent_run_at(
                &project_id,
                "task",
                true,
                &ops(),
                limits(),
                &folder.path().join("worktrees"),
            )
            .unwrap();
        engine
            .store
            .attach_adapter_session(&start.run.id, start.run.revision, "ses_fixture")
            .unwrap();
        engine
            .transition_agent_run(&start.run.id, start.run.revision, "running")
            .unwrap();
        let run = RunHandle {
            run_id: start.run.id.clone(),
            session_id: "ses_fixture".into(),
            checkout: PathBuf::from(start.packet.checkout_path),
        };
        let event = NormalizedEvent {
            external_id: Some("evt_fixture".into()),
            session_id: "ses_fixture".into(),
            kind: EventKind::PermissionRequest,
            summary: "OpenCode requests bash permission".into(),
            permission_id: Some("per_fixture".into()),
            permission_class: Some("unknown_external".into()),
            permission_target: Some("per_fixture: git push".into()),
            permission_digest_material: Some("exact action".into()),
        };
        let adapter = OpenCodeAdapter::connect("http://127.0.0.1:1/", None, "1.18.30").unwrap();
        let first = engine
            .observe_opencode_event(&adapter, &run, event.clone())
            .unwrap();
        assert!(matches!(
            first.permission,
            Some(crate::OpenCodePermissionOutcome::Pending(_))
        ));
        assert_eq!(
            engine.store.get_run(&run.run_id).unwrap().state,
            "waiting_for_input"
        );
        assert_eq!(engine.store.latest_run_cursor(&run.run_id).unwrap(), 1);
        let repeated = engine
            .observe_opencode_event(&adapter, &run, event)
            .unwrap();
        assert!(repeated.sequence.is_none());
        assert!(repeated.permission.is_none());
        assert_eq!(engine.store.latest_run_cursor(&run.run_id).unwrap(), 1);
    }

    #[test]
    #[ignore = "one approved external free-model prompt; run explicitly for P13 live verification"]
    fn live_opencode_model_permission_smoke() {
        use pipeline_adapters::{
            AgentAdapter, EventKind, Health, OpenCodeServer, RunHandle, RunRequest,
        };
        let executable = std::env::var("OPENCODE_TEST_EXECUTABLE").unwrap();
        let (folder, database, project_id) = fixture();
        let repo = folder.path().join("repo");
        fs::write(
            repo.join("opencode.json"),
            serde_json::json!({"$schema":"https://opencode.ai/config.json",
                "model":"openrouter/cohere/north-mini-code:free",
                "permission":{"*":"deny","bash":"ask"}})
            .to_string(),
        )
        .unwrap();
        run_git(&repo, &["add", "opencode.json"]);
        run_git(
            &repo,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-m",
                "permission fixture",
            ],
        );
        let mut engine = ProjectEngine::open(&database).unwrap();
        let start = engine
            .prepare_agent_run_at(
                &project_id,
                "task",
                true,
                &ops(),
                RunLimits {
                    wall_seconds: 120,
                    token_budget: 2_000,
                    token_ttl_seconds: 900,
                },
                &folder.path().join("worktrees"),
            )
            .unwrap();
        let checkout = PathBuf::from(&start.packet.checkout_path);
        let server = OpenCodeServer::launch(Path::new(&executable), &checkout).unwrap();
        let adapter = server.adapter().unwrap();
        let report = adapter.probe();
        assert_eq!(report.health, Health::Ready);
        let run: RunHandle = adapter
            .start(RunRequest {
                run_id: start.run.id.clone(),
                checkout: checkout.clone(),
            })
            .unwrap();
        engine
            .store
            .attach_adapter_session(&run.run_id, start.run.revision, &run.session_id)
            .unwrap();
        engine
            .transition_agent_run(&run.run_id, start.run.revision, "running")
            .unwrap();
        let stream_adapter = server.adapter().unwrap();
        let stream_run = run.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        let stream = std::thread::spawn(move || {
            stream_adapter.stream_events(&stream_run, |event| {
                let stop = matches!(
                    event.kind,
                    EventKind::PermissionRequest | EventKind::Error | EventKind::Idle
                );
                sender.send(event).unwrap();
                !stop
            })
        });
        let connected = receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        assert_eq!(connected.kind, EventKind::Connected);
        adapter
            .steer_with_model(
                &run,
                "For this single protocol smoke, call the bash tool exactly once with `git status --short` in this repository. Do not edit files, access the network, or call any other tool. If permission is requested, wait for it.",
                "openrouter",
                "cohere/north-mini-code:free",
            )
            .unwrap();
        let mut event_kinds = Vec::new();
        let mut permission = None;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        for _ in 0..500 {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            let Ok(event) =
                receiver.recv_timeout(remaining.min(std::time::Duration::from_secs(30)))
            else {
                continue;
            };
            event_kinds.push(format!("{:?}", event.kind));
            let result = engine
                .observe_opencode_event(&adapter, &run, event)
                .unwrap();
            if let Some(crate::OpenCodePermissionOutcome::Pending(request_id)) = result.permission {
                permission = Some((request_id, result.event.permission_id.unwrap()));
                break;
            }
            if matches!(result.event.kind, EventKind::Error | EventKind::Idle) {
                break;
            }
        }
        let (request_id, permission_id) = permission.expect("model did not request permission");
        let request = engine.store.get_policy_request(&request_id).unwrap();
        assert_eq!(request.status, "pending");
        engine
            .resolve_action_request(&request_id, request.revision, false, None)
            .unwrap();
        let outcome = engine
            .resolve_pending_opencode_permission(&adapter, &run, &permission_id)
            .unwrap();
        assert!(matches!(outcome, crate::OpenCodePermissionOutcome::Denied));
        let _ = adapter.stop(&run);
        let _ = adapter.delete_session(&run);
        let _ = stream.join().unwrap();
        let cursor = engine.store.latest_run_cursor(&run.run_id).unwrap();
        eprintln!(
            "P13_MODEL_SMOKE={}",
            serde_json::json!({"model":"openrouter/cohere/north-mini-code:free",
                "model_price_usd":0,"prompt_count":1,"event_kinds":event_kinds,
                "permission_requested":true,"policy_status":"denied",
                "harness_reply":"reject","run_event_cursor":cursor})
        );
    }
}
