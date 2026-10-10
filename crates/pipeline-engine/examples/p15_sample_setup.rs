//! Create an isolated, intentionally failing task for the P15 Windows journey.
//! Project and plan state are written only through ProjectEngine.
use pipeline_engine::{BriefContent, CriterionSpec, PlanContent, ProjectEngine, TaskSpec};
use std::path::Path;
use std::process::Command;

fn git(path: &Path, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let database = args.next().ok_or("pass disposable database path")?;
    let folder = args.next().ok_or("pass new disposable project path")?;
    let folder = Path::new(&folder);
    if folder.exists() {
        return Err("disposable project path already exists".into());
    }
    std::fs::create_dir_all(folder.join("src"))?;
    std::fs::write(
        folder.join("Cargo.toml"),
        "[package]\nname = \"p15-addition\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )?;
    std::fs::write(folder.join(".gitignore"), "target/\nCargo.lock\n")?;
    std::fs::write(
        folder.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn addition() { assert_eq!(super::add(2, 3), 5); }\n}\n",
    )?;
    git(folder, &["init"])?;
    git(folder, &["config", "user.name", "Project Pipeline Fixture"])?;
    git(folder, &["config", "user.email", "fixture@example.invalid"])?;
    git(folder, &["add", ".gitignore", "Cargo.toml", "src/lib.rs"])?;
    git(folder, &["commit", "-m", "Create failing addition sample"])?;
    let mut engine = ProjectEngine::open(&database)?;
    let project = engine.import_existing(folder, "P15 disposable addition sample")?;
    let brief = BriefContent {
        idea: "Validate Pi RPC agent task flow".into(),
        audience: "Project Pipeline owner".into(),
        problem: "The Pi adapter needs an end-to-end Windows task".into(),
        desired_outcome: "A traced agent fix with passing test and review evidence".into(),
        ..Default::default()
    };
    engine.save_brief(&project.id, 0, &brief)?;
    engine.approve_latest_brief(&project.id, 1)?;
    let plan = PlanContent {
        tasks: vec![TaskSpec {
            id: "fix-add".into(),
            epic_id: None,
            title: "Fix addition".into(),
            outcome: "Correct add to return the sum and pass the included test".into(),
            weight: 1,
            risk: "low".into(),
            criteria: vec![CriterionSpec {
                id: "addition-test".into(),
                assertion: "The included addition test passes".into(),
                verifier: "owner".into(),
                required_evidence: vec!["test-log".into(), "diff".into()],
            }],
            verification_commands: vec!["cargo test".into()],
            deliverables: vec!["Corrected src/lib.rs".into()],
            context_links: vec![],
            estimate_band: "small".into(),
            owner_decision_triggers: vec![],
        }],
        ..Default::default()
    };
    engine.save_plan(&project.id, 0, &plan)?;
    engine.approve_latest_plan(&project.id, 1)?;
    println!(
        "project_id={} task_id=fix-add database={} folder={}",
        project.id,
        database,
        folder.display()
    );
    Ok(())
}
