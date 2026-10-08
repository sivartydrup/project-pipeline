//! SQLite persistence and transactional audit for Project Pipeline.

use pipeline_domain::{ProjectHealth, ProjectStage};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, MAIN_DB, OptionalExtension, params};
use serde_json::{Map, Value, json};
use std::path::Path;
use std::time::Duration;

mod discovery;
pub use discovery::{BriefRecord, BriefStatus, ResearchInput, ResearchRecord};
mod plan;
pub use plan::{CriterionRecord, PlanRecord, PlanStatus, TaskRecord};

const CURRENT_SCHEMA_VERSION: i64 = 4;
const INITIAL_SCHEMA: &str = include_str!("../migrations/001_initial.sql");
const PROJECT_GIT_SCHEMA: &str = include_str!("../migrations/002_project_git.sql");
const DISCOVERY_SCHEMA: &str = include_str!("../migrations/003_discovery.sql");
const PLAN_SCHEMA: &str = include_str!("../migrations/004_plan.sql");
const EXPORT_TABLES: &[&str] = &[
    "projects",
    "brief_revisions",
    "plan_revisions",
    "research_findings",
    "milestones",
    "epics",
    "tasks",
    "dependencies",
    "criteria",
    "decisions",
    "approvals",
    "agent_runs",
    "artifacts",
    "run_events",
    "test_results",
    "release_candidates",
    "activity_events",
];

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("project not found: {0}")]
    NotFound(String),
    #[error("revision conflict for project {id}: expected {expected}, actual {actual}")]
    RevisionConflict {
        id: String,
        expected: i64,
        actual: i64,
    },
    #[error("{0} must not be empty")]
    EmptyField(&'static str),
    #[error("brief revision conflict for project {id}: expected {expected}, actual {actual}")]
    BriefRevisionConflict {
        id: String,
        expected: i64,
        actual: i64,
    },
    #[error("only the owner may approve a brief")]
    ApprovalRequiresOwner,
    #[error("brief requires an idea, audience, problem, and desired outcome before approval")]
    IncompleteBrief,
    #[error("brief revision is already approved")]
    BriefAlreadyApproved,
    #[error("sourced research requires a source URI and access date")]
    MissingResearchSource,
    #[error(transparent)]
    InvalidPlan(#[from] pipeline_domain::PlanError),
    #[error("plan revision conflict for project {id}: expected {expected}, actual {actual}")]
    PlanRevisionConflict {
        id: String,
        expected: i64,
        actual: i64,
    },
    #[error("an approved brief is required before plan approval")]
    PlanRequiresBrief,
    #[error("approve the latest brief revision before approving the task plan")]
    PlanRequiresCurrentBrief,
    #[error("only the owner may approve a plan or accept a task")]
    PlanRequiresOwner,
    #[error("plan revision is already approved")]
    PlanAlreadyApproved,
    #[error("task is not in the active approved scope: {0}")]
    TaskNotInActiveScope(String),
    #[error("task has unresolved blocking dependencies: {0}")]
    TaskDependencyBlocked(String),
    #[error("task cannot transition from {from} to {to}")]
    InvalidTaskTransition { from: String, to: &'static str },
    #[error("task acceptance criteria are not verified with required evidence: {0}")]
    CriteriaNotVerified(String),
    #[error("missing evidence reference for required kind: {0}")]
    MissingEvidence(String),
    #[error("database schema {found} is newer than supported schema {supported}")]
    NewerSchema { found: i64, supported: i64 },
    #[error("backup destination already exists")]
    BackupExists,
    #[error("backup failed integrity check: {0}")]
    BackupInvalid(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub path: String,
    pub git_root: Option<String>,
    pub stage: ProjectStage,
    pub health: ProjectHealth,
    pub active_scope_revision: i64,
    pub revision: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectMetrics {
    pub verified_completion_basis_points: u16,
    pub blocked_count: u32,
}

pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let connection = Connection::open(path.as_ref())?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        backup_before_upgrade(&connection, path.as_ref(), version, CURRENT_SCHEMA_VERSION)?;
        Self::from_connection(connection)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut connection: Connection) -> Result<Self> {
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.busy_timeout(Duration::from_secs(5))?;
        migrate(&mut connection)?;
        Ok(Self { connection })
    }

    pub fn schema_version(&self) -> Result<i64> {
        Ok(self
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    pub fn create_project(
        &mut self,
        id: &str,
        name: &str,
        path: &str,
        actor: &str,
        correlation_id: &str,
    ) -> Result<ProjectRecord> {
        self.create_project_with_git(id, name, path, None, actor, correlation_id)
    }

    pub fn create_project_with_git(
        &mut self,
        id: &str,
        name: &str,
        path: &str,
        git_root: Option<&str>,
        actor: &str,
        correlation_id: &str,
    ) -> Result<ProjectRecord> {
        validate_nonempty("id", id)?;
        validate_nonempty("name", name)?;
        validate_nonempty("path", path)?;
        validate_nonempty("actor", actor)?;
        validate_nonempty("correlation_id", correlation_id)?;

        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO projects(id, name, path, git_root, health) VALUES (?1, ?2, ?3, ?4, 'needs_input')",
            params![id, name.trim(), path, git_root],
        )?;
        let project = get_project_from(&transaction, id)?
            .expect("project was inserted in the current transaction");
        insert_activity(
            &transaction,
            id,
            actor,
            "project.create",
            1,
            None,
            Some(json!({"name": project.name, "path": project.path, "git_root": project.git_root})),
            correlation_id,
        )?;
        transaction.commit()?;
        Ok(project)
    }

    pub fn get_project(&self, id: &str) -> Result<Option<ProjectRecord>> {
        Ok(get_project_from(&self.connection, id)?)
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, path, stage, health, active_scope_revision, revision, git_root \
             FROM projects ORDER BY name COLLATE NOCASE, id",
        )?;
        let projects = statement.query_map([], project_from_row)?;
        projects
            .map(|project| project.map_err(StoreError::from))
            .collect()
    }

    pub fn get_project_by_path(&self, path: &str) -> Result<Option<ProjectRecord>> {
        let id: Option<String> = self
            .connection
            .query_row("SELECT id FROM projects WHERE path = ?1", [path], |row| {
                row.get(0)
            })
            .optional()?;
        match id {
            Some(id) => self.get_project(&id),
            None => Ok(None),
        }
    }

    pub fn project_metrics(&self, project_id: &str) -> Result<ProjectMetrics> {
        let (accepted_weight, total_weight, blocked_count): (i64, i64, i64) = self.connection.query_row(
            "SELECT COALESCE(SUM(CASE WHEN status='accepted' AND EXISTS
                    (SELECT 1 FROM criteria c WHERE c.task_id=tasks.id)
                    AND NOT EXISTS (SELECT 1 FROM criteria c WHERE c.task_id=tasks.id
                    AND (c.accepted_at IS NULL OR c.verified_by IS NULL))
                    THEN weight ELSE 0 END), 0), \
                    COALESCE(SUM(weight), 0), \
                    COALESCE(SUM(CASE WHEN status='blocked' THEN 1 ELSE 0 END), 0) \
             FROM tasks WHERE project_id=?1 AND scope_revision=(SELECT active_scope_revision FROM projects WHERE id=?1)",
            [project_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let completion = if total_weight == 0 {
            0
        } else {
            (accepted_weight * 10_000 / total_weight) as u16
        };
        Ok(ProjectMetrics {
            verified_completion_basis_points: completion,
            blocked_count: blocked_count as u32,
        })
    }

    pub fn rename_project(
        &mut self,
        id: &str,
        expected_revision: i64,
        new_name: &str,
        actor: &str,
        correlation_id: &str,
    ) -> Result<ProjectRecord> {
        validate_nonempty("name", new_name)?;
        validate_nonempty("actor", actor)?;
        validate_nonempty("correlation_id", correlation_id)?;

        let transaction = self.connection.transaction()?;
        let before = get_project_from(&transaction, id)?
            .ok_or_else(|| StoreError::NotFound(id.to_owned()))?;
        if before.revision != expected_revision {
            return Err(StoreError::RevisionConflict {
                id: id.to_owned(),
                expected: expected_revision,
                actual: before.revision,
            });
        }
        transaction.execute(
            "UPDATE projects SET name = ?1, revision = revision + 1, \
             updated_at = CURRENT_TIMESTAMP WHERE id = ?2 AND revision = ?3",
            params![new_name.trim(), id, expected_revision],
        )?;
        let after = get_project_from(&transaction, id)?
            .expect("project was updated in the current transaction");
        insert_activity(
            &transaction,
            id,
            actor,
            "project.rename",
            after.revision,
            Some(json!({"name": before.name})),
            Some(json!({"name": after.name})),
            correlation_id,
        )?;
        transaction.commit()?;
        Ok(after)
    }

    pub fn activity_count(&self, project_id: &str) -> Result<i64> {
        Ok(self.connection.query_row(
            "SELECT count(*) FROM activity_events WHERE project_id = ?1",
            [project_id],
            |row| row.get(0),
        )?)
    }

    /// Creates a consistent SQLite backup without overwriting an earlier copy.
    pub fn backup_to(&self, destination: impl AsRef<Path>) -> Result<()> {
        if destination.as_ref().exists() {
            return Err(StoreError::BackupExists);
        }
        self.connection.backup(MAIN_DB, destination, None)?;
        Ok(())
    }

    /// Call only from a repair workflow with exclusive ownership of the database.
    pub fn restore_from(&mut self, source: impl AsRef<Path>) -> Result<()> {
        let source_connection = Connection::open_with_flags(
            source.as_ref(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let integrity: String =
            source_connection.pragma_query_value(None, "integrity_check", |row| row.get(0))?;
        if integrity != "ok" {
            return Err(StoreError::BackupInvalid(integrity));
        }
        let version: i64 =
            source_connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > CURRENT_SCHEMA_VERSION {
            return Err(StoreError::NewerSchema {
                found: version,
                supported: CURRENT_SCHEMA_VERSION,
            });
        }
        drop(source_connection);
        self.connection.restore(MAIN_DB, source, None::<fn(_)>)?;
        self.connection.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&mut self.connection)?;
        Ok(())
    }

    /// Exports every workflow table. This schema has no credential fields;
    /// redaction of accidental secrets in free text is a later API gate.
    pub fn export_json(&self) -> Result<String> {
        let mut tables = Map::new();
        for table in EXPORT_TABLES {
            let mut statement = self.connection.prepare(&format!("SELECT * FROM {table}"))?;
            let columns = statement
                .column_names()
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>();
            let rows = statement.query_map([], |row| {
                let mut object = Map::new();
                for (index, name) in columns.iter().enumerate() {
                    let value = match row.get_ref(index)? {
                        ValueRef::Null => Value::Null,
                        ValueRef::Integer(value) => json!(value),
                        ValueRef::Real(value) => json!(value),
                        ValueRef::Text(value) => {
                            Value::String(String::from_utf8_lossy(value).into_owned())
                        }
                        ValueRef::Blob(value) => json!(value),
                    };
                    object.insert(name.clone(), value);
                }
                Ok(Value::Object(object))
            })?;
            tables.insert(
                (*table).to_owned(),
                Value::Array(rows.collect::<rusqlite::Result<Vec<_>>>()?),
            );
        }
        Ok(serde_json::to_string_pretty(&json!({
            "schema_version": self.schema_version()?, "tables": tables
        }))?)
    }
}

fn validate_nonempty(field: &'static str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(StoreError::EmptyField(field))
    } else {
        Ok(())
    }
}

fn project_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectRecord> {
    let stage: String = row.get(3)?;
    let health: String = row.get(4)?;
    Ok(ProjectRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        path: row.get(2)?,
        stage: ProjectStage::parse(&stage).ok_or(rusqlite::Error::InvalidQuery)?,
        health: ProjectHealth::parse(&health).ok_or(rusqlite::Error::InvalidQuery)?,
        active_scope_revision: row.get(5)?,
        revision: row.get(6)?,
        git_root: row.get(7)?,
    })
}

fn get_project_from(connection: &Connection, id: &str) -> rusqlite::Result<Option<ProjectRecord>> {
    connection.query_row(
        "SELECT id, name, path, stage, health, active_scope_revision, revision, git_root FROM projects WHERE id = ?1",
        [id], project_from_row,
    ).optional()
}

#[allow(clippy::too_many_arguments)]
fn insert_activity(
    connection: &Connection,
    project_id: &str,
    actor: &str,
    operation: &str,
    revision: i64,
    before: Option<Value>,
    after: Option<Value>,
    correlation_id: &str,
) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO activity_events(project_id, actor, operation, subject_type, subject_id, \
         subject_revision, before_json, after_json, correlation_id) \
         VALUES (?1, ?2, ?3, 'project', ?1, ?4, ?5, ?6, ?7)",
        params![
            project_id,
            actor,
            operation,
            revision,
            before.map(|value| value.to_string()),
            after.map(|value| value.to_string()),
            correlation_id
        ],
    )?;
    Ok(())
}

