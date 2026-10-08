//! Application services for local projects. UI and agents use this boundary.

pub use pipeline_domain::{BriefChange, BriefContent};
use pipeline_domain::{ProjectHealth, ProjectStage};
pub use pipeline_store::{BriefRecord, ResearchInput, ResearchRecord};
use pipeline_store::{Store, StoreError};
use std::path::Path;
use std::process::Command;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("folder does not exist: {0}")]
    MissingFolder(String),
    #[error("path must point to a folder below a filesystem root")]
    InvalidFolder,
    #[error("this folder is already in the portfolio")]
    AlreadyImported,
    #[error("project name must not be empty")]
    EmptyName,
}

pub type Result<T> = std::result::Result<T, EngineError>;

#[derive(Debug, Clone)]
pub struct ProjectOverview {
    pub id: String,
    pub name: String,
    pub path: String,
    pub git_root: Option<String>,
    pub stage: ProjectStage,
    pub health: ProjectHealth,
    pub verified_completion_basis_points: u16,
    pub blocked_count: u32,
    pub next_owner_action: String,
}

#[derive(Debug, Clone)]
pub struct DiscoveryView {
    pub briefs: Vec<BriefRecord>,
    pub research: Vec<ResearchRecord>,
    pub approved_revision: i64,
    pub changes: Vec<BriefChange>,
}

impl DiscoveryView {
    pub fn latest_revision(&self) -> i64 {
        self.briefs.last().map_or(0, |brief| brief.revision)
    }

    pub fn latest_content(&self) -> BriefContent {
        self.briefs
            .last()
            .map_or_else(BriefContent::default, |brief| brief.content.clone())
    }
}

pub struct ProjectEngine {
    store: Store,
}

impl ProjectEngine {
    pub fn open(database: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            store: Store::open(database)?,
        })
    }

    pub fn import_existing(
        &mut self,
        path: impl AsRef<Path>,
        name: &str,
    ) -> Result<ProjectOverview> {
        let path = path.as_ref();
        if !path.is_dir() {
            return Err(EngineError::MissingFolder(path.display().to_string()));
        }
        let canonical = dunce::canonicalize(path)?;
        if canonical.parent().is_none() {
            return Err(EngineError::InvalidFolder);
        }
        let canonical = canonical.to_string_lossy().into_owned();
        let legacy = std::fs::canonicalize(path)?.to_string_lossy().into_owned();
        if self.store.get_project_by_path(&canonical)?.is_some()
            || self.store.get_project_by_path(&legacy)?.is_some()
        {
            return Err(EngineError::AlreadyImported);
        }
        let name = if name.trim().is_empty() {
            path.file_name()
                .and_then(|segment| segment.to_str())
                .filter(|segment| !segment.trim().is_empty())
                .ok_or(EngineError::EmptyName)?
        } else {
            name.trim()
        };
        let git_root = git_root(Path::new(&canonical));
        let id = Uuid::new_v4().to_string();
        self.store.create_project_with_git(
            &id,
            name,
            &canonical,
            git_root.as_deref(),
            "owner",
            &Uuid::new_v4().to_string(),
        )?;
        self.list_overviews()?
            .into_iter()
            .find(|item| item.id == id)
            .ok_or_else(|| StoreError::NotFound(id).into())
    }

    pub fn create_new(&mut self, path: impl AsRef<Path>, name: &str) -> Result<ProjectOverview> {
        let path = path.as_ref();
        if path.exists() {
            return Err(EngineError::AlreadyImported);
        }
        if name.trim().is_empty() {
            return Err(EngineError::EmptyName);
        }
        std::fs::create_dir_all(path)?;
        self.import_existing(path, name)
    }

    pub fn list_overviews(&self) -> Result<Vec<ProjectOverview>> {
        self.store
            .list_projects()?
            .into_iter()
            .map(|record| {
                let metrics = self.store.project_metrics(&record.id)?;
                let brief = self.store.brief_status(&record.id)?;
                let next_owner_action = if metrics.blocked_count > 0 {
                    format!("Resolve {} blocked task(s)", metrics.blocked_count)
                } else if brief.latest_revision > brief.approved_revision {
                    format!("Review brief revision {}", brief.latest_revision)
                } else if brief.approved_revision == 0 {
                    "Write and approve a project brief".to_owned()
                } else if record.active_scope_revision == 0 {
                    "Review and approve the task plan".to_owned()
                } else {
                    "Review the project plan and next ready task".to_owned()
                };
                Ok(ProjectOverview {
                    id: record.id,
                    name: record.name,
                    path: dunce::simplified(Path::new(&record.path))
                        .to_string_lossy()
                        .into_owned(),
                    git_root: record.git_root.map(|root| {
                        dunce::simplified(Path::new(&root))
                            .to_string_lossy()
                            .into_owned()
                    }),
                    stage: record.stage,
                    health: record.health,
                    verified_completion_basis_points: metrics.verified_completion_basis_points,
                    blocked_count: metrics.blocked_count,
                    next_owner_action,
                })
            })
            .collect()
    }

    pub fn load_discovery(&self, project_id: &str) -> Result<DiscoveryView> {
        let status = self.store.brief_status(project_id)?;
        let briefs = self.store.list_brief_revisions(project_id)?;
        let changes = if briefs.len() >= 2 {
            briefs[briefs.len() - 1]
                .content
                .changes_from(&briefs[briefs.len() - 2].content)
        } else {
            Vec::new()
        };
        Ok(DiscoveryView {
            briefs,
            research: self.store.list_research_findings(project_id)?,
            approved_revision: status.approved_revision,
            changes,
        })
    }

    pub fn save_brief(
        &mut self,
        project_id: &str,
        expected_latest_revision: i64,
        content: &BriefContent,
    ) -> Result<DiscoveryView> {
        self.store.save_brief_revision(
            &Uuid::new_v4().to_string(),
            project_id,
            expected_latest_revision,
            content,
            "owner",
            &Uuid::new_v4().to_string(),
        )?;
        self.load_discovery(project_id)
    }

    pub fn approve_latest_brief(
        &mut self,
        project_id: &str,
        expected_latest_revision: i64,
    ) -> Result<DiscoveryView> {
        self.store.approve_brief_revision(
            project_id,
            expected_latest_revision,
            &Uuid::new_v4().to_string(),
            "owner",
            &Uuid::new_v4().to_string(),
        )?;
        self.load_discovery(project_id)
    }

    pub fn record_research(
        &mut self,
        project_id: &str,
        input: &ResearchInput,
    ) -> Result<DiscoveryView> {
        let latest = self.store.list_brief_revisions(project_id)?.pop();
        self.store.add_research_finding(
            &Uuid::new_v4().to_string(),
            project_id,
            latest.as_ref().map(|brief| brief.id.as_str()),
            input,
            "owner",
            &Uuid::new_v4().to_string(),
        )?;
        self.load_discovery(project_id)
    }
}

