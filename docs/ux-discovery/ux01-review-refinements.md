# UX01 review refinements · 12 October 2026

The owner approved the overall Modern Product Studio direction for **continued design refinement** and requested corrections to project switching, assistant continuity, and pause semantics. This record describes the isolated prototype; it is not approval to replace the native UI or change the implementation sequence.

## Project context

Northstar and Fieldnotes now have distinct illustrative goals, stage journeys, verified progress, work, recommendations, review examples and proposal revisions. Selecting a project changes the breadcrumb, project tabs, assistant context chip and compact composer context. Returning to Home reads each project's current illustrative state. A project selection does not reset the other project's run state or conversation. Global Home, Inbox, Projects, Activity, Settings and new-idea views use a separate **All projects** conversation context.

## Assistant continuity and proposal binding

The prototype keeps an in-memory conversation and one unsent draft per context: All projects, Northstar, and Fieldnotes. The compact prompt bar and expanded panel edit the same draft for the active context. Sending from either adds a user turn and a clearly labeled simulated response. Opening/closing the panel, moving between project sections, and switching projects preserve messages and drafts while the page remains open. The context chip names the current project and screen.

A proposed change records its originating project ID, screen, target plan and expected illustrative revision. Its card appears only in that project's conversation; the review action checks the bound project again before opening the roadmap. In the All projects context, a change request asks the user to choose a project. No card applies a real change. Reloading resets this disposable scenario; production conversation retention/export policy remains a later design and implementation decision under UX04.

## Automation state model

| Visible state | Meaning | Demonstrated action |
| --- | --- | --- |
| Ready | Approved, unblocked work is eligible, subject to fresh preflight | Start approved task → Working |
| Working | Approved task actively running within current limits | Pause after task → Pause requested; Pause now → Paused; finish → Awaiting review |
| Pause requested | Current task continues; no new task starts afterward | Cancel request → Working; finish → Paused with review pending |
| Paused | Automatic continuation stopped; evidence preserved | Resume → Ready after stated recheck; pending review remains available |
| Blocked | Unresolved decision/dependency prevents affected work | Open decision; no auto-resume implied |
| Awaiting review | Agent submitted work but owner has not accepted it | Review; illustrative owner accept changes accepted progress |
| Offline | Configured provider is unavailable | Saved plan/review and deterministic guidance remain available |

Pause transitions do not mark a task accepted, rejected or blocked. Finishing while pause is requested leaves a review item pending and automation paused. An owner acceptance remains separate; automatic continuation would recheck approvals, dependencies, permissions, provider/model and limits. External/high-risk actions still require scoped approval. The prototype does not run a scheduler or provider.

## Verification and screenshots

Run `npm ci` and `npm test` in `prototype/`. The [refinement smoke log](../evidence/ux01-refinement-smoke.log) covers both projects, separate histories, multiple turns, shared and preserved drafts, cross-project proposal guard, pause/cancel/finish/resume, all seven states, keyboard focus, narrow drawer, both themes and browser errors. `npm run screenshots` captured [updated and new screenshots](../../prototype/screenshots/) at 1440×900, including Fieldnotes context/conversation/proposal and pause requested/paused states. These are browser screenshots of sample data, not native egui evidence.

## Remaining limits

The prototype has no persistence across reloads, provider connection, database, real approvals, run limits or scheduler. Design, Releases and Settings remain conceptual. Native egui layout, AccessKit, 200% scale, screen-reader behavior and M1 Max validation remain open. UX02–UX08 and P16–P19 remain unchanged.
