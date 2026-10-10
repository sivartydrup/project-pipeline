//! Disposable Git project for the P14 Windows owner journey.
use pipeline_domain::{BriefContent, CriterionSpec, PlanContent, TaskSpec};
use pipeline_store::Store;
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .arg("-c")
        .arg("core.hooksPath=NUL")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(format!("git failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args()
        .nth(1)
        .ok_or("pass an isolated data directory")?;
    let directory = Path::new(&directory);
    std::fs::create_dir_all(directory)?;
    let directory = std::fs::canonicalize(directory)?;
    let project = directory.join("addition-app");
    std::fs::create_dir_all(project.join("src"))?;
    std::fs::write(
        project.join("Cargo.toml"),
        "[package]\nname = \"addition-app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    std::fs::write(
        project.join("src/lib.rs"),
        "pub fn add(left: i32, right: i32) -> i32 {\n    left - right\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn adds_two_values() {\n        assert_eq!(super::add(2, 3), 5);\n    }\n}\n",
    )?;
    git(&project, &["init"])?;
    git(&project, &["add", "Cargo.toml", "src/lib.rs"])?;
    git(
        &project,
        &[
            "-c",
            "user.name=P14 Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-m",
            "Create failing addition sample",
        ],
    )?;
    let mut store = Store::open(directory.join("portfolio.sqlite"))?;
    if store.get_project("p14-sample")?.is_some() {
        return Ok(());
    }
    store.create_project(
        "p14-sample",
        "P14 addition sample",
        &project.to_string_lossy(),
        "fixture",
        "p14-create",
    )?;
    let brief = BriefContent {
        idea: "Small addition library".into(),
        audience: "Rust callers".into(),
        problem: "The add function subtracts, so its test fails".into(),
        desired_outcome: "add(2, 3) returns 5 with a passing test".into(),
        ..Default::default()
    };
    store.save_brief_revision("p14-brief", "p14-sample", 0, &brief, "fixture", "p14-brief")?;
    store.approve_brief_revision(
        "p14-sample",
        1,
        "p14-brief-approval",
        "owner",
        "p14-brief-approve",
    )?;
    let plan = PlanContent {
        tasks: vec![TaskSpec {
            id: "fix-add".into(),
            epic_id: None,
            title: "Fix addition behavior".into(),
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
            deliverables: vec!["src/lib.rs".into()],
            context_links: vec![],
            estimate_band: "small".into(),
            owner_decision_triggers: vec![],
        }],
        ..Default::default()
    };
    store.save_plan_revision("p14-plan", "p14-sample", 0, &plan, "fixture", "p14-plan")?;
    store.approve_plan_revision(
        "p14-sample",
        1,
        "p14-plan-approval",
        "owner",
        "p14-plan-approve",
    )?;
    println!("P14 fixture ready: {}", directory.display());
    Ok(())
}
