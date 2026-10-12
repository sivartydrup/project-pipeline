# UX discovery work record

12 October 2026 · Branch `codex/ux-discovery-prototype` · This record is separate from approved `plan/backlog.json` and does not accept UX implementation tasks.

| Item | State | Evidence and remaining gate |
| --- | --- | --- |
| UX00 · Baseline audit | Prepared for review | [Audit and journey map](../docs/ux-discovery/ux00-audit.md), historical Windows screenshot inventory, code references and accessibility risk log. Baseline timings and fresh native user testing remain unmeasured. |
| UX01 · Visual prototype | Prepared for review | [Design/feasibility](../docs/ux-discovery/ux01-design.md), [interactive prototype](../prototype/README.md), [screenshots](../prototype/screenshots/), [browser smoke log](../docs/evidence/ux01-prototype-smoke.log). Owner feedback/visual approval and native egui validation remain open. |

Owner review on 12 October approved the overall direction for continued design refinement and requested specific interaction corrections. The [refinement record](../docs/ux-discovery/ux01-review-refinements.md) and [expanded smoke log](../docs/evidence/ux01-refinement-smoke.log) cover those corrections. UX01 remains prepared for review; native egui validation and final owner signoff remain open.

`plan/ux-redesign-backlog.json` remains proposed. UX02–UX08 are not started or accepted. The [implementation handoff](../docs/ux-discovery/implementation-handoff.md) recommends dependency and verification refinements for owner review.

The owner subsequently authorized a bounded native egui feasibility spike in [the Codex handoff](../docs/ux-discovery/NEXT-CODEX-NATIVE-SPIKE.md). Its [Windows results](../docs/ux-discovery/native-spike-results.md), [native screenshots](../docs/evidence/native-spike/) and [proposed decision](../docs/ux-discovery/native-spike-decision.md) are submitted for review. The native spike does not accept UX02–UX08, alter the approved backlog, or resolve the remaining accessibility and M1 Max gates.

On 12 October the owner approved the scoped UX02/UX03 production phase in [the implementation handoff](../docs/ux-discovery/NEXT-CODEX-UX02-UX03.md). The [first implementation record](../docs/ux-discovery/ux02-ux03-implementation.md) links native screenshots and verification logs. UX02/UX03 are implemented for owner review, with interactive accessibility and M1 Max checks still open; neither task is marked accepted by this record. P16–P19 sequencing and release gates remain unchanged.

PR #4 review refinements add project/portfolio draft isolation, persisted `ProjectEngine` recommendation journeys and a Windows native interaction smoke. The [updated implementation record](../docs/ux-discovery/ux02-ux03-implementation.md), [native smoke checklist](../docs/evidence/ux02-ux03-review-native-smoke.md), and linked verification logs are attached as review evidence. Sampled Windows keyboard and AccessKit checks passed; screen-reader announcements, a full contrast audit, narrower viewports and M1 Max remain open. Owner approval and merge are still pending.
