//! Deterministic navigation guidance. This module never authorizes or performs a write.
use crate::{DiscoveryView, PlanView, ProjectOverview, ReviewView, TaskRunReview};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextDestination {
    Brief,
    Plan,
    Decisions,
    Runs,
    Overview,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextAction {
    pub title: String,
    pub reason: String,
    pub prerequisite: String,
    pub alternative: String,
    pub destination: NextDestination,
    pub source_ids: Vec<String>,
}

pub struct RecommendationSnapshot<'a> {
    pub project: &'a ProjectOverview,
    pub discovery: &'a DiscoveryView,
    pub plan: &'a PlanView,
    pub review: &'a ReviewView,
    pub runs: &'a [TaskRunReview],
    /// Only true after a provider availability check has actually failed.
    pub provider_offline: bool,
}

fn action(
    title: impl Into<String>,
    reason: impl Into<String>,
    prerequisite: impl Into<String>,
    alternative: impl Into<String>,
    destination: NextDestination,
    source_ids: Vec<String>,
) -> NextAction {
    NextAction {
        title: title.into(),
        reason: reason.into(),
        prerequisite: prerequisite.into(),
        alternative: alternative.into(),
        destination,
        source_ids,
    }
}

pub fn recommend_next(s: &RecommendationSnapshot<'_>) -> NextAction {
    let project_id = &s.project.id;
    // Contradictory persisted records must be inspected rather than resolved by guesswork.
    if s.plan.active_scope_revision > s.plan.latest_revision()
        || s.discovery.approved_revision > s.discovery.latest_revision()
        || (s.plan.active_scope_revision > 0 && s.discovery.approved_revision == 0)
        || (s.project.verified_completion_basis_points > 0 && s.plan.active_scope_revision == 0)
    {
        return action(
            "Inspect inconsistent project records",
            "Approval or progress records disagree with the current revisions.",
            "Do not treat the displayed progress as verified until the records are reconciled.",
            "Open the activity history in Advanced.",
            NextDestination::Overview,
            vec![project_id.clone()],
        );
    }
    let mut inbox = s.review.inbox.iter().collect::<Vec<_>>();
    inbox.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| a.kind.cmp(&b.kind))
            .then_with(|| a.subject_id.cmp(&b.subject_id))
    });
    if let Some(item) = inbox.iter().find(|item| item.kind == "policy") {
        return action(
            &item.title,
            format!("An exact action request is waiting: {}", item.detail),
            "Inspect its scope, target and current revision before deciding.",
            "Continue reviewing unrelated work.",
            NextDestination::Decisions,
            vec![item.subject_id.clone()],
        );
    }
    if let Some(item) = inbox.iter().find(|item| {
        item.kind == "task"
            && s.plan
                .active_tasks
                .iter()
                .any(|task| task.logical_id == item.subject_id && task.status == "review")
    }) {
        let evidence = s
            .runs
            .iter()
            .find(|run| run.task_id == item.subject_id && run.state == "review");
        let evidence_note = match evidence {
            Some(run) if !run.packet_verified => {
                " Run packet verification is missing or inconsistent."
            }
            Some(run) if run.tests.iter().any(|test| test.exit_code != Some(0)) => {
                " One or more recorded checks did not pass or have no exit result."
            }
            Some(run) if run.tests.is_empty() => " No test result is recorded for the run.",
            Some(_) => " Recorded checks are present; inspect linked logs and required criteria.",
            None => " No matching run evidence is available; inspect the task record.",
        };
        return action(
            &item.title,
            format!(
                "Work is waiting for owner review: {}{}",
                item.detail, evidence_note
            ),
            "Inspect checks and required evidence before accepting or requesting changes.",
            "Review another decision or the project plan.",
            NextDestination::Plan,
            vec![item.subject_id.clone()],
        );
    }
    if let Some(run) = s
        .runs
        .iter()
        .find(|run| run.state == "review" && run.submission.is_some())
    {
        return action(
            "Review submitted work",
            "A run has a submission, but no matching inbox item was found.",
            "Verify the task and evidence state before acceptance.",
            "Inspect the run evidence in Advanced.",
            NextDestination::Runs,
            vec![run.run_id.clone(), run.task_id.clone()],
        );
    }
    if let Some(item) = inbox.iter().find(|item| item.kind == "decision") {
        return action(
            &item.title,
            format!("A decision is required: {}", item.detail),
            "Review the alternatives and current revision.",
            "Continue with unrelated approved work.",
            NextDestination::Decisions,
            vec![item.subject_id.clone()],
        );
    }
    if let Some(item) = inbox.iter().find(|item| item.kind == "task") {
        return action(
            &item.title,
            format!("The task has a recorded blocker: {}", item.detail),
            "Resolve its dependency or decision before starting it.",
            "Inspect other approved work in the plan.",
            NextDestination::Plan,
            vec![item.subject_id.clone()],
        );
    }
    if let Some(run) = s
        .runs
        .iter()
        .find(|run| run.state == "interrupted" || run.state == "failed")
    {
        return action(
            "Inspect interrupted work",
            format!(
                "Run {} is {}. Its saved evidence may still be available.",
                run.run_id, run.state
            ),
            "Inspect the last verified event before retrying or discarding work.",
            "Continue with another approved task after preflight.",
            NextDestination::Runs,
            vec![run.run_id.clone(), run.task_id.clone()],
        );
    }
    if let Some(run) = s.runs.iter().find(|run| {
        matches!(
            run.state.as_str(),
            "queued" | "starting" | "running" | "waiting_for_input"
        )
    }) {
        return action(
            "Inspect current work",
            format!("Run {} is recorded as {}.", run.run_id, run.state),
            "Check live adapter status before assuming it is still working.",
            "Review the project plan.",
            NextDestination::Runs,
            vec![run.run_id.clone()],
        );
    }
    if let Some(item) = inbox.iter().find(|item| item.kind == "brief") {
        return action(
            &item.title,
            "A saved product direction needs owner review.",
            "Approve only the exact revision after reading it.",
            "Continue editing the draft.",
            NextDestination::Brief,
            vec![item.subject_id.clone()],
        );
    }
    if let Some(item) = inbox.iter().find(|item| item.kind == "plan") {
        return action(
            &item.title,
            "A saved work plan needs owner review.",
            "Approve only the exact revision after checking scope and dependencies.",
            "Continue editing the draft.",
            NextDestination::Plan,
            vec![item.subject_id.clone()],
        );
    }
    if s.discovery.approved_revision == 0 {
        return action(
            "Shape the product direction",
            "No product direction has an approved revision.",
            "Save a draft before approval.",
            "Import or inspect the existing project folder.",
            NextDestination::Brief,
            vec![project_id.clone()],
        );
    }
    if s.plan.active_scope_revision == 0 {
        return action(
            "Prepare approved work",
            "No work plan has an active approved scope.",
            "Review the draft plan and its dependencies.",
            "Inspect the approved product direction.",
            NextDestination::Plan,
            vec![project_id.clone()],
        );
    }
    if let Some(task) = s
        .plan
        .active_tasks
        .iter()
        .find(|task| task.status == "ready" && task.unresolved_blockers.is_empty())
    {
        if s.provider_offline {
            return action(
                "Continue planning while AI is unavailable",
                "A provider availability check failed; approved work remains saved.",
                "Reconnect and run a fresh preflight before starting agent work.",
                "Inspect the ready task manually.",
                NextDestination::Plan,
                vec![task.id.clone()],
            );
        }
        return action(
            format!("Review ready work: {}", task.title),
            "This approved task is marked ready with no recorded blockers.",
            "Starting still requires fresh provider, model, limit, policy and one-run preflight.",
            "Choose another approved task in the plan.",
            NextDestination::Plan,
            vec![task.id.clone()],
        );
    }
    if let Some(task) = s
        .plan
        .active_tasks
        .iter()
        .find(|task| task.status == "blocked" || !task.unresolved_blockers.is_empty())
    {
        return action(
            format!("Inspect blocker: {}", task.title),
            "The task is blocked in the approved work record.",
            "Resolve its recorded dependency or decision before starting it.",
            "Inspect other eligible work in the plan.",
            NextDestination::Plan,
            vec![task.id.clone()],
        );
    }
    if s.plan
        .active_tasks
        .iter()
        .all(|task| task.status == "accepted")
        && !s.plan.active_tasks.is_empty()
    {
        return action(
            "Review completed scope",
            "Every task in the active scope is accepted.",
            "Check the evidence and scope revision before planning release work.",
            "Inspect the project activity history.",
            NextDestination::Plan,
            vec![project_id.clone()],
        );
    }
    action(
        "Review the work plan",
        "No higher-priority owner action is recorded.",
        "Inspect current task states and dependencies.",
        "Open the project activity history.",
        NextDestination::Plan,
        vec![project_id.clone()],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BriefContent, BriefRecord, InboxItem, PlanContent, PlanRecord, TaskRecord};
    use pipeline_domain::{ProjectHealth, ProjectStage};

    struct Fixture {
        project: ProjectOverview,
        discovery: DiscoveryView,
        plan: PlanView,
        review: ReviewView,
        runs: Vec<TaskRunReview>,
        offline: bool,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                project: ProjectOverview {
                    id: "project".into(),
                    name: "Sample".into(),
                    path: "sample".into(),
                    git_root: None,
                    stage: ProjectStage::Idea,
                    health: ProjectHealth::NeedsInput,
                    verified_completion_basis_points: 0,
                    blocked_count: 0,
                    next_owner_action: String::new(),
                },
                discovery: DiscoveryView {
                    briefs: vec![],
                    research: vec![],
                    approved_revision: 0,
                    changes: vec![],
                },
                plan: PlanView {
                    revisions: vec![],
                    active_scope_revision: 0,
                    active_tasks: vec![],
                },
                review: ReviewView {
                    decisions: vec![],
                    policy_requests: vec![],
                    inbox: vec![],
                    activity: vec![],
                },
                runs: vec![],
                offline: false,
            }
        }
        fn approved(mut self) -> Self {
            self.discovery.briefs.push(BriefRecord {
                id: "brief".into(),
                project_id: "project".into(),
                revision: 1,
                status: "approved".into(),
                content: BriefContent::default(),
                created_at: String::new(),
            });
            self.discovery.approved_revision = 1;
            self.plan.revisions.push(PlanRecord {
                id: "plan".into(),
                project_id: "project".into(),
                revision: 1,
                status: "approved".into(),
                content_hash: String::new(),
                scope_revision: Some(1),
                content: PlanContent::default(),
                created_at: String::new(),
            });
            self.plan.active_scope_revision = 1;
            self
        }
        fn task(mut self, status: &str) -> Self {
            self.plan.active_tasks.push(TaskRecord {
                id: "task-row".into(),
                logical_id: "task".into(),
                title: "Build feature".into(),
                outcome: "Feature works".into(),
                status: status.into(),
                weight: 1,
                revision: 1,
                scope_revision: 1,
                criteria: vec![],
                unresolved_blockers: vec![],
            });
            self
        }
        fn inbox(mut self, kind: &str) -> Self {
            let title = if kind == "task"
                && self
                    .plan
                    .active_tasks
                    .iter()
                    .any(|task| task.status == "blocked")
            {
                "Blocked task".to_owned()
            } else {
                format!("Review {kind}")
            };
            self.review.inbox.push(InboxItem {
                project_id: "project".into(),
                kind: kind.into(),
                subject_id: if kind == "task" {
                    "task".into()
                } else {
                    format!("{kind}-record")
                },
                title,
                detail: "Saved item".into(),
                priority: 0,
                revision: 1,
            });
            self
        }
        fn run(mut self, state: &str) -> Self {
            self.runs.push(TaskRunReview {
                run_id: "run".into(),
                task_id: "task".into(),
                state: state.into(),
                checkout_path: String::new(),
                packet: None,
                packet_sha256: None,
                packet_verified: false,
                submission: (state == "review").then(|| serde_json::json!({"submitted":true})),
                artifacts: vec![],
                tests: vec![],
                events: vec![],
            });
            self
        }
        fn next(&self) -> NextAction {
            recommend_next(&RecommendationSnapshot {
                project: &self.project,
                discovery: &self.discovery,
                plan: &self.plan,
                review: &self.review,
                runs: &self.runs,
                provider_offline: self.offline,
            })
        }
    }

    #[test]
    fn next_actions_cover_project_and_run_states() {
        let cases = [
            (
                Fixture::new(),
                NextDestination::Brief,
                "Shape the product direction",
            ),
            (
                Fixture::new().approved().task("ready"),
                NextDestination::Plan,
                "Review ready work: Build feature",
            ),
            (
                Fixture::new().approved().task("blocked"),
                NextDestination::Plan,
                "Inspect blocker: Build feature",
            ),
            (
                Fixture::new().approved().task("accepted"),
                NextDestination::Plan,
                "Review completed scope",
            ),
            (
                Fixture::new().approved().task("running").run("running"),
                NextDestination::Runs,
                "Inspect current work",
            ),
            (
                Fixture::new().approved().task("review").run("review"),
                NextDestination::Runs,
                "Review submitted work",
            ),
            (
                Fixture::new().approved().task("ready").run("interrupted"),
                NextDestination::Runs,
                "Inspect interrupted work",
            ),
            (
                Fixture::new().approved().task("ready").run("failed"),
                NextDestination::Runs,
                "Inspect interrupted work",
            ),
            (
                Fixture::new().approved().inbox("decision"),
                NextDestination::Decisions,
                "Review decision",
            ),
            (
                Fixture::new().approved().inbox("policy"),
                NextDestination::Decisions,
                "Review policy",
            ),
            (
                Fixture::new().approved().task("blocked").inbox("task"),
                NextDestination::Plan,
                "Blocked task",
            ),
            (
                Fixture::new()
                    .approved()
                    .task("review")
                    .inbox("task")
                    .run("review"),
                NextDestination::Plan,
                "Review task",
            ),
        ];
        for (fixture, destination, title) in cases {
            let action = fixture.next();
            assert_eq!(action.destination, destination, "{title}");
            assert_eq!(action.title, title);
            assert!(
                !action.reason.is_empty()
                    && !action.prerequisite.is_empty()
                    && !action.alternative.is_empty()
                    && !action.source_ids.is_empty()
            );
        }
    }

    #[test]
    fn planning_import_offline_and_stale_records_are_honest() {
        assert_eq!(
            Fixture::new().approved().next().title,
            "Review the work plan"
        );
        assert_eq!(
            Fixture::new().inbox("brief").next().destination,
            NextDestination::Brief
        );
        assert_eq!(
            Fixture::new().approved().inbox("plan").next().destination,
            NextDestination::Plan
        );
        let mut offline = Fixture::new().approved().task("ready");
        offline.offline = true;
        assert_eq!(
            offline.next().title,
            "Continue planning while AI is unavailable"
        );
        let mut stale = Fixture::new().approved().task("ready");
        stale.plan.active_scope_revision = 2;
        assert_eq!(stale.next().title, "Inspect inconsistent project records");
        let mut false_progress = Fixture::new();
        false_progress.project.verified_completion_basis_points = 5000;
        assert_eq!(
            false_progress.next().title,
            "Inspect inconsistent project records"
        );
        let review = Fixture::new()
            .approved()
            .task("review")
            .inbox("task")
            .run("review");
        assert!(
            review
                .next()
                .reason
                .contains("packet verification is missing")
        );
    }

    #[test]
    fn service_reads_persisted_project_and_routes_saved_brief() {
        let temp = tempfile::tempdir().unwrap();
        let mut engine = crate::ProjectEngine::open(temp.path().join("portfolio.sqlite")).unwrap();
        let project = engine
            .create_new(temp.path().join("source"), "Sample")
            .unwrap();
        assert_eq!(
            engine.next_action(&project.id).unwrap().destination,
            NextDestination::Brief
        );
        let brief = BriefContent {
            idea: "Build a useful app".into(),
            audience: "Owners".into(),
            problem: "Scattered planning".into(),
            desired_outcome: "One workspace".into(),
            ..Default::default()
        };
        engine.save_brief(&project.id, 0, &brief).unwrap();
        let saved = engine.next_action(&project.id).unwrap();
        assert_eq!(saved.destination, NextDestination::Brief);
        assert!(saved.title.contains("revision 1"));
        assert_eq!(saved.source_ids, vec![project.id]);
    }
}
