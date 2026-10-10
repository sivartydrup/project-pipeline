# P13 OpenCode adapter implementation and verification

9 October 2026, Windows x86_64. The installed OpenCode is 1.18.30. The adapter is pinned to this tested protocol; other versions report `VersionMismatch` with capabilities disabled. A failed health request reports `Unavailable` rather than implying a running session. The application can launch a password-protected loopback server in the managed checkout, or attach to an existing loopback server for recovery.

Routes and request shapes were checked against the installed server's `/doc` schema and the [OpenCode server documentation](https://dev.opencode.ai/docs/server/). The installed schema includes both legacy and v2 permission routes.

## Adapter contract

`pipeline-adapters` implements probe, session-only start, async prompt/steering, SSE event stream, abort, diff/status recovery, pending permission lookup, and single-use or reject permission replies. HTTP connections require an explicit loopback address. The server helper passes its generated password through the child environment, keeps it out of command arguments and logs, hides the process window on Windows, and reaps the child on drop. Windows launch requires the native `opencode.exe`: the npm `.cmd` shim can leave a child server behind when stopped.

The engine attaches the external session ID before sending a task prompt. Normalized events carry a run/session link and a bounded summary. The store writes event receipts and run events in one transaction; a repeated external event ID does not add another event or replay policy handling. P12 startup reconciliation still interrupts abandoned runs and retains their worktrees. `recover_opencode_run` probes the external session, status, and diff without replaying a prompt.

OpenCode `permission.asked` and `permission.v2.asked` events become `unknown_external` policy requests with an exact permission ID, target, and command digest. The adapter never sends `always`. A pending request receives no harness reply. A denied request sends `reject`; an approved request consumes the exact P12 approval before sending `once`. The approval remains consumed if the network fails after consumption, preventing an automatic retry of an external effect. The owner inbox from P12 displays these requests. P14 will connect the run controls and permission continuation to the Runs/Review workspace.

## Evidence

- [Protocol and workspace test log](evidence/p13-test.log): fixture covers probe, start, steer, stop, recovery, both permission event formats, session filtering, outage, and event deduplication; schema v7→v8 upgrade creates a pre-upgrade backup.
- [Live Windows run log and capability report](evidence/p13-live-windows.log): installed OpenCode 1.18.30 passed session create, status/diff recovery, abort, deletion, SSE session event, and owned password-protected server lifecycle. These checks did not start model inference.
- [Clippy log](evidence/p13-clippy.log) and [desktop build log](evidence/p13-app-build.log) cover the full workspace and native app.
- [Approved live model smoke](evidence/p13-model-smoke.log): one prompt to the $0 OpenRouter North Mini Code endpoint in a disposable checkout generated live progress, diff, and permission events. The app raised a scoped policy request, recorded denial, and sent OpenCode `reject`; no tool command was approved. The [scoped approval record](approval-p13-live-smoke-2026-10-10.md) limits this test to one prompt and no paid fallback.

## Remaining verification

The Windows model-driven permission path passed. The MacBook Pro M1 Max session smoke remains a release check when that machine is available. P14 will connect these services to owner run controls and evidence review in the desktop UI.
