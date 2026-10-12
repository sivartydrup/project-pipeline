# UX04 implementation design and approval checkpoint

Status: **architecture approved** by the owner on 12 October 2026. This was
the required pre-implementation audit for the UX04 handoff. The owner approved
an isolated, tool-free live chat interface, local inference endpoints and
compatible remote configuration, with paid usage denied unless an enforceable
provider spending limit exists. No paid model calls are authorized.

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

## Bounded implementation

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

**UX04-D01 (approved):** authorize a new isolated, tool-free conversational
provider path, separate from approved task-run adapters, with explicit
per-question Send and only a model whose use and price the owner has approved.
No automatic paid-model choice is implied. The alternative is to implement
bounded context, conversation UI, mocked interface, and deterministic offline
answers now, leaving live model responses unavailable until a separate
provider and spending decision. Both preserve the current approval policy.

The owner selected the isolated route. The implementation uses an explicit
OpenAI-compatible chat completions endpoint and model entered in the modern
shell; no model or endpoint is configured by default. Only explicit loopback
HTTP endpoints with a port may be called. Remote HTTPS configuration is
recognized but paid requests fail closed because this generic API cannot
verify a provider-enforced spending cap. A future provider-specific cap
adapter can be added without changing project records or approval policy.
Credentials, when needed, are read from a `PIPELINE_ASSISTANT_*` environment variable at Send
time and are never stored in app settings. No agent adapter, tool bridge, Git
checkout, shell, or domain mutation API is passed to the chat transport.

## Evidence and limitations

Mocked isolation, citation, cancellation, malformed-response, stale-context,
redaction, prompt-injection, no-mutation, and usage-disclosure tests; Windows
native 1024/1440 light/dark and 100/200% screenshots; keyboard and AccessKit
checks where feasible; format, Clippy, workspace tests, and production build
logs. M1 Max and real screen-reader checks remain later gates. Live model tests
will be reported only if an approved provider is available.

The Windows mocked and loopback fixtures cover streaming, malformed response,
tool-call denial, permission denial, cancellation, project separation, secret
redaction and paid remote rejection. The native screenshots use deterministic
offline guidance from an isolated sample database. There was no live model
call. Cancellation immediately stops UI delivery; the underlying synchronous
HTTP read may remain open until its overall request timeout (see the PR #5
refinement record). Local endpoint
operators must ensure the service does not proxy requests to a paid backend;
the app cannot inspect the server behind a loopback URL. Remote paid inference
remains unavailable pending a verifiable provider-side cap integration.

On 13 October the owner separately authorized up to 30 free-model requests
against their OpenRouter account. PR #5 now includes an exact-endpoint,
exact-free-variant path with a live zero-price check before each Send. It does
not permit paid remote models. See the [live smoke and residual risk](ux04-implementation.md).
