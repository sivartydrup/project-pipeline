# UX01 · Interaction design and egui feasibility

12 October 2026 · Design proposal for owner review. The [clickable prototype](../../prototype/README.md) uses illustrative data and performs no domain writes.

## Screen model

Global nav: Home, Projects, Inbox, Activity, Settings. A project opens on Overview, then Roadmap, Work, Design, Releases and History. Advanced exposes terminal, exact IDs, logs, diff, diagnostics and CLI/API. The compact composer stays visible across primary screens. An assistant panel opens at right on desktop, preserves page context and draft, and becomes an overlay drawer when the center would fall below a readable width. Escape closes it and returns focus to its toggle; Ctrl/Cmd+K focuses the composer.

| State / screen | Primary answer | Key transition |
| --- | --- | --- |
| Portfolio | What needs me across projects? | Focus card → review or decision; project card → overview. |
| Project overview | What next, why, and what is the agent doing? | One recommendation → destination; alternatives → roadmap. |
| Assistant expanded | What is observed, recommended, or unknown? | Read-only answer → typed draft proposal → before/after preview → existing service gate. |
| Active work | Is work genuinely running, and under which limits? | Inspect status/last verified event; pause or stop with explicit semantics. |
| Review | What changed, did it pass, and what risk remains? | Accept or request changes; raw evidence available. |
| Blocked / offline | What is safe and what can I do? | Exact decision, preserved work, deterministic guidance and manual fallback. |

The prototype gives the four priority screens full layouts. Roadmap and shell navigation have representative states. New idea, Design, Releases, History and Settings are navigation concepts, not completed S04–S10 designs. Their production interactions require later UX work and P16–P18 lifecycle records.

## Recommendation and proposal contract

Each recommendation carries a title, source record IDs, reason, prerequisite, intended outcome, alternative and destination. The engine must derive it from `ProjectOverview`, `PlanView`, `ReviewView`, task/run state and policy state, with a deterministic priority table and stale-data state. Assistant language may explain it, but cannot create authority. A useful priority order to test: exact action request → submitted review → blocking decision → interrupted recovery → approved ready work → brief/plan review → create/import guidance. Competing items need explicit tie-breaking and project context.

Assistant turns distinguish **observed** facts, **recommendation**, and **unknowns**. For “add subscriptions,” a typed proposal includes target object, expected revision, before/after, risk, new approvals, effect summary and discard/recovery path. It remains a draft. Save/apply calls the existing domain service; stale revision causes a fresh preview. Approved scope never changes from a chat sentence. Agent work continues automatically only when an approved ready task, dependencies, provider/model authorization, enforceable limits, scoped permissions and one-run invariant all pass. Submission waits for human acceptance; a different eligible approved task may run only after the coordinator reevaluates policy.

## Visual system, prototype tokens

Warm neutral surfaces, restrained teal accent, 8 px rhythm with 4 px increments, 8–15 px radii, 11–32 px typography hierarchy, soft elevation and explicit semantic labels. Light and dark use paired CSS tokens in `prototype/styles.css`. Cards have one main action; secondary details use disclosure. Status includes text and shape, never color alone. Motion is minimal and disabled through `prefers-reduced-motion`. These are design candidates, not final native token values; contrast and platform font rendering require measurement on native builds.

Components to implement after approval: `AppShell`, `GlobalNav`, `ProjectSwitcher`, `AssistantComposer`, `AssistantDrawer`, `NextActionCard`, `JourneyStepper`, `AgentRunCard`, `ProposalCard`, `ApprovalGate`, `EvidenceSummary`, `RecoveryCard`, and `AdvancedPanel`. Each needs loading, empty, long-text, error, disabled, focus and narrow-window behavior. Ordinary language must still link to exact immutable records.

## Rust/egui feasibility

**Current baseline:** `eframe`/`egui` 0.36.2 with `accesskit` (`Cargo.toml:21`), a single `DesktopApp` and panel composition (`crates/pipeline-app/src/main.rs:75–127`, `2438–2492`). Layout persistence already stores portfolio width, terminal fraction, text scale and dark mode (`main.rs:16–37`); the left and bottom panels resize. The UI reads/writes through `ProjectEngine`, which exposes overview, brief, plan, review, run evidence and exact approval operations (`crates/pipeline-engine/src/lib.rs:207–268`, `288–465`). This supports a native redesign without changing the approved toolkit.

**Proposed implementation within egui:** compose top nav, optional left rail and bottom composer before the central content; add a right `SidePanel` with a bounded width and a separate overlay/window for narrow widths. Use stable egui IDs and store expanded state, draft text and focused object. Build component functions around domain view models rather than copying data into a parallel UI store. Use `Visuals`/style and registered font families for visual tokens, and `ctx.set_zoom_factor` for scaling. Virtualize long project/task/event lists (`ScrollArea::show_rows`), keep bounded event polling, and debounce expensive recomputation. Use `egui::Key`/modifiers for composer and panel shortcuts, explicit focus request/return, labels and `accesskit` semantics. Keep terminal and raw evidence in Advanced without removing access.

**Risks to validate with a small native spike before broad replacement:** panel squeeze at 1024 px and 200% zoom; text wrapping and minimum widths; focus transfer with drawers and long proposals; AccessKit announcements and screen-reader reading order for dynamic chat; contrast in native rendering; high-DPI and M1 Max font metrics; custom painted terminal accessibility; large-list performance. The HTML prototype proves a layout direction, not native egui feasibility. No alternative framework is proposed now. If the native spike cannot meet minimum keyboard and screen-reader journeys, document measured failures and trade-offs for a separate owner decision under D-009.

## Screenshot set

The [prototype screenshots](../../prototype/screenshots/) show portfolio light, project overview, assistant proposal, work review, active run dark, blocked decision and provider offline. They were captured with Edge headless at 1440×900. They are visual snapshots of illustrative data, not native Windows or macOS app screenshots.

## Owner design decisions to record

1. Approve the information architecture and terminology above, including Advanced placement, for implementation.
2. Approve the visual direction after reviewing both themes and proposal/review flow; request changes if needed.
3. Confirm the Guided Auto default and its precise preflight, continuation and pause behavior before UX implementation. Existing safety gates remain authoritative.
4. Authorize UX02–UX08 sequencing alongside or ahead of P16–P19. This prototype does not authorize that implementation or release.
