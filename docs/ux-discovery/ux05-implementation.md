# UX05 implementation and verification

13 October 2026 · Windows development branch `codex/ux05-typed-proposals`. UX05 is implemented for owner PR review. No merge, release, schema migration, policy change, paid inference or external effect occurred.

## Two review increments

1. Contract/service increment: `ProjectEngine::preview_proposal` and `apply_proposal`, strict JSON v1 schema, exact evidence/revision comparison, draft-only eligibility, confirmation and replay guard, negative tests. Commit `ce0f73d`.
2. Native increment: project-scoped in-memory proposal cards (bounded to 32 contexts) in the feature flagged assistant panel, offline manual entry, optional strict parsing of a complete assistant JSON answer, before/after preview, exact proposal confirmation, discard and re-preview. Chat Send remains read-only and provider configuration remains UX04 opt-in.

The [design and service audit](ux05-design.md) records mutation side effects, UX-D11, threat model and recovery. No `ProjectEngine` approval, task acceptance, scheduler, run, policy, terminal or CLI method was widened.

## PR #6 review refinements

The proposal editor now binds each validated card to the selected target and exact manual text. Text edits and target changes invalidate the card immediately; project switches, new assistant suggestions, malformed suggestions, re-preview, discard and stale revisions clear prior owner confirmation. Apply consumes only the exact card displayed to the owner. A visible card requests a repaint every 500 ms to recheck persisted state, and the engine checks again immediately before a save.

Brief proposals now build the complete resulting brief without persistence and enforce the existing required intake rule before eligibility. Apply uses the same candidate builder. Invalid or incomplete outcomes give a field-specific error and leave revisions and audit unchanged. The ordinary brief editor's existing draft-save behavior was not changed.

New regression tests cover text edited after preview, target and project switches, discard, assistant proposal replacement, confirmation reset, complete and incomplete brief candidates, stale revisions, forged invalid Apply, and unchanged audit after rejection. The [Windows native interaction record](../evidence/ux05-review-native-smoke.md) documents actual UI actions, service snapshots and keyboard navigation.

The first CI run on the pushed refinement exposed a Rust toolchain deprecation in the existing assistant worker counter under `-D warnings`. Its bounded reservation now uses a portable compare and exchange loop; this does not change chat permissions or worker limits.
The next CI run reached the proposal tests and found that their temporary database directory was dropped before later writes on Unix. Each test now retains its fixture directory for the full test scope.

## Supported and restricted actions

| Type | Eligible Apply | Restriction |
| --- | --- | --- |
| Brief field | Existing unapproved brief; idea, audience, problem, desired outcome, constraints, value proposition, user journeys, success metrics, UX principles, architecture candidates, costs or risks | Scope, non-goals, milestone plan, and any brief with a prior approval remain read-only previews. |
| Task draft | Existing task title or outcome in an unapproved plan draft before the first active scope | No task creation, acceptance, evidence, weight, risk, criteria, command or dependency changes. |
| Plan draft | Existing milestone title or outcome in an unapproved plan draft before the first active scope | No scope approval, graph/dependency changes or new milestone. |

The manual form offers brief idea, task title and milestone title. Other allowed fields require a complete v1 JSON suggestion validated by the same service. There is no configured or automatic inference requirement.

## Windows checks

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo fmt --all --check` | Pass | [review log](../evidence/ux05-review-fmt.log) |
| `cargo clippy --workspace --all-targets -- -D warnings` | Pass | [review log](../evidence/ux05-review-clippy.log) |
| `cargo test --workspace` | Pass; 101 tests passed, 4 previously ignored, 0 failed | [review log](../evidence/ux05-review-test.log) |
| `cargo build -p pipeline-app` | Pass | [review log](../evidence/ux05-review-build.log) |
| Native screenshot feature build | Pass | [review log](../evidence/ux05-review-capture-build.log) |

The isolated sample portfolio was created by [`ux05_seed.rs`](../../crates/pipeline-engine/examples/ux05_seed.rs) through `ProjectEngine`, not direct database writes. Captures are from the running Windows Rust/egui binary with no provider configured. They show a validated, unconfirmed draft proposal; no mutation was made for screenshots.

| Native view | Evidence |
| --- | --- |
| Review refinement, 1440 × 900 dark, 100% with explicit target ID/field | [updated proposal card](../evidence/ux05-review-proposal-1440-dark.png) |
| Review refinement, 1440 × 900 dark, scrolled to confirmation | [updated unconfirmed Apply gate](../evidence/ux05-review-confirmation-1440-dark.png) |
| 1440 × 900 dark, 100% | [proposal card](../evidence/ux05-proposal-1440-dark.png) |
| 1024 × 900 light, 100% | [proposal card](../evidence/ux05-proposal-1024-light.png) |
| 1440 × 900 light, 200% | [scaled proposal](../evidence/ux05-proposal-1440-light-200.png) |
| 1024 × 900 dark, 200% | [scaled proposal](../evidence/ux05-proposal-1024-dark-200.png) |
| 1440 × 900 dark, scrolled to confirmation | [unconfirmed Apply gate](../evidence/ux05-proposal-confirmation-1440-dark.png) |

At 200% scaling the card and owner confirmation are below the visible fold and require panel scrolling. Programmatic tests cover required confirmation, stale state, denied scope, discard without audit, and replay. The PR review refinement completed a Windows native keyboard traversal through confirmation and Apply; [the record](../evidence/ux05-review-native-smoke.md) distinguishes those interactive checks from static captures. Screen-reader announcements and macOS remain unverified. Existing `Ctrl+K` and Escape behavior remains covered by the UX04 app tests.

## Acceptance checklist

- [x] Versioned typed proposal data; strict parser and bounded values.
- [x] Exact project, target, revision, before value and evidence check before preview and Apply.
- [x] Owner confirmation separate from Send and preview; fresh check and transactional revisioned save.
- [x] No new path to approve brief/scope/task, run commands, spend or external effects.
- [x] Denied, stale and repeated actions leave project state and audit unchanged in tests.
- [x] Offline manual entry, assistant JSON fallback and project-scoped card state.
- [x] Windows checks and native light/dark scaled captures.
- [x] Windows native confirmation, Apply, stale, discard, project switch and keyboard interaction smoke.
- [ ] Owner final PR review.
- [ ] Apple Silicon and screen-reader release gates.
