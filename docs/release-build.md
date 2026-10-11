# P17 release candidate and manual deployment record

11 October 2026, Windows x86_64. P17 implements a manual release workflow; no deployment command is executed by the app.

## Workflow

The Release workspace creates a candidate bound to a version, full Git commit, target, and rollback procedure. The owner attaches a build file from the project folder with platform and exact commit provenance, then records quality, security, privacy, or other checks with optional evidence files. Evidence is hashed on attachment and checked again at readiness approval.

Readiness approval requires a build and all required checks to pass. It creates an approval for the immutable candidate snapshot. A separate owner action approves execution for that candidate revision, snapshot hash, and target. The owner can then record the result of a manual deployment and its smoke result. A pending smoke result can be completed later. Approval IDs are one use for a deployment record; check changes after readiness approval require a new candidate.

Migration 011 adds build, check, and deployment records with foreign keys. Mutations use the store service, revision checks, and transactional activity events. D-032 records the separation between readiness and execution approvals.

## Verification and evidence

- [Focused service tests](evidence/p17-test.log) cover a failed required check blocking readiness, a passing retry, stale revision and non-owner denial, separate exact-target execution approval, manual result, and replay denial.
- [Sample release record](evidence/p17-release-record.json) and [run log](evidence/p17-sample.log) were produced by `cargo run -p pipeline-store --example p17_release_record`; they include the blocked failed check and a passing manual deployment/smoke record.
- [Clippy log](evidence/p17-clippy.log) and [desktop build log](evidence/p17-build.log) pass on Windows with the Release workspace.
- [Windows desktop visual/form smoke](evidence/p16-p17-ui-smoke.md) confirmed the Release tab and candidate form render and remain responsive.

Apple Silicon hands-on release validation remains part of P19. This implementation records manual execution only; a provider adapter and production command execution are outside P17.
