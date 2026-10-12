# UX04 implementation design and approval checkpoint

Status: **architecture and spending decision pending** (12 October 2026). This is
the required pre-implementation audit for the UX04 handoff; no application code
or provider calls have been changed.

## Contract audit

- `ProjectEngine` already exposes read-only `list_projects`, `load_discovery`,
  `load_plan`, `load_review`, `task_run_reviews`, and `next_action` paths. A
  bounded context assembler can use these, restrict every call to the selected
  project ID, cite persisted IDs and revisions, and reject inconsistent or
  stale snapshots. It need not read project files, SQLite directly, or secrets.
- The modern shell has a compact draft composer and collapsible Guidance panel.
  Drafts are isolated by project ID and a separate portfolio key. No
  conversation history is persisted. The legacy shell and Advanced controls
  remain available behind `PIPELINE_MODERN_SHELL=1`.
- `AgentAdapter` is a task-run contract (`start`, `events`, `steer`, `stop`,
  `recover`, permission replies). It has no conversational answer or usage-cost
  contract. OpenCode currently prompts its `build` agent in an isolated
  checkout; the run config permits reads and edits and asks for shell commands.
  Pi launches with read, edit, write, and shell tools plus a policy extension.
  Both run paths supply a scoped write bridge. Reusing either path for a
  supposedly read-only conversation would grant unnecessary capabilities.
- Provider/model fields are transient run inputs, initially blank. The run UI
  warns that charges may apply and uses an **observed** token stop that can be
  exceeded between observations. There is no approved chat model, verified
  price, enforceable monetary cap, or chat-specific authorization. The Pi
  extension's 30-call stop applies to task runs, not assistant conversations.

## Proposed bounded implementation after owner decision

1. Add a read-only `AssistantContext` assembled solely from selected-project
   service reads. Include a compact overview, current brief/plan revision and
   approval state, bounded tasks/decisions/run evidence, and the deterministic
   next action. Carry record IDs/revisions with every factual claim. Limit
   characters and record counts; redact credential-shaped strings before any
   display or provider request. Do not include local paths unless essential.
2. Keep project conversation state in memory, keyed by project ID (portfolio
   separately), with bounded turns and text. Capture project ID and source
   revisions when sending. Discard late results from a different selected
   context and mark changed revisions stale. A reply distinguishes observed
   facts, interpretation, and unknowns. Modification requests explain that a
   future typed proposal and owner review are required.
3. Introduce a narrow `ReadOnlyConversation` interface with mocked adapter
   tests. A live implementation must use an isolated, tool-free provider
   session with no write bridge or checkout tools, explicit instruction/data
   separation, hard response/time bounds, cancellation, and clear provider,
   model, and actual available usage/cost disclosure. A normal chat Send must
   be explicit; navigation must never call a provider. If safe live access or
   verified permitted pricing is unavailable, show deterministic offline
   guidance and the precise unavailable/error state.
4. Render text as inert egui labels; never parse model output into domain
   commands. Preserve Advanced, scheduler, approvals, audit, CLI, PTY, and
   default legacy shell. Add no schema migration.

## Decision needed before application implementation

**UX04-D01 (proposed):** authorize a new isolated, tool-free conversational
provider path, separate from approved task-run adapters, with explicit
per-question Send and only a model whose use and price the owner has approved.
No automatic paid-model choice is implied. The alternative is to implement
bounded context, conversation UI, mocked interface, and deterministic offline
answers now, leaving live model responses unavailable until a separate
provider and spending decision. Both preserve the current approval policy.

The UX04 handoff explicitly requires stopping before implementation when live
conversation needs a new authorization boundary, spending decision, or
architecture change. This audit finds all three unresolved for the existing
task-run route. Owner selection of the path and any live provider/model
authorization is therefore required before application code is changed.

## Planned evidence after approval

Mocked isolation, citation, cancellation, malformed-response, stale-context,
redaction, prompt-injection, no-mutation, and usage-disclosure tests; Windows
native 1024/1440 light/dark and 100/200% screenshots; keyboard and AccessKit
checks where feasible; format, Clippy, workspace tests, and production build
logs. M1 Max and real screen-reader checks remain later gates. Live model tests
will be reported only if an approved provider is available.
