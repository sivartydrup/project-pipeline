# UX02/UX03 first production slice · owner review

12 October 2026 · branch `codex/ux02-ux03-production` · authorized by [UX02/UX03 handoff](NEXT-CODEX-UX02-UX03.md). This is an implementation for review, not task acceptance or release approval.

## Scope and design decisions

- `PIPELINE_MODERN_SHELL=1` enables the new shell. Omitting it retains the current production interface. The rollout is reversible and has no database migration.
- The shell shows portfolio attention, project switching, a project snapshot, one typed next action, a compact draft composer and a right guidance panel or narrow drawer. The panel states plainly that no conversational provider is connected; draft text is neither sent nor applied. Advanced embeds the existing portfolio editor, all eight management tabs, run evidence and PTY terminal.
- `ProjectEngine::next_action` reads persisted overview, brief, plan, inbox and run review records. `recommend_next` is a pure priority function. Every action has a title, reason, prerequisite, alternative, destination and source IDs. Action buttons navigate to existing screens; no new write path was added.
- The UI refreshes the read-only recommendation at most once every three seconds while a project is open and immediately after project switching, avoiding service queries on every egui frame.
- The priority is inconsistent records → exact policy request → task review → unmatched submitted run → decision → interrupted/failed run → blocked task → active run → brief/plan review → approved ready work → blocked/complete/fallback. A ready task remains subject to fresh scheduler preflight. Missing packet verification, failed/unknown checks and missing run evidence are called out in review guidance.
- No provider probe is performed by this slice. The offline recommendation branch exists and is fixture tested, but the production service does not claim an offline provider state without an observed failed check. No new run, approval, or scope mutation is triggered by guidance.

## Evidence

The Windows renderer captured the running production app using the optional `screenshot-evidence` build feature and isolated sample data created through `ProjectEngine::create_new`. The sample project has no brief or approved scope; its 0% value is not presented as verified completion in the new overview.

| View | Native screenshot |
| --- | --- |
| Empty portfolio, 1440 × 900, light | [Home](../evidence/ux02-ux03-home-1440-light.png) |
| Project and guidance side panel, 1440 × 900, dark, 100% | [Project](../evidence/ux02-ux03-project-1440-dark.png) |
| Project, 1024 × 900, light, 200% | [Scaled project](../evidence/ux02-ux03-project-1024-zoom200.png) |
| Project and assistant drawer, 1024 × 900, light, 100% | [Narrow drawer](../evidence/ux02-ux03-drawer-1024-light.png) |
| Advanced legacy views and terminal, 1024 × 900, light | [Advanced](../evidence/ux02-ux03-advanced-1024.png) |

Verification: [format](../evidence/ux02-ux03-fmt.log), [workspace tests](../evidence/ux02-ux03-test.log), [Clippy](../evidence/ux02-ux03-clippy.log), [app build](../evidence/ux02-ux03-build.log), and [screenshot-feature test](../evidence/ux02-ux03-capture-test.log). The recommendation fixtures cover empty/new, draft review, approved ready, active, blocked, review with missing packet verification, interrupted, failed, offline, complete and contradictory approval/progress records. Existing scheduler, evidence and policy regression tests ran in the workspace suite.

## PR #4 review refinements

- The assistant keeps unsent text in memory under each project ID, with a separate portfolio draft. The compact composer and expanded Guidance editor edit the same active draft. Switching contexts and opening or closing Guidance preserve text without exposing another project's draft.
- `ProjectEngine::next_action` now has persisted-project tests for creation, brief/plan review and approval, ready and blocked tasks, decisions, active/interrupted/failed runs, submissions with missing or failing checks, and accepted scope. They assert title, reason, prerequisite, source IDs, destination and unchanged activity count. A stale approval is rejected through the domain service; impossible contradictory records remain synthetic negative fixtures.
- Interrupted or failed run recovery takes priority over a generic blocked-task inbox entry. This read-only ordering change is recorded in [UX-D07](decision-records.md).
- Ctrl+K now reads the modifier state on the key event, fixing a native focus failure. Escape returns focus to the Guidance toggle. Dark brand and theme-specific hover colors were adjusted after a basic contrast check.

Fresh verification: [workspace tests](../evidence/ux02-ux03-review-test.log), [format](../evidence/ux02-ux03-review-fmt.log), [Clippy](../evidence/ux02-ux03-review-clippy.log), [production build](../evidence/ux02-ux03-review-build.log), [screenshot feature build](../evidence/ux02-ux03-review-capture-build.log), and the [Windows native smoke record](../evidence/ux02-ux03-review-native-smoke.md). New captures show the [dark narrow drawer](../evidence/ux02-ux03-review-narrow-100-dark.png) and [light 200% drawer](../evidence/ux02-ux03-review-narrow-200-light.png).

## Remaining verification and limitations

- The Windows desktop was unlocked for the review-refinement smoke. Sampled keyboard traversal, Escape focus return, Ctrl+K focus and AccessKit controls passed. A real screen reader was not run, so spoken announcements and dynamic drawer behavior remain unverified.
- Selected text/background token ratios were calculated, but a full contrast audit remains open. Long translated strings, narrower than 1024 physical pixels, OS scaling other than the current 125%, and M1 Max remain unverified. At 1024 pixels with 200% egui zoom, content below the recommendation requires scrolling.
- The new shell retains the existing forms in Advanced. Guided idea intake, conversational responses, proposal apply, integrated review and autonomous behavior are UX04–UX08 or P16–P19 work and were not implemented here.
- The `screenshot-evidence` feature is opt-in and used only for local capture. The sample database and folder are under ignored `target/`; screenshot images contain sample data only.

## Rollback and owner review

Unset `PIPELINE_MODERN_SHELL` or set it to any value other than `1` to return to the prior shell. No data rollback is needed. Owner review should inspect the original and refinement captures, try the feature-flagged build with an actual review/blocked/active-run project, and confirm that Advanced still exposes evidence and exact actions. Keep the PR unmerged until that review is complete. Repeat the native journey on the M1 Max before release.
