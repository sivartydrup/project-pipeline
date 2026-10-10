//! Prepare and cancel a disposable task without sending a model prompt.
use pipeline_engine::{ProjectEngine, RunLimits};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database = std::env::args()
        .nth(1)
        .ok_or("pass fixture database path")?;
    let mut engine = ProjectEngine::open(&database)?;
    let start = engine.prepare_agent_run(
        "p14-sample",
        "fix-add",
        true,
        &[
            "project.get".into(),
            "task.get".into(),
            "task.submit".into(),
            "artifact.attach".into(),
            "test.record".into(),
        ],
        RunLimits {
            wall_seconds: 900,
            token_budget: 20_000,
            token_ttl_seconds: 900,
        },
    )?;
    let packet = serde_json::to_string_pretty(&start.packet)?;
    println!("{packet}");
    engine.transition_agent_run(&start.run.id, start.run.revision, "cancelled")?;
    Ok(())
}
