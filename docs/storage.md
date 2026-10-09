# Local storage, migrations, and recovery

The app stores workflow metadata in one local SQLite database. Source code, large assets, and Git history remain in project folders. Schema v1 is in `crates/pipeline-store/migrations/001_initial.sql` and covers projects, briefs, research, milestones, epics, tasks, dependencies, criteria, decisions, approvals, runs, artifacts, test results, releases, and activity events. Schema v2 adds the detected Git root; v3 adds approved brief and research metadata; v4 adds immutable plans and scoped task evidence; v5 adds decision review fields; v6 adds scoped agent capabilities and replay protection; v7 adds managed checkouts, run checkpoints and limits, and policy requests. Existing versioned databases migrate with a pre-upgrade backup.

## Mutation contract

The `Store` service opens the connection with foreign keys enabled. `create_project` and `rename_project` write the project and its activity event in one transaction. The rename requires an expected revision and returns a conflict with the actual revision when stale. Activity rows have triggers blocking update and delete. The project foreign key on activity restricts project deletion; archiving is the intended workflow.

Brief revisions, owner approval, research findings, plan revisions, task review, criterion evidence, task acceptance, scoped agent writes, runs, managed checkouts, and policy requests mutate through the `Store` service with a transaction and activity event. Brief and plan approvals reference the exact revision and SHA-256 content hash. Saving a new revision leaves the active approval intact until the owner approves the new version. Sourced facts require a source URI and access date; hypotheses are explicitly marked. Plan approval materializes tasks, criteria, and dependencies into a new scope revision. Unchanged accepted tasks retain their evidence in the new scope; changed definitions or blocking prerequisites reset to ready. Agents never get a raw database connection.

## Migration behavior

`PRAGMA user_version` identifies the schema. Each migration executes in a SQLite transaction and sets the new version only after the SQL succeeds. A failed migration rolls back both schema changes and the version. A database newer than the running app is rejected. Before an upgrade from a versioned database, `Store::open` creates a sibling `pre-vN.sqlite` backup and refuses to overwrite an existing backup. Desktop startup now records interrupted runs and retained checkout state; the detailed run review UI follows in P14.

## Backup, restore, export

`backup_to` uses SQLite's online backup API and rejects an existing destination. `restore_from` first opens the backup read-only, runs `PRAGMA integrity_check`, rejects a schema newer than this app, then restores through SQLite's backup API. The caller must hold exclusive ownership of the target database and should create a pre-restore backup. JSON export includes workflow tables and schema version while omitting live capability and idempotency tables. Agent payload credential checks and read redaction are enforced at the bridge.

The tested cases are: atomic mutation and stale revision, injected activity-write failure rollback, failed migration rollback, versioned upgrades with pre-upgrade backup, file close/reopen, immutable activity event, backup/restore/JSON round-trip, exact brief/plan approval, pending revision preservation, research source enforcement, graph cycles, blocked scheduling, and evidence-based weighted completion across scope revisions. Large-database backup performance remains a release check.
