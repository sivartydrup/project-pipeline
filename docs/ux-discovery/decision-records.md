# UX design decision records · proposed

These records keep material choices visible for owner review. They do not change D-009, the approved product scope, or action policy. Owner approval and exact implementation revision are pending.

## UX-D01 · Navigation and disclosure

**Status:** Proposed. **Question:** Should the dashboard and task-oriented project sections replace the tab-first presentation, with technical tools under Advanced?

**Recommendation:** Yes: Home/Projects/Inbox/Activity/Settings globally and Overview/Roadmap/Work/Design/Releases/History inside a project. Preserve raw evidence, terminal and diagnostics in Advanced. **Alternative:** Keep eight flat management tabs and improve their labels. **Reason:** The current review crosses Runs and Plan, and the Overview action can misroute to Decisions. **Impact:** Material navigation and learnability change; records and capabilities remain. **Evidence:** [UX00 audit](ux00-audit.md), [prototype](../../prototype/README.md). **Reversibility:** UI-only layout can be revised; no data migration in this decision.

## UX-D02 · Assistant behavior

**Status:** Proposed. **Question:** Should the default experience use one guided recommendation plus a persistent composer and collapsible right assistant?

**Recommendation:** Yes, using a deterministic domain recommendation and read-only assistant explanation; typed changes require preview and existing service approvals. **Alternative:** Open a separate assistant screen on demand. **Reason:** Keeps project context and owner action in view. **Impact:** New assistant architecture and provider/cost disclosure are required; no conversational authority. **Evidence:** [UX01 design](ux01-design.md), approved design-direction proposals. **Reversibility:** Panel placement can change without changing domain records.

## UX-D03 · Guided Auto semantics

**Status:** Proposed. **Question:** Should Guided Auto be the default mode after implementation, and what exact continuation/pause semantics apply?

**Recommendation:** Allow continuation only after fresh deterministic preflight for approved, ready, unblocked tasks with authorized provider/model, enforceable limits and one active code-changing run per project. Agent submission waits for owner acceptance; a separate eligible task may run if policy allows. “Pause after current task” stops future starts; “Pause now” safely cancels the active run and preserves evidence. **Alternative:** Suggest-only default until native validation establishes trust. **Reason:** Matches selected balanced-autonomy direction while retaining the agent contract. **Impact:** Material scheduler/UI behavior and potential provider use; requires exact owner authorization and negative tests. **Evidence:** [AI-guided direction](../proposals/ai-guided-experience.md), [agent contract](../agent-contract.md). **Reversibility:** Mode can be changed; externally incurred provider usage cannot be undone.

## UX-D04 · Implementation sequence

**Status:** Proposed. **Question:** Should UX02–UX08 be prioritized relative to P16–P19 after visual signoff?

**Recommendation:** Authorize a new scoped sequence that preserves P16–P19 acceptance and release gates, with native egui feasibility before production shell replacement. **Alternative:** Finish P16–P19 first and defer UX work. **Reason:** Current interface creates avoidable comprehension work, but release-domain capabilities remain unfinished. **Impact:** Material backlog sequencing; requires an approved scope/dependency update. **Evidence:** [handoff](implementation-handoff.md), `plan/progress.md`. **Reversibility:** Planning sequence can be revised; implementation effort is sunk.
