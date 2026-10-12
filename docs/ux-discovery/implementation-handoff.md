# UX implementation handoff proposal

This document recommends backlog refinements; `plan/ux-redesign-backlog.json` remains **proposed** and `plan/backlog.json` remains the approved implementation baseline. UX00/UX01 artifacts await owner design review. No implementation item has been accepted because of this prototype.

## Proposed phases and gates

| Phase | Scope | Acceptance and verification |
| --- | --- | --- |
| 0 · Design signoff | UX00 audit, UX01 visual/interaction review | Owner tests light/dark prototype, records feedback and approves or requests changes. Native egui feasibility spike covers 1024/1440 px, 100/200% zoom, keyboard and AccessKit. Attach screenshots/checklist. |
| 1 · Grounded shell | UX02 shell + UX03 deterministic recommendation engine | Project Home defaults correctly; one recommendation cites records, prerequisite, alternative and destination. Advanced retains existing terminal and diagnostics. Unit fixtures for every project/run state; native Windows keyboard/resizing/screen-reader smoke; no false progress. |
| 2 · Assistant | UX04 read-only context + UX05 typed proposals | Authorized project context, streaming/cancel/error/offline, provider/model/cost disclosure, redaction and prompt-injection tests. Before/after proposal uses expected revision; no write from plain answer; stale and unauthorized writes rejected by service tests. |
| 3 · Journeys and review | UX06 new/import/returning + UX07 active run/review/recovery | Novice reaches reviewable brief; import leaves source unchanged; review shows outcome/checks/risk and calls exact acceptance; interrupted work remains inspectable. Run journey, negative evidence, policy and recovery tests plus recordings. |
| 4 · Release quality | UX08 | Novice/expert studies, keyboard/contrast/scale/screen-reader matrix, Windows and Apple Silicon native smoke, existing regression and three-OS CI. Owner release signoff remains separate from UX design signoff. |

## Backlog refinements recommended for owner review

1. Split UX01 acceptance: prototype delivered versus owner visual approval. Record owner feedback as an open gate, not a task completion implied by a committed artifact.
2. Make the native egui layout/accessibility spike a prerequisite of UX02. HTML screens alone cannot prove AccessKit or responsive native behavior.
3. Add UX03 as a dependency for UX02’s “grounded next action,” or define a temporary deterministic recommendation fixture for shell verification. The current dependencies let UX02 promise a grounded action before its source exists.
4. Split UX05 into structured proposal contract/service validation and UI review. Require revision conflicts, policy denial, audit and exact-action negative tests before the interface offers Apply.
5. Add explicit run-state and evidence-view-model work before UX07. Existing data spans Plan and Runs tabs; consolidation requires a service-level summary with links to immutable evidence.
6. Add acceptance for offline/manual guidance and “configured but unreachable” provider distinction to UX04 and UX06. Do not show generative claims when no provider is available.
7. Reconcile P16–P19 dependencies and release gates in a new owner-approved scope revision; UX screens for Design/Releases do not substitute for their domain workflows.

## Evidence and safety inheritance

All production mutations go through `ProjectEngine`/domain service, with optimistic revisions, activity records and existing P11–P15 policy checks. Agent-generated plans remain drafts until approved. Submission remains separate from owner acceptance and verified completion. The scheduler may auto-continue only approved, ready, unblocked work with enforceable limits and one active code-changing run per project. External publication, production changes, spending, credentials, destructive effects and material scope changes retain exact scoped approval. The UI must report unavailable capabilities, observed checks and uncertainty honestly.

## Prototype limitations and review checklist

The prototype is static HTML/CSS/JS with sample Northstar and Fieldnotes data. It does not connect to SQLite, a provider, Git, native egui, the scheduler or a policy service. Navigation concepts for idea intake, Design, Releases, History and Settings are intentionally shallow; Accept only toggles sample display. Narrow-width overlay and keyboard behavior are illustrative; native and assistive-technology validation remains open.

Owner review: inspect the portfolio focus, project overview, expanded assistant proposal, work review, active run, blocked and offline states. Check whether the reason/next action is understandable, whether a proposed scope change clearly waits for approval, whether advanced details remain findable, and whether both themes feel appropriate. Record corrections before authorizing production UX work.
