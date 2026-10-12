# UX04 read-only project assistant · implementation and evidence

12 October 2026 · Windows x86_64 · branch
`codex/ux04-readonly-assistant` · owner review pending

## Delivered

- The modern shell's compact composer and collapsible project assistant now
  accept explicit questions. The portfolio has separate offline guidance;
  each project retains its own draft and bounded in-memory conversation.
- `ProjectEngine::assistant_context` assembles up to 6,000 characters from
  the selected project's read-only service views: overview, latest brief and
  plan revisions, up to 12 active tasks, six recent decisions, and the
  deterministic next action. It excludes paths, raw logs, file contents,
  tokens and other projects. Context includes record IDs/revisions and a
  fingerprint used to flag a reply when source facts change.
- A separate OpenAI-compatible chat completions transport declares no tools,
  has no task-run adapter, checkout or write bridge, and never calls a domain
  mutation. Local loopback inference needs an explicitly entered provider,
  model and endpoint. Optional credentials come from an environment variable
  named in the UI with the `PIPELINE_ASSISTANT_*` prefix. No provider, model, endpoint or credential is stored by
  default. Remote paid requests fail closed because the generic API cannot
  verify a provider-enforced spending cap.
- Streaming SSE chunks, buffered JSON replies, cancellation, retry, observed
  token usage when supplied, loading, permission denial, malformed response,
  unavailable, and stale-context states are shown. Monetary cost is shown as
  unavailable, never estimated. Conversation turns, context, questions,
  answers, stream lines, request time and the event queue have explicit
  bounds. Incoming text is displayed as inert egui labels; it cannot apply a
  plan or approve an action.
- With no provider, the assistant presents deterministic observed status,
  next-action interpretation and the missing live-answer reason. The legacy
  shell remains the default; `PIPELINE_MODERN_SHELL=1` enables the new UI.
  Existing scheduler, approvals, audit, CLI and PTY code paths were untouched.

## Verification

| Check | Result | Evidence |
| --- | --- | --- |
| Format | Pass | [log](../evidence/ux04-fmt.log) |
| Clippy workspace/all targets, warnings denied | Pass | [log](../evidence/ux04-clippy.log) |
| Workspace tests | Pass: 87 passed, 3 existing ignored | [log](../evidence/ux04-test.log) |
| Production app build | Pass | [log](../evidence/ux04-build.log) |
| Native screenshot feature build | Pass | [log](../evidence/ux04-capture-build.log) |

Assistant tests use mocked provider behavior and local HTTP fixtures. They
check no configured provider, paid remote rejection, endpoint and credential
URL restrictions, tool-call denial, instruction/data separation, secret
patterns, project and draft isolation, cancellation, streamed usage,
permission denial, malformed data, unchanged activity on context reads and
revision fingerprint changes. No live model inference was performed.

## Windows native screenshots

All captures came from the running Rust/egui application using an isolated
sample portfolio created through `ProjectEngine`. They show an offline answer,
so no provider was contacted.

| View | Capture |
| --- | --- |
| 1440 × 900, dark, 100% | [project assistant](../evidence/ux04-project-1440-dark.png) |
| 1024 × 900, light, 100% | [narrow drawer](../evidence/ux04-project-1024-light.png) |
| 1024 × 900, light, 200% | [scaled drawer](../evidence/ux04-project-1024-light-200.png) |
| 1440 × 900, dark, 200% | [scaled dark](../evidence/ux04-project-1440-dark-200.png) |

The 200% drawer needs vertical scrolling for lower controls. Escape closes
the drawer; the compact composer remains outside it. Ctrl+K behavior has an
automated keyboard-event regression test, but a fresh end-to-end keyboard
traversal and a real screen-reader session were not run. AccessKit is enabled
in the app build, but spoken updates are unverified. Apple Silicon macOS
testing remains a later release gate.

## Risk review and rollback

Project data and provider responses are untrusted. The context uses an
allowlist of domain fields and known credential-pattern redaction, and the
transport rejects declared tool calls. Pattern redaction cannot identify
every possible secret; the selected local endpoint receives the bounded
context and is trusted to remain local. The app cannot inspect whether a
loopback server itself proxies to a paid backend. Remote paid inference is
closed until a provider-specific cap can be checked before requests. A
cancelled synchronous HTTP read may remain open until its overall timeout,
though the UI stops accepting its response immediately.

## PR #5 review refinements (13 October 2026)

Cancellation now closes the request's receiver immediately, and the transport
checks cancellation before emitting each event. Switching projects cancels an
in-flight request for the previous project. Each retry gets a fresh channel
and cancellation token, so late events have no path into a later turn. At most
two assistant HTTP workers can run at once; if both slots are occupied while
old requests finish, the UI asks the owner to retry shortly. A full event
queue cancels its worker. SSE lines are read with a 64 KiB bound, including
streams that never send a newline. The request has a 65-second overall HTTP
timeout and a 60-second streaming deadline checked between lines.

`ureq` 2.12 has no request-abort handle for an active synchronous read.
Its overall timeout takes precedence over the read timeout. UI delivery stops
promptly, but an abandoned worker can remain until the network call returns
or times out. The two-worker cap bounds thread and connection accumulation;
retry can temporarily report that earlier requests are finishing. A server
that stalls midway through a read can therefore delay reclaiming a slot.

Context assembly still uses only selected-project service views and a fixed
field list: project name/status, brief goal/audience and revision, plan
revision, active task titles/status, recent decision questions/status, and
the deterministic next action. Per-field limits and a 6,000-character total
bound apply. A revised sanitizer normalizes whitespace, removes common key
and credential forms, environment assignments/references, URLs, and path-like
text, and omits very long fields. This reduces accidental disclosure; no
pattern filter can guarantee discovery of an arbitrary secret embedded in
owner-supplied project text. The owner should review sensitive project text
before explicitly sending it to a local endpoint. The endpoint itself remains
trusted to stay local; the app cannot prove it does not proxy elsewhere.

Added regression tests cover a stalled streaming fixture, cancellation,
late responses, project switching, retry isolation, oversized SSE lines,
multiline secrets, unusual whitespace, credential formats, URLs, paths,
environment assignments, prompt injection, and cross-project context. The
standard Windows verification was rerun after the refinement:

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo fmt --all --check` | Pass | [log](../evidence/ux04-review-fmt.log) |
| `cargo clippy --workspace --all-targets -- -D warnings` | Pass | [log](../evidence/ux04-review-clippy.log) |
| `cargo test --workspace` | Pass | [log](../evidence/ux04-review-test.log) |
| `cargo build -p pipeline-app` | Pass | [log](../evidence/ux04-review-build.log) |

No suitable local OpenAI-compatible model endpoint was running. The common
loopback inference ports (1234, 11434, 8000, 8080, 5000, 5001, 3000) had no
listener, and the other loopback listeners belonged to unrelated services.
No model was downloaded or service started. Live model selection, grounded
answers, provider/model rendering, and live cancellation remain unverified;
local HTTP fixtures verify protocol handling and error states instead.

To return to the prior UI, unset `PIPELINE_MODERN_SHELL` or set it to a value
other than `1`. No migration or data rollback is needed. The owner should
review these captures and risk notes, test the local endpoint they intend to
use, and keep the PR unmerged until satisfied.
