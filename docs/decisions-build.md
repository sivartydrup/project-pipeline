# P10 decisions, inbox, and history build

9 October 2026, Windows x86_64. Accepted by the owner; see [P10 acceptance](approval-p10-2026-10-09.md).

## Implemented

- Schema v5 adds decision evidence, recommendation, update time, and indexes. Existing v4 files upgrade with a pre-upgrade backup.
- The store service proposes, edits, approves, rejects, and supersedes decisions transactionally with activity events. Edits and owner resolutions require the current decision revision. Approval records bind the decision, project, exact revision, selected option, and content digest.
- The owner inbox derives pending decisions, draft brief/plan revisions, and blocked or review-stage active tasks from their source records. Blocking and high-impact decisions sort first. The portfolio shows a global inbox, next owner action, and health derived from the current blockers and requests.
- The desktop Decisions view shows alternatives, recommendation, rationale, evidence, impact, original actor, status, revision, and supersession. The Activity view shows audit entries and links back to their workspaces. Active task cards link to their decisions.
- Plan revisions now include validated milestone task groups. Approved milestone summaries appear in the Plan view and link their task decisions to the same register.
- D-015 and D-016 record the derived-inbox, immutable-resolution, and milestone-grouping choices.

## Verification

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo build -p pipeline-app` passed on Windows. Log: [p10-test.log](evidence/p10-test.log).
- Store tests exercise proposal editing, exact-revision conflict, owner-only approval, invalid option rejection, approval hashing, successful rejection, supersession, priority order, project isolation, audit rollback, and v4 migration.
- The built Windows app launched against an isolated sample database created through the store service by `p10_ui_fixture`. A blocking proposal appeared in the global inbox, opened the Decisions view, and showed its options and evidence. A sample approval removed the inbox item and displayed the selected option at revision 2; the Activity view showed the approval and proposal events and linked back to the register. Screenshots: [Decisions](evidence/p10-decisions-windows.png), [approval result](evidence/p10-approval-windows.png), [Activity](evidence/p10-activity-windows.png).
- A second isolated fixture included an approved plan and task-linked milestone. The Windows Plan view displayed its milestone and decision link; selecting the link opened the same decision in the register. Screenshot: [milestone](evidence/p10-milestone-windows.png).

## Review limits

- The UI smoke used isolated fixtures under `target/p10-ui-smoke` and `target/p10-ui-milestone` and did not add test decisions to owner data. Interactive editing and rejection were verified at the service layer, but have not been hands-on tested in the UI.
- Apple Silicon macOS hands-on validation remains a release check under P19.
