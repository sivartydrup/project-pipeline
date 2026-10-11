use pipeline_store::Store;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from("target/p17-sample");
    std::fs::create_dir_all(&root)?;
    let root = dunce::canonicalize(root)?;
    let build = root.join("app.exe");
    let check = root.join("checks.log");
    std::fs::write(&build, b"sample build")?;
    std::fs::write(&check, b"all required checks passed")?;
    let mut store = Store::open_in_memory()?;
    store.create_project(
        "sample",
        "Sample release",
        root.to_str().unwrap(),
        "owner",
        "project",
    )?;
    let commit = "a".repeat(40);
    let mut release = store.create_release(
        "sample",
        "1.0.0",
        &commit,
        "sample production",
        "Restore previous artifact and data backup",
        "owner",
        "create",
    )?;
    let build_id = store.attach_release_artifact(
        "sample",
        &build,
        "build",
        Some(&commit),
        "owner",
        "build-file",
    )?;
    release = store.add_release_build(
        "sample",
        &release.id,
        release.revision,
        &build_id,
        "windows-x86_64",
        "owner",
        "build",
    )?;
    let check_id = store.attach_release_artifact(
        "sample",
        &check,
        "check-log",
        Some(&commit),
        "owner",
        "check-file",
    )?;
    release = store.set_release_check(
        "sample",
        &release.id,
        release.revision,
        "quality",
        true,
        "failed",
        Some(&check_id),
        "owner",
        "fail",
    )?;
    let failed_approval =
        store.approve_release("sample", &release.id, release.revision, "owner", "blocked");
    assert!(failed_approval.is_err());
    release = store.set_release_check(
        "sample",
        &release.id,
        release.revision,
        "quality",
        true,
        "passed",
        Some(&check_id),
        "owner",
        "pass",
    )?;
    release = store.approve_release("sample", &release.id, release.revision, "owner", "approve")?;
    let execution_approval = store.approve_release_execution(
        "sample",
        &release.id,
        release.revision,
        "owner",
        "execute",
    )?;
    release = store.record_manual_deployment(
        "sample",
        &release.id,
        release.revision,
        &execution_approval,
        "succeeded",
        "pending",
        "Manual procedure completed",
        "owner",
        "deploy",
    )?;
    let deployment_id = release.deployments[0].id.clone();
    release = store.record_release_smoke(
        "sample",
        &release.id,
        release.revision,
        &deployment_id,
        "passed",
        "Launch smoke checks passed",
        "owner",
        "smoke",
    )?;
    let record = serde_json::json!({"failed_required_check_blocked_approval":failed_approval.unwrap_err().to_string(),
        "release":release});
    let path = root.join("release-record.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&record)?)?;
    println!("{}", path.display());
    Ok(())
}
