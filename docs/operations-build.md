# P18 launch and operations build

11 October 2026, Windows x86_64.

Migration 012 adds revisioned lifecycle records for positioning, launch copy/assets, channel plans, metrics, feedback, bugs, incidents, and maintenance. Records can link an active-scope task and project artifact. The Launch & Ops tab creates and edits them. Every mutation has an activity event; the JSON export includes the records.

External publication uses a separate one-use owner approval bound to exact content hash, revision, and target. The app only records a manual publication after that approval and does not send anything externally. D-034 records this choice.

Portfolio health and the owner inbox include unresolved high or critical bugs/incidents, explicitly blocked operations, and failures on the latest release candidate. A passing or replacement candidate clears an earlier failed-check blocker.

## Verification

- [Walkthrough record](evidence/p18-walkthrough.md) and [sample export](evidence/p18-sample-export.json) cover idea, approved plan, verified task, manual release, launch work, metrics, feedback, bug, incident, and maintenance.
- [Service tests](evidence/p18-test.log) cover publication approval revision/target gates, launch asset linkage, operation blockers, failed-to-passing release checks, and replacement candidate behavior.
- [Clippy](evidence/p18-clippy.log) passes for store, engine, and app. Apple Silicon hands-on validation remains in P19.
- [Store suite](evidence/p18-store-test.log) passes 41 tests, and the [Windows app build](evidence/p18-build.log) passes.
