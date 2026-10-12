//! Isolated native screenshot fixture; writes only through ProjectEngine services.
use pipeline_engine::{
    BriefContent, CriterionSpec, MilestoneSpec, PlanContent, ProjectEngine, TaskSpec,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::env::var_os("PIPELINE_UX05_SEED_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("target/ux05-native"));
    std::fs::create_dir_all(&data)?;
    let project_path = data.join("sample-project");
    std::fs::create_dir_all(&project_path)?;
    let mut engine = ProjectEngine::open(data.join("portfolio.sqlite"))?;
    if !engine
        .list_overviews()?
        .iter()
        .any(|item| item.name == "Sample product")
    {
        let project = engine.import_existing(&project_path, "Sample product")?;
        engine.save_brief(
            &project.id,
            0,
            &BriefContent {
                idea: "A calm product dashboard".into(),
                audience: "Independent makers".into(),
                problem: "Project updates are scattered".into(),
                desired_outcome: "See the next useful action".into(),
                ..Default::default()
            },
        )?;
    }
    let second_path = data.join("second-project");
    std::fs::create_dir_all(&second_path)?;
    if !engine
        .list_overviews()?
        .iter()
        .any(|item| item.name == "Second sample")
    {
        let project = engine.import_existing(&second_path, "Second sample")?;
        engine.save_brief(
            &project.id,
            0,
            &BriefContent {
                idea: "A separate owner project".into(),
                audience: "Small teams".into(),
                problem: "Scattered feedback".into(),
                desired_outcome: "One review queue".into(),
                ..Default::default()
            },
        )?;
    }
    let first = engine
        .list_overviews()?
        .into_iter()
        .find(|item| item.name == "Sample product")
        .ok_or("sample project missing")?;
    if engine.load_plan(&first.id)?.revisions.is_empty() {
        engine.save_plan(
            &first.id,
            0,
            &PlanContent {
                milestones: vec![MilestoneSpec {
                    id: "first-milestone".into(),
                    title: "First milestone".into(),
                    outcome: "Draft outcome".into(),
                    task_ids: vec!["first-task".into()],
                }],
                tasks: vec![TaskSpec {
                    id: "first-task".into(),
                    epic_id: None,
                    title: "First task".into(),
                    outcome: "Draft work".into(),
                    weight: 1,
                    risk: "low".into(),
                    criteria: vec![CriterionSpec {
                        id: "first-criterion".into(),
                        assertion: "Works".into(),
                        verifier: "owner".into(),
                        required_evidence: vec!["test".into()],
                    }],
                    verification_commands: vec![],
                    deliverables: vec![],
                    context_links: vec![],
                    estimate_band: "small".into(),
                    owner_decision_triggers: vec![],
                }],
                ..Default::default()
            },
        )?;
    }
    if std::env::args().any(|arg| arg == "--advance-first-brief") {
        let project = engine
            .list_overviews()?
            .into_iter()
            .find(|item| item.name == "Sample product")
            .ok_or("sample project missing")?;
        let view = engine.load_discovery(&project.id)?;
        let mut content = view.latest_content();
        content.idea.push_str(" (changed outside preview)");
        engine.save_brief(&project.id, view.latest_revision(), &content)?;
    }
    if std::env::args().any(|arg| arg == "--inspect") {
        for project in engine.list_overviews()? {
            let brief = engine.load_discovery(&project.id)?;
            let plan = engine.load_plan(&project.id)?;
            let activity = engine.load_review(&project.id)?.activity;
            println!(
                "{}: brief_revision={} idea={:?} plan_revision={} task_title={:?} activity_count={}",
                project.name,
                brief.latest_revision(),
                brief.latest_content().idea,
                plan.latest_revision(),
                plan.latest_content()
                    .tasks
                    .first()
                    .map(|task| task.title.as_str()),
                activity.len()
            );
        }
    }
    Ok(())
}
