# UX05 typed proposal decision and service audit

13 October 2026. Owner authorization: `NEXT-CODEX-UX05.md`. This record applies only to the feature flagged modern shell and the new engine proposal boundary.

## Existing mutation routes and side effects

| Route | Revision and side effects | UX05 use |
| --- | --- | --- |
| `ProjectEngine::save_brief(project, expected_latest, content)` | Store transaction checks project and latest brief revision; creates immutable draft, bumps project revision, updates health/stage, appends `brief.revise` audit. Approval stays separate. | Apply a field edit only while no brief revision is approved. |
| `approve_latest_brief(project, expected_latest)` | Owner actor and exact latest revision; advances approved brief/stage and logs approval. | Never called by a proposal. |
| `save_plan(project, expected_latest, content)` | Store transaction checks project and latest plan revision; creates draft, bumps project revision, updates health, appends `plan.revise` audit. Approval stays separate. | Apply task title/outcome or milestone title/outcome only to an existing draft before any active scope. Validate full resulting plan first. |
| `approve_latest_plan(project, expected_latest)` | Requires current approved brief and owner actor; validates plan, creates scope revision and materializes active tasks, with audit. | Never called by a proposal. |
| Task review/acceptance, policy approval, run methods | Exact task/run revisions and dedicated owner/policy gates. | Never called by a proposal. |

The existing `save_plan` service can save a new draft after an approved scope exists, but UX05 deliberately treats that as read-only preview. The generic service has no proposal-specific material-scope review route. UX05 does not invent one. There is no direct task-draft mutation service; a task draft edit is a new complete plan draft revision through `save_plan`.

## Contract v1

`Proposal::parse_json` accepts a single JSON object of at most 16 KiB. All envelope and change fields are typed; unknown fields, types and variants fail parsing. The envelope includes `version=1`, unique display ID, project ID, expected latest revision, expected approved brief and active scope revisions, exactly one original evidence record ID, reason, impact, claimed affected approved scope, prerequisites, risk, claimed owner approvals, recovery guidance, and a tagged change. Values are bounded to 2,000 bytes and control characters are rejected. Supported changes are `brief_field` (one of the enumerated brief fields), `task_draft` (title/outcome), and `plan_draft` (existing milestone title/outcome). Each has stable target ID and exact before/after strings.

The metadata is untrusted suggestion text. It never grants eligibility. The preview derives eligibility from authoritative service reads and checks the project, current revision, record ID, before value, approved brief/scope revisions and full resulting plan validation. It returns a structured before/after card and an authoritative explanation. `apply_proposal` requires explicit owner confirmation, re-previews the exact proposal, and calls only `save_brief` or `save_plan`. Both services repeat optimistic revision checks in their transactions and append audit atomically. A replay conflicts after the first save. No proposal storage or schema migration is added; cancelled previews are discarded in memory.

## Decision UX-D11: conservative draft boundary

**Status:** Implemented for PR review. **Choice:** Apply only initial unapproved brief field edits (excluding scope, non-goals and milestone plan) and edits to an existing unapproved plan draft before the first active scope. All later approved brief/scope edits are preview only with “Requires separate approval / Not supported yet.” **Reason:** Existing revision services allow draft saves, but UX05 has no dedicated material-scope proposal review gate. **Alternative:** Save all proposed drafts and rely on later approval. **Impact:** Fewer executable proposal types, no authorization or schema change. **Reversibility:** A later owner-approved route can widen eligibility. **Evidence:** proposal negative tests and this service audit.

## Threat model

An assistant reply, repository content, or pasted JSON can lie about approval, risk, project ID, evidence, or prior values. The parser rejects malformed and oversized input; authoritative preview ignores claimed authority and requires exact current IDs and values. The UI identifies suggestion notes as unverified and keeps chat Send, preview, checkbox and Apply separate. Provider calls remain UX04 opt-in and tool-free. No SQL, shell, file path, command, policy, spend, run, release or external-effect change variant exists. Stale evidence clears the actionable card and requires a fresh proposal. Current `ProjectEngine` owner methods presume the interactive local owner session; UX05 adds a UI confirmation and an explicit `owner_confirmed` argument, but it does not create a new remote identity/auth service.

## Recovery

Before Apply, discard the in-memory card. After Apply, the previous immutable brief/plan revision remains available; the owner can save a restoring draft through the existing editor and review it. No scope or task acceptance changes occur. Disable `PIPELINE_MODERN_SHELL=1` to return to the legacy UI; persisted draft revisions remain visible there.
