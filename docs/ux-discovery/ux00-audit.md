# UX00 · Current experience audit

12 October 2026 · Baseline: `6391201` on `main` · Discovery, not implementation acceptance.

## Method and evidence

I inspected the Rust UI, its domain-service reads/actions, the approved UX direction, and existing Windows screenshots. The screenshots below were captured in earlier foundation work and are **historical baseline evidence**, not a fresh 12 October live-session recording. The current UI remains substantially the same egui shell in `crates/pipeline-app/src/main.rs`; current source references below establish the presently implemented behavior. No novice user test or stopwatch study was performed, so baseline task times and success rates are **unmeasured**. The first native UX implementation slice should capture them with a scripted study.

| Existing screen | Evidence | Current code |
| --- | --- | --- |
| Portfolio and overview | [portfolio-windows.png](../evidence/portfolio-windows.png) | `main.rs:332–411`, `2152–2204` |
| Three-region shell | [three-pane-windows.png](../evidence/three-pane-windows.png) | `main.rs:2438–2492` |
| Brief revision | [brief-revision-windows.png](../evidence/brief-revision-windows.png) | `main.rs:1003–1124` |
| Plan review | [plan-review-windows.png](../evidence/plan-review-windows.png) | `main.rs:1201–1709` |
| Decision approval | [p10-approval-windows.png](../evidence/p10-approval-windows.png) | `main.rs:1711–1934` |
| Agent run review | [p14 artifact evidence](../task-review-build.md) | `main.rs:1971–2150` |

## Screen and navigation inventory

The shell has a resizable project list at left, eight management tabs (Overview, Brief, Research, Plan, Decisions, Activity, Runs, About), and a resizable terminal below. F1/F2 toggle the portfolio/terminal; text zoom and dark mode are in a utility toolbar (`main.rs:48–58`, `2438–2492`). The project overview contains folder, Git root, stage, health, completion, blocker count, and a next-owner-action button. That button always opens **Decisions**, even if the next action is brief or plan review (`main.rs:2174–2200`). The portfolio inbox does route by item kind (`main.rs:339–357`).

The brief screen is a long form with 15 fields and explicit save-then-approve revision controls (`main.rs:1019–1089`). Plan editing presents epics, tasks, weights, dependency edges, revisions and criteria in one scrollable view (`main.rs:1220–1705`). Run setup exposes executable paths, provider/model IDs and observed token limits before any plain-language explanation of the task (`main.rs:1987–2047`). Run review displays checkout paths, run IDs, packet hashes, JSON, event sequence numbers, test IDs and artifact hashes (`main.rs:2053–2138`). Task acceptance and criterion verification are in Plan, while diff and submission are in Runs (`main.rs:1615–1702`, `2076–2137`).

## Main friction and opportunity

| Finding | Evidence in code | Consequence / design response |
| --- | --- | --- |
| No portfolio-level focus narrative | `portfolio()` renders inbox, raw create/import controls, and project buttons (`332–411`) | Lead with one explainable priority and project cards; keep exact data available. |
| New project starts with a folder path | `main.rs:360–372` | Begin with intent and essential questions; show folder choice during preparation. Import inspection stays read-only until authorized. |
| Next action routing is inconsistent | Overview button always selects Decisions (`2193–2199`) | Use typed destination and prerequisite, not a label-only next action. |
| Workflow requires domain vocabulary | “revision,” “active approved scope,” “epic,” “weight,” “criterion,” “command digest” (`1025`, `1227`, `1567`, `1744–1753`) | Show user outcomes first; expose exact records in Advanced. |
| Review crosses tabs | Agent submission in Runs and acceptance in Plan (`2086–2137`, `1641–1689`) | One review page with outcome, checks, risk, diff and exact acceptance gate. |
| Run setup precedes useful guidance | `main.rs:1987–2028` | Recommend eligible work, then show provider/model, limits and permission implications at start. |
| Errors are mostly inline text | `main.rs:1980–1985`, `2048–2050` | Give blocked, interrupted and offline states a recovery action and last verified event. |
| Visual density and weak hierarchy | Historical screenshots plus long `ui.group`/`ui.label` sequences | Design reusable cards, spacing, typography and concise summaries; retain expandable records. |
| No persistent assistant | `DesktopApp` fields and shell (`75–127`, `2438–2492`) | Add global composer and contextual right panel after design approval. |

## Journey maps: current → intended

| Journey | Current route and friction | Prototype / implementation target |
| --- | --- | --- |
| First idea | Enter folder/name → Brief tab → fill required fields → save revision → approve (`299–330`, `1003–1123`). No conversational intake. | Describe idea → essential questions → labeled assumptions → brief preview → exact-revision approval. |
| Import | Enter folder path/name → import (`360–372`); imported project appears in list. | Read-only inspection summary → unknowns → confirm intended project outcome → propose alignment. |
| Return | Pick project → raw status overview → navigate to relevant tab. Overview’s next-action button can misroute. | Portfolio focus → project snapshot → one linked action, reason, alternatives and recent outcome. |
| Blocked | Blocker count in overview; task blockers in Plan; policy requests in Decisions (`2192`, `1628–1633`, `1739–1760`). | Explain what is blocked, preserved work, unrelated eligible work and exact decision. |
| Work review | Runs for submission/diff/tests → Plan for criterion evidence and accept/request changes. | One outcome review with checks and risk; raw proof opens on demand; service still enforces acceptance. |
| Release | Baseline plan calls for release workspace, but `Tab` has no Release tab (`48–58`). P17 remains unstarted in `plan/progress.md`. | Show guided readiness concept only; implementing the lifecycle remains P17/approved sequencing work. |

## Accessibility and measurement risks

- The app enables egui `accesskit` in `Cargo.toml:21`, and built-in controls have keyboard focus. F1/F2 and zoom 0.8–2.0 exist (`main.rs:2447–2469`). Native screen-reader, 200% scale, high-DPI, focus-order and M1 Max checks remain open in `docs/spike-results.md`; no compliance claim follows from `accesskit` alone.
- The custom terminal paints text directly and handles focus/input itself (`main.rs:911–1001`). It needs a separate accessibility path in Advanced rather than assuming the normal panel covers it.
- The existing 10,000-row spike used `ScrollArea::show_rows` (`spikes/desktop/src/main.rs:62`), but the production portfolio uses an ordinary loop (`main.rs:380–405`); measure large portfolios.
- Initial usability study: ask a novice and an experienced owner to create an idea, find the next action, review a submitted task, resolve a blocker and find release readiness. Record completion, time, wrong turns, jargon questions, keyboard path and 200% scale. Do not treat this proposal’s target metrics as observed results.

## Terminology for ordinary mode

| Current term | User-facing label | Advanced detail retained |
| --- | --- | --- |
| Brief revision | Product direction / draft version | Revision, content hash, history |
| Active approved scope | Approved work | Scope revision and weighted denominator |
| Epic / task graph | Milestone / work item | Epic IDs, dependency edges |
| Criterion / verifier | Outcome check | Criterion record and evidence references |
| Agent run / worktree | Current work / saved work area | Run ID, checkout path, events |
| Policy request / command digest | Permission request / exact action | Class, target, digest, expiry |
| Harness / provider / model | AI setup | Adapter capabilities and identifiers |
