# Local storage, migrations, and recovery

The app stores workflow metadata in one local SQLite database. Source code, large assets, and Git history remain in project folders. Schema v1 is in `crates/pipeline-store/migrations/001_initial.sql` and covers projects, briefs, research, milestones, epics, tasks, dependencies, criteria, decisions, approvals, runs, artifacts, test results, releases, and activity events. Schema v2 adds the detected Git root to each project. Existing v1 databases migrate with a pre-upgrade backup.

## Mutation contract

The `Store` service opens the connection with foreign keys enabled. `create_project` and `rename_project` write the project and its activity event in one transaction. The rename requires an expected revision and returns a conflict with the actual revision when stale. Activity rows have triggers blocking update and delete. The project foreign key on activity restricts project deletion; archiving is the intended workflow.

Other entities currently have schema only. Their service methods will be added with the same transaction, revision, and audit pattern before agents can mutate them. Agents never get a raw database connection.

## Migration behavior

`PRAGMA user_version` identifies the schema. Each migration executes in a SQLite transaction and sets the new version only after the SQL succeeds. A failed migration rolls back both schema changes and the version. A database newer than the running app is rejected. Before an upgrade from a versioned database, `Store::open` creates a sibling `pre-vN.sqlite` backup and refuses to overwrite an existing backup. The recovery UI will be added during app integration.

## Backup, restore, export

`backup_to` uses SQLite's online backup API and rejects an existing destination. `restore_from` first opens the backup read-only, runs `PRAGMA integrity_check`, rejects a schema newer than this app, then restores through SQLite's backup API. The caller must hold exclusive ownership of the target database and should create a pre-restore backup. JSON export includes all workflow tables and schema version. No credential fields exist in v1; free-text redaction remains an API/release gate.

The tested cases are: atomic mutation and stale revision, injected activity-write failure rollback, failed migration rollback, v1-to-v2 upgrade with pre-upgrade backup, file close/reopen, immutable activity event, and backup/restore/JSON round-trip. Large-database backup performance remains a release check.