fn migrate(connection: &mut Connection) -> Result<()> {
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > CURRENT_SCHEMA_VERSION {
        return Err(StoreError::NewerSchema {
            found: version,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }
    if version == 0 {
        apply_migration(connection, 1, INITIAL_SCHEMA)?;
    }
    if version < 2 {
        apply_migration(connection, 2, PROJECT_GIT_SCHEMA)?;
    }
    if version < 3 {
        apply_migration(connection, 3, DISCOVERY_SCHEMA)?;
    }
    if version < 4 {
        apply_migration(connection, 4, PLAN_SCHEMA)?;
    }
    Ok(())
}

fn backup_before_upgrade(connection: &Connection, path: &Path, from: i64, to: i64) -> Result<()> {
    if from == 0 || from >= to {
        return Ok(());
    }
    let destination = path.with_extension(format!("pre-v{from}.sqlite"));
    if destination.exists() {
        return Err(StoreError::BackupExists);
    }
    connection.backup(MAIN_DB, destination, None)?;
    Ok(())
}

fn apply_migration(connection: &mut Connection, version: i64, sql: &str) -> Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(sql)?;
    transaction.pragma_update(None, "user_version", version)?;
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_changes_are_atomic_and_revision_checked() {
        let mut store = Store::open_in_memory().unwrap();
        let project = store
            .create_project("p1", "First", "C:/first", "owner", "create-1")
            .unwrap();
        assert_eq!(project.revision, 1);
        assert_eq!(store.activity_count("p1").unwrap(), 1);
        let project = store
            .rename_project("p1", 1, "Renamed", "owner", "rename-1")
            .unwrap();
        assert_eq!(project.revision, 2);
        assert_eq!(store.activity_count("p1").unwrap(), 2);
        let error = store
            .rename_project("p1", 1, "Stale", "owner", "rename-2")
            .unwrap_err();
        assert!(matches!(error, StoreError::RevisionConflict { .. }));
        assert_eq!(store.get_project("p1").unwrap().unwrap().name, "Renamed");
        assert_eq!(store.activity_count("p1").unwrap(), 2);
    }

    #[test]
    fn failed_migration_rolls_back_all_schema_changes() {
        let mut connection = Connection::open_in_memory().unwrap();
        assert!(
            apply_migration(
                &mut connection,
                1,
                "CREATE TABLE temporary_test(id INTEGER); INVALID SQL;"
            )
            .is_err()
        );
        let exists: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='temporary_test'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 0);
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 0);
    }

    #[test]
    fn backup_restore_and_export_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let backup = temp.path().join("backup.sqlite");
        let mut store = Store::open_in_memory().unwrap();
        store
            .create_project("p1", "First", "C:/first", "owner", "create-1")
            .unwrap();
        store.backup_to(&backup).unwrap();
        let mut restored = Store::open_in_memory().unwrap();
        restored.restore_from(&backup).unwrap();
        assert_eq!(restored.get_project("p1").unwrap().unwrap().name, "First");
        assert_eq!(restored.activity_count("p1").unwrap(), 1);
        let export: Value = serde_json::from_str(&restored.export_json().unwrap()).unwrap();
        assert_eq!(export["schema_version"], 4);
        assert_eq!(export["tables"]["projects"][0]["id"], "p1");
        assert_eq!(
            export["tables"]["activity_events"][0]["operation"],
            "project.create"
        );
    }

    #[test]
    fn file_store_survives_reopen_and_audit_cannot_be_rewritten() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pipeline.sqlite");
        {
            let mut store = Store::open(&path).unwrap();
            store
                .create_project("p1", "First", "C:/first", "owner", "create-1")
                .unwrap();
            assert!(
                store
                    .connection
                    .execute("DELETE FROM activity_events WHERE project_id = 'p1'", [])
                    .is_err()
            );
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(store.get_project("p1").unwrap().unwrap().name, "First");
        assert_eq!(store.activity_count("p1").unwrap(), 1);
    }

    #[test]
    fn activity_failure_rolls_back_project_creation() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_activity BEFORE INSERT ON activity_events \
             BEGIN SELECT RAISE(ABORT, 'test failure'); END;",
            )
            .unwrap();
        assert!(
            store
                .create_project("p1", "First", "C:/first", "owner", "create-1")
                .is_err()
        );
        assert!(store.get_project("p1").unwrap().is_none());
    }

    #[test]
    fn upgrade_backup_preserves_previous_database() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pipeline.sqlite");
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch("CREATE TABLE old_data(value TEXT); INSERT INTO old_data VALUES ('keep'); PRAGMA user_version = 1;").unwrap();
        backup_before_upgrade(&connection, &path, 1, 2).unwrap();
        let backup = Connection::open(path.with_extension("pre-v1.sqlite")).unwrap();
        let value: String = backup
            .query_row("SELECT value FROM old_data", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, "keep");
        assert!(matches!(
            backup_before_upgrade(&connection, &path, 1, 2),
            Err(StoreError::BackupExists)
        ));
    }

    #[test]
    fn upgrades_v1_and_creates_preupgrade_backup() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pipeline.sqlite");
        let mut connection = Connection::open(&path).unwrap();
        apply_migration(&mut connection, 1, INITIAL_SCHEMA).unwrap();
        connection
            .execute(
                "INSERT INTO projects(id,name,path) VALUES ('p1','First','C:/first')",
                [],
            )
            .unwrap();
        drop(connection);
        let store = Store::open(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(), 4);
        assert_eq!(store.get_project("p1").unwrap().unwrap().git_root, None);
        let backup = Connection::open(path.with_extension("pre-v1.sqlite")).unwrap();
        let version: i64 = backup
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn upgrades_v3_task_rows_and_keeps_preupgrade_backup() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pipeline.sqlite");
        let mut connection = Connection::open(&path).unwrap();
        apply_migration(&mut connection, 1, INITIAL_SCHEMA).unwrap();
        apply_migration(&mut connection, 2, PROJECT_GIT_SCHEMA).unwrap();
        apply_migration(&mut connection, 3, DISCOVERY_SCHEMA).unwrap();
        connection.execute("INSERT INTO projects(id,name,path,active_scope_revision) VALUES ('p1','First','C:/first',1)",[]).unwrap();
        connection
            .execute(
                "INSERT INTO tasks(id,project_id,title,outcome,status,weight,scope_revision)
            VALUES ('old-task','p1','Old','done','ready',2,1)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO criteria(id,task_id,assertion,verifier)
            VALUES ('old-criterion','old-task','works','owner')",
                [],
            )
            .unwrap();
        drop(connection);
        let store = Store::open(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(), 4);
        let tasks = store.list_active_tasks("p1").unwrap();
        assert_eq!(tasks[0].logical_id, "old-task");
        assert_eq!(tasks[0].criteria[0].logical_id, "old-criterion");
        let backup = Connection::open(path.with_extension("pre-v3.sqlite")).unwrap();
        assert_eq!(
            backup
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            3
        );
    }

    #[test]
    fn portfolio_metrics_use_only_active_scope_and_accepted_weight() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .create_project("p1", "First", "C:/first", "owner", "create-1")
            .unwrap();
        store
            .connection
            .execute(
                "UPDATE projects SET active_scope_revision=2 WHERE id='p1'",
                [],
            )
            .unwrap();
        for (id, status, weight, scope) in [
            ("a", "accepted", 2, 2),
            ("b", "blocked", 3, 2),
            ("old", "accepted", 20, 1),
        ] {
            store.connection.execute(
                "INSERT INTO tasks(id,project_id,title,outcome,status,weight,scope_revision) VALUES (?1,'p1',?1,'done',?2,?3,?4)",
                params![id, status, weight, scope],
            ).unwrap();
            if status == "accepted" {
                store.connection.execute(
                    "INSERT INTO criteria(id,task_id,assertion,verifier,accepted_at,verified_by)
                     VALUES (?1,?2,'works','owner',CURRENT_TIMESTAMP,'owner')",
                    params![format!("criterion-{id}"),id],
                ).unwrap();
            }
        }
        let metrics = store.project_metrics("p1").unwrap();
        assert_eq!(metrics.verified_completion_basis_points, 4000);
        assert_eq!(metrics.blocked_count, 1);
    }
}
