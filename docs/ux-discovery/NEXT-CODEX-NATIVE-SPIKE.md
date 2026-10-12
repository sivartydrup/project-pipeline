# UX native feasibility spike — Codex start handoff

Owner approved the UX00/UX01 browser prototype and its refinements on 2026-10-12; merged in PR #2 (merge commit `78aa665`). This document authorizes **only a bounded, isolated native Rust/egui feasibility spike**, not the production UX02–UX08 redesign or changes to approved P16–P19 scope.

## Read first
`AGENTS.md`, `README.md`, `docs/product-spec.md`, `docs/architecture.md`, `docs/decisions.md`, `docs/proposals/ai-guided-experience.md`, `docs/proposals/modern-product-studio-screens.md`, `docs/ux-discovery/ux00-audit.md`, `docs/ux-discovery/ux01-design.md`, `docs/ux-discovery/ux01-review-refinements.md`, `docs/ux-discovery/implementation-handoff.md`, `plan/ux-redesign-backlog.json`, `plan/backlog.json`, `plan/progress.md`, and `prototype/README.md`.

## Objective
Create a small **native eframe/egui spike**, isolated from the production app, to prove or falsify the approved Modern Product Studio shell: portfolio focus card, project overview, always-visible compact composer, collapsible right assistant, narrow-window drawer, work-review summary and Advanced access. Reuse the approved visual direction and illustrative Northstar/Fieldnotes data; do not connect a real provider or mutate project data.

## Verification
- Demonstrate at 1024 and 1440 px wide, 100% and 200% scaling; document minimum readable width, panel behavior and text wrapping.
- Verify keyboard navigation, focus transfer/return, Escape, Ctrl/Cmd+K, visible focus and AccessKit behavior where a screen reader is available. Record unavailable environments honestly.
- Show light/dark modes, project switching, context-aware assistant and preservation of unsent draft/conversation while the app is open.
- Distinguish Ready, Working, Pause requested, Paused, Blocked, Awaiting review and Offline without equating submission to acceptance.
- Run targeted tests, compile checks, screenshots where possible, and document measured gaps. Do not claim native accessibility or Apple Silicon validation unless actually tested.

## Boundaries and deliverables
Use a dedicated branch and isolated spike path/crate; do not replace `crates/pipeline-app` shell, change domain policy/scheduler, connect AI, mark UX02–UX08 complete, or resequence P16–P19. Commit and push the spike, instructions, evidence, risks and recommended production architecture. Open a PR **for owner review only**, do not merge. Stop for material architectural decisions. The owner must approve the native result and separate implementation scope before production changes begin.
