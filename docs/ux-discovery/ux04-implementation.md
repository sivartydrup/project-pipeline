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
cancelled synchronous HTTP read may remain open until its read timeout,
though the UI stops accepting its response immediately.

To return to the prior UI, unset `PIPELINE_MODERN_SHELL` or set it to a value
other than `1`. No migration or data rollback is needed. The owner should
review these captures and risk notes, test the local endpoint they intend to
use, and keep the PR unmerged until satisfied.
