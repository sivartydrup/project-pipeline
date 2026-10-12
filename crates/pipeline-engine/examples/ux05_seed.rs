//! Isolated native screenshot fixture; writes only through ProjectEngine services.
use pipeline_engine::{BriefContent, ProjectEngine};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::path::PathBuf::from("target/ux05-native");
    std::fs::create_dir_all(&data)?;
    let project_path = data.join("sample-project");
    std::fs::create_dir_all(&project_path)?;
    let mut engine = ProjectEngine::open(data.join("portfolio.sqlite"))?;
    if engine.list_overviews()?.is_empty() {
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
    Ok(())
}
