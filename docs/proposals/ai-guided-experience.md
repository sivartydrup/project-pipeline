# AI-guided product studio — UX redesign proposal
Status: PROPOSED, awaiting owner design/scope approval. Does not supersede approved baseline until approved.
Date: 2026-10-12

## Problem
Manual user testing found technical terminology, an expectation that users already know the next workflow step, and an interface needing substantial visual and interaction redesign. Preserve the working Rust/SQLite/agent foundations, auditability, and scoped approvals; redesign the human-facing experience.

## Product principles
1. Intent before implementation: user says what they want in plain language; AI drafts structured proposals.
2. Next step before navigation: one primary recommended action with explanation and optional alternatives.
3. Progressive disclosure: ordinary mode shows goals, outcomes, risks, and decisions; developer mode reveals task IDs, hashes, logs, Git/worktrees, terminal and protocol detail.
4. Preview before mutation: assistant read-only by default; explicit owner confirmation for consequential project changes; existing exact revision/approval and external-effect gates still apply.
5. Evidence without jargon: show understandable test outcomes and link to raw proof.
6. Recovery over dead ends: empty, blocked, failed, offline and interrupted states always offer safe next actions.
7. Accessibility: keyboard, screen reader, scalable text, contrast, focus, reduced motion, and no colour-only status.

## Information architecture
Global: Home (portfolio + attention inbox), Projects, Activity, Settings. Each project defaults to Home: project goal, stage journey, progress from verified approved scope, current agent status, one next-action card, decisions requiring input, and recent outcomes. Secondary views: Roadmap, Work, Design, Releases, History. Advanced workspace toggles technical detail (terminal, logs, exact artifacts, API/CLI, diagnostics). Keep existing capabilities discoverable; avoid removing underlying records.

## Guided journeys
New project: welcome -> describe idea conversationally -> AI asks only essential missing questions -> draft brief with assumptions/sources -> visual review and approve exact revision -> proposed roadmap -> approve scope -> recommended first task -> choose harness/model and cost guardrails -> run -> plain-language review with tests/diff drill-down -> accept/request changes -> next step.
Existing project: import -> inspect without writing -> summarize detected repo/state and unknowns -> confirm intent -> propose brief/roadmap alignment -> owner approval -> next step.
Returning user: project snapshot -> what changed -> one recommended action and why -> alternatives (review, resume, change plan, investigate blocker).
Blocked/failure: explain what happened, what remains safe, and recover/retry options; never pretend a run completed.
Release: guided readiness checklist and approval, no automatic publish.

## Assistant interaction contract
Persistent project-scoped assistant composer plus optional expanded conversation. Supported intents: explain status, recommend next step, brainstorm idea, revise brief, propose tasks, explain tests, triage blockers, compare options, prepare release. Context is retrieved from domain services with project/revision IDs, not inferred from UI text. Assistant replies separate observed facts, recommendations, and unknowns, and link to records.
Use typed proposed actions: target object, expected revision, effect summary, before/after preview, risk class, required approval, and undo/recovery path. Read-only questions execute without mutation. Draft changes require explicit user apply/approval as appropriate; approved scope and risky effects retain existing gates. Reject stale proposals and re-preview after conflict. Never parse prose as authority to bypass policy. Untrusted repository content and model output cannot grant permission. Do not imply autonomous actions if no compatible harness/model configured.
Support streaming, cancel, retry, error states, conversation retention/export controls, clear model/provider and potential cost information; no secret leakage into prompts/logs.

## Visual design system direction
Modern, calm, professional product studio; friendly but not playful/childish. Strong typographic hierarchy, spacious layout, clear contrast, restrained accent, rounded but disciplined surfaces, consistent 8pt spacing rhythm, icon+text labels, purposeful micro-interactions, excellent dark/light themes. Prioritize content and actionable cards over dense tables. Establish tokens for typography, spacing, colours, elevation, radius, states, responsive breakpoints. Build reusable components: AppShell, ProjectSwitcher, JourneyStepper, NextAction, AttentionInbox, AIComposer, ChatTurn, ProposalPreview, DecisionCard, EvidenceSummary, StatusTimeline, EmptyState, ErrorRecovery, AdvancedPanel.
Do not treat a cosmetic reskin as sufficient. Validate accessibility and usability with actual Windows and Apple Silicon native builds; investigate egui limitations and propose a separate owner decision if toolkit cannot meet minimum usability requirements. Do not silently change the approved Rust/egui choice.

## Key UX acceptance journeys
- First-time user can create an idea and reach a reviewable AI-drafted brief without knowing 'epic', 'revision' or 'scope'.
- Existing repo import explains findings and recommends an actionable next step without modifying source.
- Owner asks 'What should I do next?' and receives a grounded recommendation citing project state, prerequisites and blockers.
- 'Add subscriptions' produces an inspectable proposed change; no approved scope mutation before approval.
- Agent finishes work: user sees summary, test status, what changed, risk, and Accept / Request changes; raw diff is optional.
- Missing harness/model/network clearly degrades to manual planning without misleading claims.
- Keyboard-only and scaled-text use is practical; interrupted runs preserve recovery options.

## Success metrics (targets to validate in usability tests, not current measurements)
First idea-to-draft without technical help; time to locate next action; task review comprehension; zero unapproved state mutations; keyboard completion; reduced navigation steps and confusion reports.

## Approval questions
Confirm: (1) AI-first home + workflow-forward navigation; (2) progressive advanced workspace; (3) design direction; (4) assistant scope and provider/harness strategy; (5) whether to prioritize this redesign before P16–P19. Until approved, only discovery/prototyping and documentation are authorized.

## Direction selected in design discussion (2026-10-12)
Owner preference: **guided dashboard with an always-available AI assistant**, rather than chat-only or a permanently equal split. This is a design preference, not final scope or implementation approval.

### Shell behavior
- Global portfolio and project dashboard are the default; an AI composer remains visible and keyboard accessible.
- Assistant expands into a contextual side panel or focused conversation view without losing project context; narrow layouts use a drawer.
- Project home foregrounds a single explainable next action, attention queue, verified progress, current activity and stage journey.
- Stage-specific guidance adapts for idea, discovery, planning, design, build, test, release and operation; users may override recommendations.
- Preserve advanced developer tooling behind explicit progressive disclosure, not removal.

### Design review checkpoints
1. Approve information architecture and core journeys.
2. Review high-fidelity light/dark prototypes for global home, project home, assistant expanded, new idea, review and recovery.
3. Validate usability and accessibility on real native builds before broad implementation.
4. Only then authorize and sequence UX backlog tasks against P16–P19.

### Assistant usability requirements
- Context includes selected project and currently focused object, clearly indicated to user; allow changing context.
- Offer actionable suggested prompts and explicit previews, not invisible autonomous state changes.
- Show what was observed vs inferred, why next step is recommended, and which approvals are required.
- Preserve project workflow when AI provider is unavailable; deterministic guidance and manual controls remain.
