# Decision register

Status values: **confirmed requirement**, **proposed**, **approved**, **rejected**, **superseded**. A confirmed requirement records an explicit owner preference; it does not approve the entire specification. A proposed decision is an implementation assumption, not owner approval.

| ID | Status | Proposal | Why / consequence |
| --- | --- | --- | --- |
| D-001 | confirmed requirement | Build a native, compiled Rust desktop application, including the interface. | Explicit owner choice. |
| D-002 | confirmed requirement | Keep project data local-first. | Explicit owner choice; sync can follow later. |
| D-003 | confirmed requirement | Prioritize Windows and Apple Silicon macOS (M1 Max); retain Linux portability and CI, with Linux release support after the primary targets pass. | Matches the owner's current Windows PC and incoming MacBook Pro. |
| D-004 | approved | Use an adapter interface, with OpenCode first and Pi second. | Both have process integration surfaces; the app's task model remains harness-neutral. |
| D-005 | confirmed requirement | Use approval gates for risky agent actions. | Explicit owner choice; proposed policy details are in the agent contract. |
| D-006 | approved | One owner and one active local agent run per project in v1. | Simplifies conflict handling and approval ownership; concurrent runs follow later. |
| D-007 | approved | Completion is based on required evidence and accepted outcomes, not an agent's estimate or time spent. | Makes portfolio status auditable. |
| D-008 | approved | Agent runs operate in a project checkout or isolated Git worktree; default to worktree for code-changing tasks. | Gives clear diffs and a recoverable boundary. |
| D-009 | approved | Use `eframe`/`egui` for the Rust UI. | Windows layout spike passed; retain accessibility and M1 Max release checks in P19. |
| D-010 | approved | Use SQLite for workflow metadata and keep source code in normal project folders. | Supports local-first use and offline access. |
| D-011 | approved implementation sequencing | Configure three-OS CI in P04, but require actual Windows/macOS/Linux CI execution in P19 once this project has its own remote. | The current folder was nested inside an unrelated Git repository; this preserves the cross-platform release gate without blocking local implementation. |
| D-012 | approved implementation choice | Use `portable-pty` with `vt100` parsing and a bounded reader queue for the first terminal integration. | The Windows PTY spike validated these libraries. UI frames process a limited output budget; a richer terminal renderer can build on the same session boundary. |
| D-013 | approved implementation choice | Store briefs as immutable structured revisions; bind owner approval to a revision and SHA-256 content hash. | New edits stay pending while the previously approved revision remains identifiable. Field-level deltas and transactional activity events make the review trail explicit. |
| D-014 | approved implementation choice | Store each task plan as an immutable structured revision and materialize tasks into a new scope only on owner approval of the plan, after the latest brief is approved. | Draft edits leave active completion unchanged. An accepted task carries into the next scope only when its definition and blocking prerequisites are unchanged; changed work must be reviewed again. |

## Baseline approval

The owner approved the baseline on 8 October 2026 (Australia/Sydney). The exact file hashes and environment profile are recorded in [approval-2026-10-08.md](approval-2026-10-08.md). OpenCode is the first adapter by the approved plan; Pi remains second. No deployment provider or design-tool constraint has been specified.

## Decision record format for the future app

Each decision has an immutable ID, project/task/run links, question, alternatives, selected option, rationale, evidence links, risk, author (human or agent), timestamp, status, and supersedes link. A new record supersedes a prior decision; history is never overwritten. Approval captures approver, scope, exact revision/hash, and time.
