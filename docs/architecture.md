# Technical architecture

Status: proposed. This architecture implements the requirements in [product-spec.md](product-spec.md) and is refined by early technical spikes.

## 1. Process and component boundaries

```text
┌──────────────────────────── Rust desktop process ────────────────────────────┐
│ egui/eframe UI: portfolio | management workspace | terminal renderer       │
│ Application services: projects, plans, decisions, approvals, runs, releases │
│ Event bus + scheduler + policy engine + artifact index + PTY manager        │
│ SQLite repository + migrations + audit log                                  │
│ Local loopback API / capability-scoped bridge for agent updates             │
└───────────────┬───────────────────────┬──────────────────────────────────────┘
                │                       │
       project folders / Git     harness adapter processes
                               OpenCode server; Pi RPC later
```

The UI never writes directly to SQLite. Application services enforce state transitions, revision checks, policy, and event recording. The same commands serve UI actions and agent API calls. Run events are normalized before persistence and display.

## 2. Proposed Rust workspace

```text
Cargo.toml
crates/
  pipeline-domain/       IDs, entities, validation, state machines
  pipeline-store/        SQLite repositories, migrations, audit queries
  pipeline-engine/       scheduler, policy, run orchestration
  pipeline-adapters/     harness trait and OpenCode/Pi implementations
  pipeline-terminal/     PTY lifecycle and terminal model
  pipeline-api/          loopback API and scoped capability tokens
  pipeline-app/          eframe desktop executable and UI
  pipeline-cli/          project/task/decision commands for scripts and repair
```

This is a modular monolith: one distributable desktop app with helper processes only for harnesses and shell sessions. Crate boundaries can be collapsed if they create unnecessary ceremony, but the domain/service/adapter separation must remain.

## 3. Technology choices and validation

| Concern | Proposed choice | Validation before commitment |
| --- | --- | --- |
| Desktop UI | `eframe`/`egui` | Three-pane resizing, keyboard navigation, high DPI, accessibility, 10k-row virtual list |
| Metadata | SQLite via `rusqlite` with bundled SQLite | Migrations, concurrent readers, backup/restore, corruption handling |
| Async/processes | Tokio plus channels | UI responsiveness and clean cancellation |
| Terminal | Cross-platform PTY such as `portable-pty`; parser/model such as `alacritty_terminal` | Windows ConPTY, macOS/Linux PTY, ANSI, Unicode, resize, large output |
| Source control | Git CLI behind an adapter | Worktree creation, dirty repository import, diff capture, cleanup |
| OpenCode | Local server HTTP API + event stream | Version/capability negotiation, session lifecycle, permission events |
| Pi | Long-lived `--mode rpc` JSONL subprocess | Framing, event mapping, resume, error recovery |
| Agent write bridge | Local loopback HTTP API plus short-lived run token; CLI fallback | Bind only loopback, scoped auth, replay protection, stale revision behavior |

The OpenCode server exposes sessions, events, diffs, and permission responses; Pi documents RPC for non-Node integrations. These are integration surfaces, not guarantees of stable semantics. Pin tested versions, negotiate capabilities at startup, and maintain adapter contract tests.

## 4. Domain model

All primary records have UUID, created/updated timestamps, actor, and integer revision. Foreign keys are enforced. Important records are append-only or revisioned.

| Entity | Key fields / relationships |
| --- | --- |
| `Project` | name, path, repo identity, stage, health, active scope revision, brief revision |
| `BriefRevision` | structured sections, status, parent revision, approval link |
| `ResearchFinding` | claim, source URL/file, access date, confidence, project/brief links |
| `Milestone` / `Epic` | project, outcome, order, target, scope revision |
| `Task` | parent, status, weight, priority, risk, owner, acceptance set, revision |
| `Dependency` | from task, to task, type (`blocks`/`informs`); acyclic for `blocks` |
| `Criterion` | task, assertion, verifier type, required evidence |
| `Decision` | question, alternatives, selected option, rationale, impact, status, supersedes |
| `Approval` | subject type/id/revision/hash, policy class, approver, expiry/revocation |
| `Artifact` | kind, URI/path, content hash, MIME, source run/commit, metadata |
| `AgentRun` | task, harness, session id, checkout, state, token scope, start/end |
| `RunEvent` | run, sequence, timestamp, normalized type, payload/redacted raw reference |
| `TestResult` | task/run, command, exit status, environment, artifact/log, commit |
| `ReleaseCandidate` | project, version, commit, build artifacts, checks, approval, target |
| `ActivityEvent` | actor, operation, object/revision, before/after digest, correlation id |

