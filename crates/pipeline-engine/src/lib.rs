//! Application services for local projects. UI and agents use this boundary.

use pipeline_domain::{ProjectHealth, ProjectStage};
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
                let next_owner_action = if metrics.blocked_count > 0 {
                    format!("Resolve {} blocked task(s)", metrics.blocked_count)
                } else if record.active_scope_revision == 0 {
                    "Write and approve a project brief".to_owned()
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
}
