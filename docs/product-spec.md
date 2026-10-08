# Product specification

Version 0.1, 8 October 2026. Status: proposed baseline. Product name is provisional.

## 1. Goal and operating model

Project Pipeline is a desktop control room for a time-poor software product owner. It organizes multiple app and website projects, turns rough ideas into approved plans, coordinates coding agents, and keeps a durable account of decisions, actions, evidence, and remaining work through launch and maintenance.

The app is the **system of record for workflow**, while Git and project folders remain the source of truth for code. Agents can read the project, change files in an authorized workspace, and use the app's project API to create/update tasks, decisions, artifacts, questions, and evidence. The owner can see and correct these changes in one place.

### Success criteria

- An owner can create a project from a rough idea in under five minutes and get a reviewable brief and phased plan without manual project setup.
- After approval, an agent can complete a bounded implementation task, update the task, attach a diff and test evidence, and queue only decisions that truly require the owner.
- Every visible completion claim can be traced to acceptance criteria and evidence.
- The owner can understand current state and next required action for any project within one minute.
- A released product has a reproducible release record: commit, build, tests, deployment target, approval, and rollback notes.

## 2. Users and jobs

**Primary user:** one owner overseeing several software products with limited daily attention. They need concise summaries, prioritization, clear approvals, and recoverable automation.

**Agent user:** a local harness session working on a scoped task. It needs structured context, safe tools, project files, an update API, and a way to ask for decisions without blocking unrelated work.

**Later users:** collaborators and reviewers. Multi-user permissions and cloud synchronization are later-phase capabilities.

## 3. Information architecture and interface

The main window has three durable regions:

1. **Left portfolio panel** (resizable, collapsible): project search/filter; cards with name, stage, health, verified completion, active run, blocked count, next owner action, and last activity. Projects can be pinned/archived. A global inbox shows all approval requests and blockers.
2. **Upper-right management workspace**: tabs for Overview, Plan, Board, Design, Research, Decisions, Runs, Artifacts, Release, and Activity. The selected project is persistent when switching tabs. Use a details drawer for item inspection/editing, evidence, links, and history.
3. **Lower-right terminal** (resizable, hideable, multiple tabs): interactive shell in the selected project directory plus dedicated agent-run terminals. A user can inspect output, send input to a manually controlled terminal, stop a process, or open an external terminal. A running agent's terminal clearly distinguishes agent output from user shell activity.

On narrow windows, the portfolio collapses and the terminal can become a tab. Keyboard navigation, scalable text, dark/light themes, and accessible status labels are required. The UI must not rely on color alone for stage or failure.

### Portfolio status vocabulary

Project stages: `idea`, `discovery`, `planning`, `design`, `build`, `verify`, `release`, `operate`, `paused`, `archived`.

Health: `on_track`, `needs_input`, `blocked`, `at_risk`, `failed`. Health is calculated from unresolved blockers, failed checks, overdue owner decisions, and stalled runs; it is separate from stage.

Completion is a **verified weighted fraction** of approved scope. Each task has an explicit weight and can count only when its acceptance criteria are met and required evidence is attached. A project also shows counts of proposed, in-progress, review, blocked, and accepted tasks. New approved scope changes the denominator and is recorded as a scope revision. This avoids presenting an agent's subjective percentage as fact.

## 4. Core workflow

### A. Capture and discovery

The owner enters a short idea, target user, desired outcome, constraints, and optional links/files. The app creates a project and a discovery checklist. An agent may research competitors, user needs, technical options, pricing, constraints, and risks, marking sourced findings versus hypotheses. Research records include source, date, summary, confidence, and relevance. Missing facts become questions in the owner inbox.

### B. Shape and approve

The agent drafts a product brief with problem, audience, value proposition, scope, non-goals, user journeys, success metrics, UX principles, architecture candidates, costs, risks, and milestone plan. The owner reviews a concise summary and can inspect every section. Approval captures the exact revision. Changes after approval create a new revision and delta requiring approval when material.

### C. Design and plan

The agent creates a dependency graph of epics and tasks. Every executable task has a desired outcome, context links, acceptance criteria, verification commands, deliverables, risk class, dependencies, estimate band, and owner-decision triggers. Design artifacts may include flows, wireframes, visual direction, component inventory, accessibility notes, copy, assets, and responsive states. The owner can accept, request edits, or override a decision with a recorded rationale.

### D. Build and verify

The scheduler selects unblocked approved tasks. The agent gets a scoped working directory, task packet, relevant decisions, and allowed tools. It implements in an isolated checkout where practical, runs checks, reports changed files and results, and updates the task. The app displays diff, screenshots, test results, and decision trail. Failures create repair tasks or blockers; they do not silently mark completion.

### E. Release and operate

The release workspace contains readiness checks, build provenance, version, changelog, security/privacy review, documentation, deployment configuration, smoke tests, monitoring, rollback steps, and owner approval. Later automation can publish through provider adapters. Operational work includes bugs, user feedback, analytics, incidents, maintenance, dependencies, and planned improvements. Marketing work includes positioning, landing page, launch copy, distribution plan, campaign assets, and measurement.

## 5. Required modules