Never infer accepted task status solely from agent text. Acceptance is a domain transition requiring criteria evidence and a verifier or owner action. Completion can be recomputed from accepted tasks and scope revision.

## 5. State and event rules

- Task transitions: `draft → ready → running → review → accepted`; any active state may enter `blocked`; a blocked task resumes to its prior state after its blockers are resolved. `cancelled` is terminal. Reopening an accepted task creates a new revision and adjusts verified completion.
- Run transitions: `queued → starting → running → waiting_for_input/review → completed/failed/cancelled/interrupted`. Startup reconciliation marks abandoned `running` records as `interrupted`.
- Every state change and approval is one database transaction with an `ActivityEvent` and outbox event for UI refresh. External actions record intent before execution and result afterward.
- Event payloads are schema-versioned. Incompatible unknown events remain stored but do not mutate domain state.

## 6. Agent adapter interface

```rust
trait AgentAdapter {
    fn probe(&self) -> CapabilityReport;
    fn start(&self, request: RunRequest) -> RunHandle;
    fn events(&self, run: RunId) -> EventStream;
    fn send(&self, run: RunId, message: SteeringMessage) -> Result<()>;
    fn stop(&self, run: RunId) -> Result<()>;
    fn recover(&self, session: ExternalSessionId) -> RecoveryReport;
}
```

The adapter reports capabilities such as session resume, steering, permissions, diff, and structured tool events. The engine degrades features explicitly when a harness lacks them. It must not fabricate an approval event from plain text. Sessions are one per run/task unless an adapter proves safe reuse. Commands never embed secrets in command-line arguments.

## 7. Security and boundaries

- **Project access:** the owner selects root folders. Canonicalize paths on each write and reject traversal or symlink escapes. Agent OS-level permissions may exceed app policy; isolated checkout and explicit harness permissions reduce, but do not eliminate, that risk. The UI must describe this honestly.
- **Write API:** bind to loopback only; issue a short-lived token scoped to project, run, allowed operations, and expiry. Authenticate each call, enforce optimistic revision and rate limits, and log actor/correlation ID. Prefer passing token through a protected environment channel or inherited pipe; never persist it in project files.
- **Commands:** classify by effects, not only string matching. Unknown commands with elevated or external effects require review. Install/build/test in the project is allowed under the selected autonomy policy, while destructive filesystem changes, production changes, publishing, spending, and credential access require explicit gates.
- **Secrets:** use OS credential manager or the harness's provider auth. Redact known secret values and sensitive environment variable names from persisted output. Tests must include secret-leak checks.
- **Prompt injection:** research pages and project files are data. Only owner-approved policy and task packets govern the agent. Tool output cannot grant new permissions.
- **Audit:** append-only events include hashes of key artifacts and approvals. Export omits secrets and live tokens.

## 8. Cross-platform delivery

CI builds and tests on Windows, Apple Silicon macOS, and Linux. Hands-on native smoke tests and packaging gates prioritize Windows and Apple Silicon macOS; Linux CI protects portability and its release support follows primary-platform validation. Native smoke tests cover first launch, project import, SQLite migration, PTY shell, harness probe, interruption recovery, and packaging. Shell selection is OS-aware (PowerShell/cmd on Windows; user shell on macOS/Linux). No code assumes `/bin/sh`, POSIX paths, case-sensitive filesystems, or Unix PTY behavior. Platform-specific helpers remain behind traits. Signed packages and notarization are release-gate items once distribution identity is available.

## 9. Source references checked for this proposal

- [eframe native runner](https://docs.rs/eframe/latest/eframe/fn.run_native.html) and [egui](https://docs.rs/egui/latest/egui/)
- [rusqlite and bundled SQLite](https://docs.rs/crate/rusqlite/latest)
- [OpenCode server API](https://dev.opencode.ai/docs/server/) and [SDK](https://docs.opencode.ai/docs/sdk/)
- [Pi RPC protocol](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/rpc.md)
- [portable-pty](https://docs.rs/portable-pty/latest/portable_pty/) and [alacritty_terminal](https://docs.rs/alacritty_terminal/latest/alacritty_terminal/)
