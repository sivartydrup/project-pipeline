//! Creates isolated sample data for the P10 desktop review smoke test.
use pipeline_domain::{BriefContent, CriterionSpec, MilestoneSpec, PlanContent, TaskSpec};
use pipeline_store::{DecisionInput, Store};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args()
        .nth(1)
        .ok_or("pass an isolated data directory")?;
    let directory = Path::new(&directory);
    std::fs::create_dir_all(directory)?;
    let directory = std::fs::canonicalize(directory)?;
    let project_path = directory.join("sample-project");
    std::fs::create_dir_all(&project_path)?;
    let mut store = Store::open(directory.join("portfolio.sqlite"))?;
    if store.get_project("p10-sample")?.is_some() {
        return Ok(());
    }
    store.create_project(
        "p10-sample",
        "P10 UI sample",
        &project_path.to_string_lossy(),
        "fixture",
        "fixture-project",
    )?;
    let brief = BriefContent {
        idea: "Sample approval journey".into(),
        audience: "Owner".into(),
        problem: "Review decisions".into(),
        desired_outcome: "Traceable choice".into(),
        ..Default::default()
    };
    store.save_brief_revision(
        "sample-brief",
        "p10-sample",
        0,
        &brief,
        "fixture",
        "fixture-brief",
    )?;
    store.approve_brief_revision(
        "p10-sample",
        1,
        "sample-brief-approval",
        "owner",
        "fixture-brief-approval",
    )?;
    let plan = PlanContent {
        milestones: vec![MilestoneSpec {
            id: "sample-milestone".into(),
            title: "Sample delivery".into(),
            outcome: "Decision reviewed".into(),
            task_ids: vec!["sample-task".into()],
        }],
        tasks: vec![TaskSpec {
            id: "sample-task".into(),
            epic_id: None,
            title: "Choose storage".into(),
            outcome: "A recorded choice".into(),
            weight: 1,
            risk: "low".into(),
            criteria: vec![CriterionSpec {
                id: "sample-criterion".into(),
                assertion: "Choice is recorded".into(),
                verifier: "owner".into(),
                required_evidence: vec!["decision".into()],
            }],
            verification_commands: vec![],
            deliverables: vec![],
            context_links: vec![],
            estimate_band: "small".into(),
            owner_decision_triggers: vec![],
        }],
        ..Default::default()
    };
    store.save_plan_revision(
        "sample-plan",
        "p10-sample",
        0,
        &plan,
        "fixture",
        "fixture-plan",
    )?;
    store.approve_plan_revision(
        "p10-sample",
        1,
        "sample-plan-approval",
        "owner",
        "fixture-plan-approval",
    )?;
    let choice = DecisionInput {
        task_logical_id: Some("sample-task".into()),
        question: "Which storage strategy should the sample use?".into(),
        alternatives: vec!["SQLite".into(), "JSON files".into()],
        recommendation: "SQLite".into(),
        rationale: "Transactions and revision checks are required.".into(),
        evidence: vec!["artifact://storage-spike".into()],
        impact: "blocking".into(),
        ..Default::default()
    };
    store.create_decision(
        "sample-blocking",
        "p10-sample",
        &choice,
        "fixture-agent",
        "fixture-choice",
    )?;
    let mut second = choice.clone();
    second.question = "Which theme should the sample start with?".into();
    second.alternatives = vec!["Dark".into(), "Light".into()];
    second.recommendation = "Dark".into();
    second.impact = "low".into();
    second.task_logical_id = None;
    store.create_decision(
        "sample-theme",
        "p10-sample",
        &second,
        "fixture-agent",
        "fixture-theme",
    )?;
    store.resolve_decision(
        "p10-sample",
        "sample-theme",
        1,
        Some("Dark"),
        true,
        "owner",
        "fixture-theme-approval",
    )?;
    Ok(())
}