| Module | V1 behavior | Later expansion |
| --- | --- | --- |
| Portfolio | Projects, stages, health, verified completion, inbox | Teams, cross-project capacity, cloud sync |
| Brief & discovery | Guided idea intake, versioned brief, sourced research | Interview/research connectors |
| Plan | Epics/tasks/dependencies, acceptance, approvals | Estimates and scheduling optimization |
| Design | Artifact library, UX flows, visual review, decision links | Figma/design-tool integration |
| Agent runs | OpenCode adapter, live stream, pause/cancel, task updates | Pi adapter, parallel runs, remote workers |
| Terminal | Interactive PTY, tabs, history, resize | Split terminals and sharing |
| Review | Diff/evidence/decision/approval views | Inline visual annotations |
| Quality | Test runs, accessibility/security/performance check records | Provider-specific CI and audit integrations |
| Release | Release checklist and artifact provenance | Deployment connectors and rollback execution |
| Growth | Positioning, launch tasks, content/assets, metrics records | Campaign service integrations |
| Operations | Feedback, bug/incident tasks, maintenance cadence | Telemetry integrations |

The model supports all lifecycle modules from the outset; the first release's working automation is deliberately narrower. A module marked for later expansion must still have a clear manual workflow in v1, so the end-to-end project record does not break at release or marketing.

## 6. Functional requirements

### Projects and planning

- Create/import a project from an empty folder or existing Git repository; preserve existing files.
- Show and edit project brief, goals, constraints, risks, links, stage, and activity.
- Create epics, tasks, subtasks, dependency edges, milestones, and tags; reject cycles.
- Support task statuses `draft`, `ready`, `running`, `review`, `accepted`, `blocked`, `cancelled`.
- Capture acceptance criteria as discrete checkable items with evidence references and verifier.
- Calculate completion from accepted task weights within an approved scope revision.
- Allow owner edits while preventing lost updates with revision checks and visible conflict resolution.

### Decisions and approvals

- Record every material agent recommendation and decision with alternatives, reason, evidence, impact, and reversible/irreversible classification.
- Show owner inbox items ordered by urgency and impact, with a concise recommendation and consequences.
- Approval is explicit, scoped, revision-bound, revocable where possible, and logged. An approval does not silently extend to newly expanded scope.
- Batch low-risk decisions for review; interrupt only for high-impact or blocking decisions.
- Maintain an append-only activity history of task transitions, agent commands, approvals, errors, and release actions; secrets are redacted.

### Agent and terminal

- Detect configured harness executables and versions; verify adapter capabilities before a run.
- Start, stream, steer, stop, and resume/recover supported agent sessions; represent unknown/disconnected state accurately.
- Give agents structured read/write operations for project objects, never raw SQLite credentials.
- Correlate each agent action, tool call, file diff, and task update with a run ID and actor.
- Spawn an interactive shell via native PTY, handle resize, Unicode, ANSI rendering, scrollback, copy/paste, and process exit.
- Keep the agent process and user terminal as separate sessions even when they share a directory.

### Artifacts and delivery

- Store artifact metadata and hashes; link files in the project, avoid silently copying large binaries into the database.
- Track test evidence with command, exit code, environment, timestamp, logs, and related commit.
- Record release candidates with exact Git revision, build artifact, target, checklist, approval, deploy result, and rollback instructions.
- Permit manual completion of design, marketing, and deployment tasks when no connector exists.

## 7. Non-functional requirements

- Native compiled builds and hands-on smoke tests for Windows and Apple Silicon macOS. Linux remains a CI portability target and gains release support after primary-platform validation.
- Local-first operation for core planning/review; agent inference and external research may require network access and show that dependency.
- SQLite migrations are versioned, transactional, backed up, and tested on upgrade; app never writes inside source trees except by explicit project operations.
- Crash recovery preserves project/task/decision data and identifies interrupted runs without claiming they completed.
- UI remains responsive during long agent runs, terminal output, indexing, and builds. Use bounded queues and virtualized histories.
- Secrets stay in OS credential storage or provider-managed auth, never in logs, task text, exported plans, or SQLite plaintext.
- Project-scope path checks prevent accidental writes outside the selected checkout; symlinks and canonical paths receive explicit handling.
- Exports use documented JSON and Markdown formats so the owner can leave the app without losing the project plan.
- Telemetry is off by default; any future diagnostic upload requires opt-in.

## 8. Acceptance journeys

1. **New idea:** create project → agent drafts brief and research → owner sees assumptions and sources → owner edits/approves revision → plan becomes executable.
2. **Autonomous task:** approved task becomes ready → agent runs in project checkout → updates task and decision log → tests run → review view shows diff and evidence → owner accepts; completion changes only after acceptance.
3. **Needs input:** agent encounters a product choice → creates decision request with recommendation → continues unrelated ready tasks → owner approves/changes choice → affected task resumes with recorded context.
4. **Failure/recovery:** agent or app crashes → run is marked interrupted → partial diff and logs remain → owner can inspect, retry from checkpoint, or discard isolated worktree.
5. **Release:** candidate references exact revision and checklist → failed checks block approval → approved candidate is deployed through a configured connector or recorded manual procedure → smoke result and rollback plan remain visible.

## 9. Explicit exclusions from first release

No hosted service, mobile app, team permissions, built-in model provider billing, autonomous production deployment, automatic purchasing, or promise of fully unattended software creation. The v1 focus is a reliable local workflow that makes human attention efficient.
