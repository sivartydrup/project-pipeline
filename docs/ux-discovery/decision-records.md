# UX design decision records · proposed

## UX-D08 · UX04 conversational boundary and spending gate

**Status:** Owner approved isolated, tool-free live chat architecture on 12
October 2026; implementation choice is in PR review. **Decision:** Keep the
assistant separate from OpenCode and Pi task-run adapters. Require explicit
provider/model/endpoint configuration and Send. Allow loopback inference
endpoints; keep remote paid requests closed until a provider-specific,
verifiable spending cap can be enforced. **Alternative:** Reuse task-run
sessions with prompt-only restrictions. **Reason:** Existing task adapters
grant checkout, write bridge and tool access, while prompts cannot remove
those capabilities. Generic remote APIs do not prove a monetary cap.
**Impact:** New in-memory chat transport and UI only; no schema, scheduler,
approval or task-run changes. **Evidence:**
[UX04 audit and design](ux04-design.md). **Reversibility:** Disable the modern
shell or remove the chat transport; no project data migration is needed.

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

## UX-D06 · First production rollout boundary

**Status:** Implemented for owner review under the scoped 12 October UX02/UX03 authorization. **Choice:** Enable the new egui shell only with `PIPELINE_MODERN_SHELL=1`, preserve the legacy management and terminal screens in Advanced, and derive guidance through a read-only `ProjectEngine` method. **Alternative:** Replace the default shell immediately. **Reason:** The feature flag allows native usability and accessibility review without changing current workflows or data. **Impact:** No schema, scheduler, provider, approval or release-policy change. **Evidence:** [implementation record](ux02-ux03-implementation.md) and linked screenshots/checks. **Reversibility:** Unset the flag; no migration or repair is required. **Open gate:** Owner review plus interactive keyboard/screen-reader validation before considering default enablement.

## UX-D07 · Draft context and recovery guidance

**Status:** Implemented for PR #4 owner review on 12 October 2026. **Choice:** Keep one in-memory draft per project ID and one separate portfolio draft; the compact and expanded editors bind to the active context. Show an interrupted or failed persisted run before a generic blocked-task inbox entry. **Reason:** Project switching must not reveal another project's unsent text, and saved run recovery is the more specific action when that run caused the blocker. **Alternative:** Retain one shell-wide draft and the original generic blocker priority. **Impact:** UI memory and read-only recommendation order only; no persistence, provider, scheduler, policy, schema or approval change. **Evidence:** [refinement record](ux02-ux03-implementation.md) and [workspace tests](../evidence/ux02-ux03-review-test.log). **Reversibility:** Both choices are local to the feature-flagged shell and recommendation function.
