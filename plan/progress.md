# Implementation progress

Updated 8 October 2026. Task acceptance follows `plan/backlog.json`; an implementation note or passing compile does not by itself accept a task.

| Task | State | Evidence / remaining work |
| --- | --- | --- |
| P00 | Accepted | Owner approval and Windows environment profile in `docs/approval-2026-10-08.md`. |
| P01 | Accepted for foundation | `spikes/desktop` compiles and launches with egui 0.36.2; three-region screenshot and accessibility risk assessment in `docs/spike-results.md`. M1 Max and screen-reader release checks remain. |
| P02 | Accepted for foundation | `spikes/pty` passes ANSI/Unicode, scrollback, and Windows cmd/PowerShell PTY lifecycle checks. UI rendering moves to P07. |
| P03 | Accepted for foundation | OpenCode 1.18.30 lifecycle, SSE, empty diff, restart recovery and Pi 1.1.0 RPC state/new-session/abort probes passed. Live permission/tool mapping moves to P13/P15. |
| P04 | Accepted for foundation | Eight-crate Rust workspace, local Git repository, three-OS CI workflow, Windows format/lint/test/build and native app launch passed. Evidence: `docs/foundation-build.md`. CI execution moves to P19 under D-011. |
| P05 | Accepted for foundation | SQLite schema, transactional project changes, append-only audit, versioned migration and pre-upgrade backup path, restore/export, eight integrity tests, and full Windows workspace checks passed. Evidence: `docs/storage.md` and `docs/foundation-build.md`. |
| P06 | Accepted for foundation | Folder create/import, Git identity, persisted portfolio and scope metrics, two-repository persistence test, Windows UI visual review and screenshot passed. Three-OS CI passed. M1 Max hands-on import remains a release check. Evidence: `docs/portfolio-build.md`. |
| P07 | In verification | Three-pane shell, project-scoped PTY tabs, drag-resizable and persisted layout implemented. Windows shell, resize, exit, ANSI/Unicode parser, bounded large-output, and process-close tests pass; UI layout screenshot and passing three-OS CI captured. Hands-on terminal input in the UI and M1 Max smoke remain. Evidence: `docs/terminal-build.md`. |
| P08–P19 | Not started | Dependencies and gates in `plan/backlog.json`. |

The MacBook Pro M1 Max is not yet available. Native macOS checks remain release requirements and do not block Windows foundation work.
