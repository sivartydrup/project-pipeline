use pipeline_domain::{
    BriefContent, CriterionSpec, DependencyKind, DependencySpec, EpicSpec, PlanContent, TaskSpec,
};
use pipeline_store::Store;
use std::collections::BTreeMap;

fn task(id: &str, title: &str, weight: i64) -> TaskSpec {
    TaskSpec {
        id: id.into(),
        epic_id: Some("launch-app".into()),
        title: title.into(),
        outcome: format!("{title} is verified"),
        weight,
        risk: "low".into(),
        criteria: vec![CriterionSpec {
            id: format!("{id}-check"),
            assertion: format!("{title} passes its review"),
            verifier: "owner".into(),
            required_evidence: vec!["test-log".into()],
        }],
        verification_commands: vec!["cargo test --workspace".into()],
        deliverables: vec![format!("{title} implementation")],
        context_links: Vec::new(),
        estimate_band: "small".into(),
        owner_decision_triggers: Vec::new(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::open_in_memory()?;
    store.create_project(
        "sample-project",
        "Sample app",
        "/sample/app",
        "owner",
        "create",
    )?;
    let brief = BriefContent {
        idea: "A sample app".into(),
        audience: "Solo builders".into(),
        problem: "Planning is scattered".into(),
        desired_outcome: "A verified launch".into(),
        ..Default::default()
    };
    store.save_brief_revision(
        "sample-brief",
        "sample-project",
        0,
        &brief,
        "owner",
        "brief",
    )?;
    store.approve_brief_revision(
        "sample-project",
        1,
        "sample-brief-approval",
        "owner",
        "brief-approve",
    )?;
    let plan = PlanContent {
        milestones: vec![],
        epics: vec![EpicSpec {
            id: "launch-app".into(),
            title: "Launch the app".into(),
            outcome: "A testable release candidate".into(),
        }],
        tasks: vec![
            task("desktop-shell", "Build desktop shell", 2),
            task("review-flow", "Build review flow", 3),
        ],
        dependencies: vec![DependencySpec {
            from_task_id: "review-flow".into(),
            to_task_id: "desktop-shell".into(),
            kind: DependencyKind::Blocks,
        }],
    };
    store.save_plan_revision(
        "sample-plan-v1",
        "sample-project",
        0,
        &plan,
        "owner",
        "plan",
    )?;
    store.approve_plan_revision(
        "sample-project",
        1,
        "sample-plan-approval",
        "owner",
        "plan-approve",
    )?;
    store.submit_task_for_review("sample-project", "desktop-shell", 1, "owner", "review")?;
    store.verify_criterion(
        "sample-project",
        "desktop-shell",
        "desktop-shell-check",
        2,
        &BTreeMap::from([("test-log".into(), "artifact://sample-shell-check".into())]),
        "owner",
        "verify",
    )?;
    store.accept_task("sample-project", "desktop-shell", 3, "owner", "accept")?;
    println!("{}", store.export_active_plan_json("sample-project")?);
    Ok(())
}
