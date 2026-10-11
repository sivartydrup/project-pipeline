# P19 focused security and data-loss review

11 October 2026, Windows. Scope: P16 design, P17 release, P18 launch/operations, the agent bridge, policy gates, export, and migration/restore paths. This is a focused code review and local test, not a full security certification.

## Findings fixed

1. **Changed design file could be approved under its original hash.** `review_design_artifact` checked version/status but did not recompute file bytes. It now rehashes the submitted body and canonical in-project file before approval, and rejects a changed screenshot. See `crates/pipeline-store/src/design.rs` and its journey test.
2. **Release and launch artifacts could change after readiness review.** Release builds and check evidence were rehashed at readiness but not at execution approval or deployment recording. Linked launch assets were not rehashed at publication approval or recording. These steps now reverify bytes and canonical project-root containment. Release file hashing streams fixed-size blocks, so large builds do not load into RAM. Passing check evidence must match the candidate's exact commit. Tamper tests cover changes before execution, deployment, and publication. See `crates/pipeline-store/src/release.rs` and `operations.rs`.
3. **External-effect approvals did not expire.** Release execution and external publication approvals now expire after 24 hours; recording rejects expired, revoked, wrong-target, changed-revision, or reused approvals. Readiness approval remains an immutable candidate record. Expiry tests cover both flows.
4. **Owner-entered credential-shaped text could be persisted in workflow records.** A narrow shared guard now rejects obvious password/API key/token assignments, bearer text, long `sk-`/`ghp_` keys, and private-key markers before new brief, plan, decision, research, design, release, or lifecycle writes. Agent-command payload rejection remains in place. The guard avoids ordinary text such as “passwordless login” and “risk-based review”; workflow tests confirm rejection occurs before any activity event is committed. Export redaction remains a second layer.

## Boundaries checked

- The agent bridge validates scoped capabilities, expiry, active scope, idempotency, and canonical checkout paths. Existing denial and path tests pass.
- Policy action consumption checks exact run/action/target/digest, current scope, expiry, and one-use state; the restart/denial test passes.
- Backup/restore and migration tests pass, including v11-to-v12 preservation and restoration of a pre-upgrade backup. `restore_from` is documented as a repair-only operation requiring exclusive database ownership.
- The sample release and launch flows do not execute deployment or publication; they record owner-approved manual results.

## Remaining limits for the P19 gate

- Credential detection is intentionally pattern based; it cannot guarantee detection of arbitrary unlabelled secrets or remove plaintext that may already exist in older databases. Existing data needs a user-guided review or repair procedure before distribution where that risk matters.
- `cargo audit` and `cargo deny` are unavailable on this host. No global tool was installed. Dependency advisory review remains outstanding; see [tool output](p19-dependency-audit-unavailable.log).
- Cross-platform runtime, packaging, signing/notarization, and CI checks remain separate P19 gates.

## Verification

- [Store suite](p19-security-store.log): 46 tests passed, including tamper, expiry, path, migration, restore, export-redaction, and credential-input tests.
- [Policy gate](p19-security-policy.log): exact one-use approval and persisted denial test passed.
- [Clippy](p19-security-clippy.log): `pipeline-store` all targets passed with warnings denied.
