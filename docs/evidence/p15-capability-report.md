# P15 Windows Pi capability report

Observed on 10 October 2026 with `@earendil-works/pi-coding-agent` CLI version `1.1.0` (`--version` returned `1.1.0`). The adapter rejects other versions before launch.

| Capability | Reported | Evidence and limit |
| --- | --- | --- |
| Probe | Ready for version 1.1.0 | Explicit executable/version check; unavailable and mismatch states reported separately. |
| Start | Supported | Owned RPC subprocess starts in isolated checkout; policy-extension ready handshake and session ID verified in no-inference process test and live runs. |
| Events | Supported | Strict LF JSONL decoder, Unicode/partial-read fixtures, response correlation, normalized progress/tool/permission/idle/error events. Live event logs are linked from `../pi-adapter-build.md`. |
| Steer | Supported | Prompt and explicit provider/model sent through RPC after one-use prompt approval. |
| Stop | Supported | Abort command and owned-process kill/reap tested; live wall/model limits stopped runs and retained checkouts. |
| Recovery | Limited | `get_state` reports active/idle. A same-session reopen passed a no-inference test. Automatic run continuation after application restart is **not** supported; `can_resume=false`. |
| Permission reply | Supported | Pi extension UI request pauses tool execution until exact project policy decision; no-inference deny/abort and live allow/deny observed. |
| Diff | Host capture only | Pi does not provide a native diff capability; app captures tracked checkout diff for review. `can_diff=false`. |

Windows `cargo test --workspace`, Clippy with warnings denied, and build logs are `p15-test.log`, `p15-clippy.log`, and `p15-build.log`. M1 Max task journey remains a pre-release check.
