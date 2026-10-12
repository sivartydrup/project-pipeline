# UX02/UX03 first production implementation — owner authorization

**Status:** Authorized for a scoped first production implementation phase on 2026-10-12, following approval and merge of native feasibility PR #3 (merge commit `64508fa`). This authorization covers **UX02 native shell and UX03 deterministic recommendation foundation together**, not UX04–UX08 or reordering/closing P16–P19.

## Required context
Read `AGENTS.md`, `README.md`, `docs/product-spec.md`, `docs/architecture.md`, `docs/decisions.md`, `docs/agent-contract.md`, `docs/proposals/ai-guided-experience.md`, `docs/proposals/modern-product-studio-screens.md`, `docs/ux-discovery/ux00-audit.md`, `docs/ux-discovery/ux01-design.md`, `docs/ux-discovery/ux01-review-refinements.md`, `docs/ux-discovery/implementation-handoff.md`, `docs/ux-discovery/native-spike-results.md`, `docs/ux-discovery/native-spike-decision.md`, `plan/ux-redesign-backlog.json`, `plan/backlog.json`, and `plan/progress.md`.

## Authorized deliverable
Build a **reviewable, incrementally integrated production Rust/egui shell** in `pipeline-app` with premium light/dark tokens, clear portfolio/project navigation, project overview and responsive layout, compact assistant entry/composer presentation, collapsible panel/drawer **without live AI integration**, and preserved access to all existing technical views through Advanced. Implement deterministic, service-backed next-action view models from persisted project, task, run, evidence and approval records; each recommendation must show why it is next, any prerequisite, alternative and exact destination. Never invent progress, checks or agent states; show honest unavailable/empty/offline states.

## Execution plan and safety
1. Audit existing view/service contracts and record UX02/UX03 dependency refinements. Do not silently rewrite approved backlog sequencing.
2. Implement the native design system and shell behind a reversible feature flag or equivalent safe rollout; retain legacy views and no data migration.
3. Implement deterministic recommendation logic with unit fixtures for empty, new/imported, planning, approved ready, active, blocked, awaiting review, failed/offline and complete projects, including contradictory/stale evidence.
4. Connect read-only view models to existing services, and route any existing consequential action through its current exact domain service, revision and policy gates. No plain assistant text may authorize writes.
5. Verify compile/test/clippy, regression tests, 1024/1440 widths, 100/200% scale, keyboard focus, Escape and Ctrl/Cmd+K, contrast and AccessKit where available. Record explicitly what could not be tested; test M1 Max when environment available.
6. Commit and push on a dedicated branch; open PR with screenshots, test logs, architecture notes, known gaps, rollback instructions and precise owner-review checklist. **Do not merge without owner review.**

## Out of scope
No real conversational provider, AI proposal apply, autonomous scheduler changes, domain schema migration, release publication, destructive operations, UX04–UX08 completion, or replacement/removal of terminal, diagnostics and existing evidence tools. Keep P16–P19 gates intact. Stop for material architectural or scope decisions.

The owner has approved the native direction and this bounded first production phase, **not** unrestricted redesign or release.
