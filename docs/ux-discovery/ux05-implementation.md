# UX05 implementation and verification

13 October 2026 · Windows development branch `codex/ux05-typed-proposals`. UX05 is implemented for owner PR review. No merge, release, schema migration, policy change, paid inference or external effect occurred.

## Two review increments

1. Contract/service increment: `ProjectEngine::preview_proposal` and `apply_proposal`, strict JSON v1 schema, exact evidence/revision comparison, draft-only eligibility, confirmation and replay guard, negative tests. Commit `ce0f73d`.
2. Native increment: project-scoped in-memory proposal cards (bounded to 32 contexts) in the feature flagged assistant panel, offline manual entry, optional strict parsing of a complete assistant JSON answer, before/after preview, exact proposal confirmation, discard and re-preview. Chat Send remains read-only and provider configuration remains UX04 opt-in.

The [design and service audit](ux05-design.md) records mutation side effects, UX-D11, threat model and recovery. No `ProjectEngine` approval, task acceptance, scheduler, run, policy, terminal or CLI method was widened.

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
| `cargo fmt --all --check` | Pass | [log](../evidence/ux05-fmt.log) |
| `cargo clippy --workspace --all-targets -- -D warnings` | Pass | [log](../evidence/ux05-clippy.log) |
| `cargo test --workspace` | Pass; 97 tests passed, 4 previously ignored; proposal negative tests and existing workspace suite | [log](../evidence/ux05-test.log) |
| `cargo build -p pipeline-app` | Pass | [log](../evidence/ux05-build.log) |
| Native screenshot feature build | Pass | [log](../evidence/ux05-capture-build.log) |

The isolated sample portfolio was created by [`ux05_seed.rs`](../../crates/pipeline-engine/examples/ux05_seed.rs) through `ProjectEngine`, not direct database writes. Captures are from the running Windows Rust/egui binary with no provider configured. They show a validated, unconfirmed draft proposal; no mutation was made for screenshots.

| Native view | Evidence |
| --- | --- |
| 1440 × 900 dark, 100% | [proposal card](../evidence/ux05-proposal-1440-dark.png) |
| 1024 × 900 light, 100% | [proposal card](../evidence/ux05-proposal-1024-light.png) |
| 1440 × 900 light, 200% | [scaled proposal](../evidence/ux05-proposal-1440-light-200.png) |
| 1024 × 900 dark, 200% | [scaled proposal](../evidence/ux05-proposal-1024-dark-200.png) |
| 1440 × 900 dark, scrolled to confirmation | [unconfirmed Apply gate](../evidence/ux05-proposal-confirmation-1440-dark.png) |

At 200% scaling the card and owner confirmation are below the visible fold and require panel scrolling. Programmatic tests cover required confirmation, stale state, denied scope, discard without audit, and replay. A hands-on keyboard traversal through the native confirmation, screen-reader announcement, and macOS run were not performed and remain unverified. The current Windows capture verifies layout and rendering, not those interaction outcomes. Existing `Ctrl+K` and Escape behavior remains covered by the UX04 app tests.

## Acceptance checklist

- [x] Versioned typed proposal data; strict parser and bounded values.
- [x] Exact project, target, revision, before value and evidence check before preview and Apply.
- [x] Owner confirmation separate from Send and preview; fresh check and transactional revisioned save.
- [x] No new path to approve brief/scope/task, run commands, spend or external effects.
- [x] Denied, stale and repeated actions leave project state and audit unchanged in tests.
- [x] Offline manual entry, assistant JSON fallback and project-scoped card state.
- [x] Windows checks and native light/dark scaled captures.
- [ ] Owner PR review and hands-on native confirmation/keyboard assessment.
- [ ] Apple Silicon and screen-reader release gates.
