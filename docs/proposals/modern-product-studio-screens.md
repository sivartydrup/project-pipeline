# Modern Product Studio — screen and interaction specification
Status: DESIGN PROPOSAL — owner selected visual direction, not implementation approval.
Date: 2026-10-12

## Decisions confirmed as design preferences
- Modern product studio: elegant, spacious, approachable, premium productivity UI.
- Guided dashboard as primary experience; AI composer always accessible, conversation expands on demand.
- Balanced autonomy: proactive recommendations, preparation, and permitted automatic continuation of approved work; preserve explicit review, scope and risky-action gates.
- Retain native Rust and egui baseline unless owner explicitly approves a change after a toolkit feasibility assessment.

## Navigation and shell
Desktop global nav: Home, Projects, Inbox, Activity, Settings. Project nav: Overview, Roadmap, Work, Design, Releases, History. Advanced workspace reveals terminal, raw evidence, technical IDs, run diagnostics and CLI/API. Navigation should be task-oriented, not mirror backend modules.
Global assistant composer is visible across all screens; contextual panel opens without destroying page state. On narrow windows use an overlay/drawer. Keyboard shortcut and focus return must be specified and tested.

## Screens
### S01 Portfolio home
Purpose: quickly answer what needs attention across projects.
Content: greeting, one highest-priority focus card with reason, project cards (stage, verified completion, blockers, current activity), attention count, new/import project CTA.
Actions: open project, review focus item, create/import, ask assistant.
Empty: 'Start with an idea' and 'Bring an existing project'; no fake metrics.
### S02 Project overview
Purpose: answer what next, what AI is doing, what requires input, and where project is headed.
Content order: project name/goal; next action with why/alternatives; run state and pause controls; attention; verified progress; stage journey; upcoming work; activity.
States: ready, running, awaiting review, decision needed, paused, blocked, interrupted, offline. Status labels must not claim an agent is working when only preparing or waiting.
### S03 Assistant expanded
Context chip displays current project/object; explicit change context. Streaming response with stop/retry. Suggested prompts. Actionable proposal cards show observed facts, assumptions, proposed mutations, revision, risk and approval requirements. Read-only assistant cannot mutate state. If no provider is configured, offer deterministic guidance and setup.
### S04 Guided idea and import
Conversational idea intake: capture goal/audience/problem/constraints in stages; avoid demanding jargon. AI prepares a brief with highlighted assumptions, owner edits and exact-revision approval. Existing repository import is read-only until user authorizes changes; present observed repo state and unknowns.
### S05 Roadmap
Visual milestones and stage progress grounded in accepted scope/evidence. Show ready/blocked/in-progress/review tasks with dependency explanations. Natural-language change request opens draft preview; approval required to materialize scope.
### S06 Active work
Plain-language task goal, harness/model, live activity, limits, elapsed time, pause/stop, pending exact action approvals; expandable technical logs. Never imply live streaming when unavailable. Interruptions retain worktree/evidence and offer recovery.
### S07 Work review
Summary: user-visible outcome, acceptance criteria, checks and failures, risks, before/after preview, evidence completeness. Primary actions Accept and Request changes. Raw diff/logs expandable. No agent self-acceptance; missing evidence blocks acceptance.
### S08 Attention inbox
Prioritize blocking decisions and reviews, explain effect of delay, batch only independent safe decisions, revision-bound confirmation, immutable history.
### S09 Settings and autonomy
Modes Suggest only / Guided Auto / Custom. Explain what can happen without prompting; per-project harness/model, enforceable token/call/time/cost guardrails, permissions, review cadence. High-risk operations always gated. Fail closed when budget cannot be enforced.
### S10 Recovery and offline
Clearly distinguish paused, blocked, interrupted, unavailable harness and provider offline. Explain last verified event, what is preserved, safe retry, diagnostics and manual fallback.

## Component library
AppShell, GlobalNav, ProjectSwitcher, AssistantComposer, AssistantDrawer, ContextChip, NextActionCard, AttentionCard, JourneyStepper, ProgressSummary, AgentRunCard, AutonomyControl, ProposalCard, RevisionDiff, ApprovalGate, EvidenceSummary, StatusBadge, EmptyState, RecoveryCard, Toast, Modal.
Each needs loading, empty, error, focus, disabled, long-text and narrow-window states.

## Initial visual tokens for prototype, not final production approval
Neutral warm light surfaces and charcoal dark surfaces; restrained teal or blue accent; semantic success/warning/error colours never used alone; 8pt spacing scale with 4pt increments; 12–16px card radii; strong typographic scale and readable line lengths. Avoid heavy gradients, excessive glass, novelty decoration and dense tab grids. Ensure accessible contrast and keyboard/focus feedback. Native egui font/rendering/accessibility feasibility must be verified before claiming compliance.

## Interaction rules
- Every recommended action: title, reason, expected outcome, current prerequisite and alternative.
- Assistant proposes changes as typed draft actions with before/after and revision; applying invokes existing domain service and exact approval gates.
- Auto-run begins only for approved, ready, unblocked tasks and only when provider/model, scoped permissions, limits and one-active-code-run invariant pass.
- After agent submission, task awaits human acceptance; scheduler may choose a different eligible approved task if policy permits, but never treat submission as acceptance.
- Stop and pause semantics are explicit; retry bounded, audited and never repeats external effects without fresh permission.
- Never fabricate metrics, tests, model availability or completion.

## Design validation and Codex handoff
1. UX00 audit actual running UI, terminology and current user journeys; collect screenshots.
2. UX01 prototype S01–S10 in light/dark themes, verify egui feasibility, record owner feedback. Do not begin production reskin without design approval.
3. After approval, implement shell, deterministic recommendations, contextual read-only assistant, typed proposals, guided journeys and reviews in dependency order defined by plan/ux-redesign-backlog.json.
4. Validate keyboard and scaled text, Windows and M1 Max, actual provider-offline behaviour, safe approvals, regression tests and verified evidence.
5. Preserve approved baseline backlog; owner must authorize integration and re-sequencing before implementation.

## Owner-confirmed assistant placement (2026-10-12)
The user agreed to a **collapsible right-hand assistant panel with an always-visible compact prompt bar**. This confirms layout direction, not blanket authorization to implement the redesign.
- Desktop default: project content remains primary; right panel expands for conversation and proposal previews, and collapses without losing draft/context.
- The prompt bar remains accessible when the panel is closed, on every primary screen.
- Resizing must preserve readability and usable minimum content width; on narrow windows assistant becomes a drawer, not a permanently squeezed column.
- Focus, keyboard toggle, screen-reader labels, close/escape, and restoration of context/drafts require explicit prototype checks.
- Prototype should demonstrate read-only questions, typed change proposal, approval preview, offline mode, and a running task while assistant panel is open.

## Immediate Codex handoff: discovery and prototype only
Codex may use this specification as input to prepare UX00 audit and UX01 prototype proposals. Do not alter production shell, change approved scope, or run unapproved work merely because this design document exists. First deliver: current UI screenshot inventory and pain points, detailed wireframes for S01/S02/S03/S07, egui feasibility/accessibility assessment, and high-fidelity interactive prototype or equivalent native spike for owner review. Await explicit owner authorization for implementation and backlog resequencing.
