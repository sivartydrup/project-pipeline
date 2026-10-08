# Delivery plan

Status: proposed implementation sequence. `plan/backlog.json` is the executable index of work items; this document explains the gates and expected vertical slices. Work begins after the owner approves this baseline.

## Guiding execution rules

- Build one usable path at a time: project → plan → agent run → evidence → review. Avoid building disconnected feature screens.
- Keep the app compileable at the end of each slice. Every task has acceptance criteria, verification, and evidence.
- Prefer a working vertical slice over broad placeholder interfaces. Manual workflows are acceptable for modules whose external connectors come later.
- On Windows, verify the actual desktop binary and ConPTY; on the M1 Max, verify native ARM64 build and PTY. Linux CI remains active for portability.
- Stop implementation at a decision gate when a proposed product behavior or architecture choice is not approved.

## Phase 0 — validate foundations

**Outcome:** a confirmed architecture with no hidden blocker in the two most important integrations.

1. Record baseline approvals and versioned scope (P00).
2. Spike `eframe`/`egui` with resizable panels, large list, keyboard and scaling checks on Windows (P01).
3. Spike PTY with interactive shell, ANSI/Unicode, resize, and clean exit on Windows (P02).
4. Probe installed OpenCode and Pi versions and test their process APIs against minimal run/event/stop scenarios on Windows (P03). Implement OpenCode first unless the owner selects Pi. Use macOS CI as soon as code exists; repeat hands-on checks when the M1 Max is available.

**Gate:** owner reviews Windows screenshots/short recording of UI and terminal spike plus compatibility report. If accessibility or terminal constraints make egui unsuitable, propose a documented alternative that still keeps Rust throughout before proceeding. The Mac hands-on check remains a release gate.

## Phase 1 — local product skeleton

**Outcome:** native app opens a real local project and persists portfolio state.

5. Create Rust workspace, three-OS CI workflow, and lint/test/build scripts (P04). Run the matrix after this project has its own remote and before release acceptance.
6. Implement SQLite schema, migrations, repositories, backup/export, and activity history (P05).
7. Implement project create/import, Git identity, folder validation, and portfolio overview (P06).
8. Implement the three-pane shell and usable PTY tabs (P07).

**Gate:** on Windows, user can add two projects, switch between them, run shell commands in the correct folders, restart, and see persisted state. macOS and Linux CI build/tests pass; repeat the journey on the M1 Max before release.

## Phase 2 — plan and review

**Outcome:** the owner can approve a brief and task graph and inspect decisions.

9. Build guided idea intake, research record, versioned brief, and approval (P08).
10. Build task graph, dependencies, acceptance criteria, scope revision, and verified completion calculation (P09).
11. Build decisions, owner inbox, activity timeline, evidence/artifact views, and revision-bound approval UI (P10).

**Gate:** an idea can become an approved, dependency-ordered plan; changing approved scope is visible and requires a new approval. Completion remains zero until tasks are accepted with evidence.

## Phase 3 — autonomous task loop

**Outcome:** one approved task can be safely completed by an agent with full traceability.

12. Implement scoped loopback API/CLI, token policy, idempotency, revision checks, and audit (P11).
13. Implement scheduler, run state machine, checkout/worktree lifecycle, crash recovery, and policy gates (P12).
14. Implement OpenCode adapter, session/event mapping, permission handling, steering/stop, and contract tests (P13).
15. Connect task packet, agent updates, diffs, tests, review, and acceptance to UI (P14).
16. Implement Pi RPC adapter using the same contract (P15).

**Gate:** on Windows, the app can choose an approved task, launch each supported harness, show live progress and terminal output, receive task/decision updates, stop/recover a run, review a real diff and test result, and accept only when criteria are met. A denied risky action must remain denied. Repeat the full journey on the M1 Max before release.

## Phase 4 — full lifecycle record

**Outcome:** design, QA, release, launch, and operation fit the same project model.

17. Add design/UX artifact and asset workflows, screenshots, copy, and accessibility reviews (P16).
18. Add release candidate, build provenance, quality/security/privacy checks, manual deployment and rollback record (P17).
19. Add launch/marketing plan, content/assets, distribution tasks, metrics, feedback, bugs, and incidents (P18).

**Gate:** a sample app project can be represented from idea through live release and post-launch work, including manual steps where automation is not yet connected.

## Phase 5 — release hardening

20. Cross-platform packaging, migration/restore testing, large-log performance, secret redaction, accessibility, documentation, and installation smoke tests (P19).

**Release candidate gate:** Windows and Apple Silicon installers/binaries install and launch; onboarding, import, terminal, planning, agent runs, recovery, review, export, and manual release flow pass on both. Linux CI passes. No critical security or data-loss issue remains. Signing/notarization are completed when identities are available; otherwise distribution limits are stated explicitly.

## Later roadmap, ordered by value

1. Provider adapters for Git hosting, CI, deploy platforms, analytics, research/design tools, and marketing channels. Each has a capability declaration and scoped approval for external effects.
2. Parallel isolated agents with conflict detection and merge review.
3. Optional encrypted sync and collaboration roles, built on the existing event/revision model.
4. Budget controls, scheduled unattended runs, and owner-defined notification windows.
5. Template marketplace and reusable product playbooks, only after the base workflow proves useful.

## Measurement during development

Track time from idea to approved brief, time owner spends per approval, proportion of agent tasks accepted on first review, blocked time awaiting owner input, recovery success after interrupted runs, and percentage of completion claims with valid evidence. These are product quality measures, not incentives to hide blockers.

## Change control

A task may be split into smaller items without changing its acceptance outcome. New features or materially changed policy require a decision record, scope revision, dependency update, and owner review. An agent must not treat the existence of an item in the backlog as approval to deploy or publish externally.