fn git_root(path: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let reported = String::from_utf8(output.stdout).ok()?;
    dunce::canonicalize(reported.trim())
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_two_repositories_without_touching_source_and_persists() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("portfolio.sqlite");
        let mut engine = ProjectEngine::open(&database).unwrap();
        for index in 0..2 {
            let folder = temp.path().join(format!("repo-{index}"));
            std::fs::create_dir(&folder).unwrap();
            let source = folder.join("README.md");
            std::fs::write(&source, format!("content {index}")).unwrap();
            assert!(
                Command::new("git")
                    .arg("init")
                    .arg(&folder)
                    .output()
                    .unwrap()
                    .status
                    .success()
            );
            let imported = engine.import_existing(&folder, "").unwrap();
            assert_eq!(imported.stage, ProjectStage::Idea);
            assert_eq!(imported.verified_completion_basis_points, 0);
            assert!(imported.git_root.is_some());
            assert_eq!(
                std::fs::read_to_string(source).unwrap(),
                format!("content {index}")
            );
            assert!(matches!(
                engine.import_existing(&folder, ""),
                Err(EngineError::AlreadyImported)
            ));
        }
        drop(engine);
        assert_eq!(
            ProjectEngine::open(database)
                .unwrap()
                .list_overviews()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn idea_to_approved_brief_journey_survives_restart_and_revisions() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("idea-project");
        std::fs::create_dir(&folder).unwrap();
        let database = temp.path().join("portfolio.sqlite");
        let mut engine = ProjectEngine::open(&database).unwrap();
        let project = engine.import_existing(&folder, "Idea Project").unwrap();
        assert_eq!(
            engine
                .load_discovery(&project.id)
                .unwrap()
                .latest_revision(),
            0
        );
        let mut brief = BriefContent {
            idea: "A calm project dashboard".to_owned(),
            audience: "Solo developers".to_owned(),
            problem: "Work gets lost between tools".to_owned(),
            desired_outcome: "A reviewable path to release".to_owned(),
            ..Default::default()
        };
        let view = engine.save_brief(&project.id, 0, &brief).unwrap();
        assert_eq!(view.latest_revision(), 1);
        assert_eq!(view.approved_revision, 0);
        let view = engine.approve_latest_brief(&project.id, 1).unwrap();
        assert_eq!(view.approved_revision, 1);
        assert_eq!(
            engine.list_overviews().unwrap()[0].stage,
            ProjectStage::Planning
        );
        let incomplete_fact = ResearchInput {
            claim: "Competitors have dashboards".to_owned(),
            summary: "Observed competitor feature".to_owned(),
            relevance: "Potential baseline".to_owned(),
            confidence: "medium".to_owned(),
            ..Default::default()
        };
        assert!(matches!(
            engine.record_research(&project.id, &incomplete_fact),
            Err(EngineError::Store(StoreError::MissingResearchSource))
        ));
        let hypothesis = ResearchInput {
            is_hypothesis: true,
            ..incomplete_fact.clone()
        };
        engine.record_research(&project.id, &hypothesis).unwrap();
        let sourced = ResearchInput {
            source_uri: "https://example.com/feature".to_owned(),
            accessed_at: "2026-10-08".to_owned(),
            ..incomplete_fact
        };
        engine.record_research(&project.id, &sourced).unwrap();
        brief.scope = "Local-first desktop only".to_owned();
        let view = engine.save_brief(&project.id, 1, &brief).unwrap();
        assert_eq!(view.latest_revision(), 2);
        assert_eq!(view.approved_revision, 1);
        assert_eq!(
            view.changes
                .iter()
                .map(|change| change.field)
                .collect::<Vec<_>>(),
            vec!["Scope"]
        );
        assert!(matches!(
            engine.save_brief(&project.id, 1, &brief),
            Err(EngineError::Store(StoreError::BriefRevisionConflict { .. }))
        ));
        assert!(matches!(
            engine.approve_latest_brief(&project.id, 1),
            Err(EngineError::Store(StoreError::BriefRevisionConflict { .. }))
        ));
        engine.approve_latest_brief(&project.id, 2).unwrap();
        drop(engine);
        let reopened = ProjectEngine::open(database).unwrap();
        let view = reopened.load_discovery(&project.id).unwrap();
        assert_eq!(view.approved_revision, 2);
        assert_eq!(view.research.len(), 2);
        assert_eq!(view.briefs[0].status, "approved");
        assert_eq!(view.briefs[1].status, "approved");
    }
}
