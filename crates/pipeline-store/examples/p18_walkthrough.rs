use pipeline_domain::{BriefContent, CriterionSpec, EpicSpec, PlanContent, TaskSpec};
use pipeline_store::{LifecycleInput, Store};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn entry(kind: &str, title: &str, body: &str) -> LifecycleInput {
    LifecycleInput {
        kind: kind.into(),
        title: title.into(),
        body: body.into(),
        status: "open".into(),
        severity: "low".into(),
        ..Default::default()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from("target/p18-sample");
    std::fs::create_dir_all(&root)?;
    let root = dunce::canonicalize(root)?;
    let build = root.join("app.exe");
    let check = root.join("checks.log");
    let asset = root.join("launch.png");
    std::fs::write(&build, b"sample build")?;
    std::fs::write(&check, b"quality and security checks passed")?;
    std::fs::write(&asset, b"sample launch asset")?;
    let mut s = Store::open_in_memory()?;
    s.create_project(
        "sample",
        "Sample idea to operation",
        root.to_str().unwrap(),
        "owner",
        "project",
    )?;
    let brief = BriefContent {
        idea: "Desktop app for solo builders".into(),
        audience: "Solo builders".into(),
        problem: "Scattered product work".into(),
        desired_outcome: "One reviewable workflow".into(),
        ..Default::default()
    };
    s.save_brief_revision("brief", "sample", 0, &brief, "owner", "brief")?;
    s.approve_brief_revision("sample", 1, "brief-approval", "owner", "brief-approve")?;
    let plan = PlanContent {
        milestones: vec![],
        epics: vec![EpicSpec {
            id: "app".into(),
            title: "Build app".into(),
            outcome: "Usable sample".into(),
        }],
        tasks: vec![TaskSpec {
            id: "build".into(),
            epic_id: Some("app".into()),
            title: "Build sample".into(),
            outcome: "Sample app works".into(),
            weight: 1,
            risk: "low".into(),
            criteria: vec![CriterionSpec {
                id: "check".into(),
                assertion: "Smoke test passes".into(),
                verifier: "owner".into(),
                required_evidence: vec!["test-log".into()],
            }],
            verification_commands: vec!["manual smoke".into()],
            deliverables: vec!["app.exe".into()],
            context_links: vec![],
            estimate_band: "small".into(),
            owner_decision_triggers: vec![],
        }],
        dependencies: vec![],
    };
    s.save_plan_revision("plan", "sample", 0, &plan, "owner", "plan")?;
    s.approve_plan_revision("sample", 1, "plan-approval", "owner", "plan-approve")?;
    s.submit_task_for_review("sample", "build", 1, "owner", "review")?;
    s.verify_criterion(
        "sample",
        "build",
        "check",
        2,
        &BTreeMap::from([("test-log".into(), check.to_string_lossy().to_string())]),
        "owner",
        "verify",
    )?;
    s.accept_task("sample", "build", 3, "owner", "accept")?;
    let commit = "a".repeat(40);
    let mut release = s.create_release(
        "sample",
        "1.0.0",
        &commit,
        "sample production",
        "Restore prior build",
        "owner",
        "release",
    )?;
    let build_id = s.attach_release_artifact(
        "sample",
        &build,
        "build",
        Some(&commit),
        "owner",
        "build-artifact",
    )?;
    release = s.add_release_build(
        "sample",
        &release.id,
        release.revision,
        &build_id,
        "windows",
        "owner",
        "build-link",
    )?;
    let check_id = s.attach_release_artifact(
        "sample",
        &check,
        "check-log",
        Some(&commit),
        "owner",
        "check-artifact",
    )?;
    release = s.set_release_check(
        "sample",
        &release.id,
        release.revision,
        "quality and security",
        true,
        "passed",
        Some(&check_id),
        "owner",
        "check",
    )?;
    release = s.approve_release("sample", &release.id, release.revision, "owner", "ready")?;
    let execution =
        s.approve_release_execution("sample", &release.id, release.revision, "owner", "execute")?;
    release = s.record_manual_deployment(
        "sample",
        &release.id,
        release.revision,
        &execution,
        "succeeded",
        "passed",
        "Sample manual deployment record",
        "owner",
        "deployment",
    )?;
    let mut positioning = entry(
        "positioning",
        "Simple product control",
        "One place for reviewed work",
    );
    positioning.publication_target = "sample landing page".into();
    let mut positioning = s.create_lifecycle("sample", &positioning, "owner", "positioning")?;
    let approval = s.approve_publication(
        "sample",
        &positioning.id,
        positioning.revision,
        "owner",
        "publish-approval",
    )?;
    positioning = s.record_manual_publication(
        "sample",
        &positioning.id,
        positioning.revision,
        &approval,
        "owner",
        "manual-publication",
    )?;
    let mut copy = entry("launch_copy", "Launch copy", "The sample app is available");
    copy.publication_target = "sample announcement".into();
    s.create_lifecycle("sample", &copy, "owner", "copy")?;
    let asset_id = s.attach_release_artifact(
        "sample",
        &asset,
        "launch-asset",
        None,
        "owner",
        "launch-asset",
    )?;
    let mut launch_asset = entry("launch_asset", "Launch image", "Sample illustration");
    launch_asset.artifact_id = Some(asset_id);
    s.create_lifecycle("sample", &launch_asset, "owner", "asset")?;
    let mut channel = entry("channel_plan", "Community launch", "Post after review");
    channel.channel = "community".into();
    s.create_lifecycle("sample", &channel, "owner", "channel")?;
    let mut metric = entry("metric", "First week signups", "Manual count");
    metric.metric_value = "12".into();
    metric.metric_unit = "signups".into();
    s.create_lifecycle("sample", &metric, "owner", "metric")?;
    for (kind, title) in [
        ("feedback", "Requested keyboard shortcut"),
        ("bug", "Minor layout issue"),
        ("incident", "Sample outage resolved"),
        ("maintenance", "Monthly dependency review"),
    ] {
        let mut item = entry(kind, title, "Follow up in the same project");
        if kind == "incident" {
            item.status = "resolved".into();
            item.severity = "high".into();
        }
        s.create_lifecycle("sample", &item, "owner", kind)?;
    }
    let export = serde_json::json!({"walkthrough":{
        "brief_approved":s.brief_status("sample")?.approved_revision,
        "accepted_task":"build","release_status":release.status,
        "published_positioning":positioning.input.status,
        "lifecycle_kinds":s.list_lifecycle("sample")?.iter().map(|r|r.input.kind.clone()).collect::<Vec<_>>(),
        "blockers":s.lifecycle_blockers("sample")?.operations,
        "activity_count":s.activity_count("sample")?},
        "export":serde_json::from_str::<serde_json::Value>(&s.export_json()?)?});
    let path = root.join("walkthrough-export.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&export)?)?;
    println!("{}", path.display());
    Ok(())
}
