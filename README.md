# Project Pipeline

Project Pipeline is a planned cross-platform, local-first desktop application for taking software products from idea to launch and ongoing operation. It combines a project portfolio, a detailed product workspace, an integrated terminal, and supervised AI agent runs.

**Status:** baseline approved on 8 October 2026; implementation underway. See [approval record](docs/approval-2026-10-08.md).

## Read first

1. [Product specification](docs/product-spec.md): users, workflows, interface, requirements, and acceptance criteria.
2. [Technical architecture](docs/architecture.md): Rust desktop design, data model, harness adapters, portability, and security.
3. [Agent contract](docs/agent-contract.md): task protocol, write rules, evidence, approval gates, and recovery.
4. [Delivery plan](docs/delivery-plan.md): phased implementation, vertical slices, test gates, and release criteria.
5. [Machine-readable backlog](plan/backlog.json): dependency-ordered work items for an agent runner.
6. [Decision register](docs/decisions.md): approved baseline choices and later implementation decisions.

## Product promise

The owner can capture an idea, approve a product brief and executable plan, then let agents make bounded progress across research, design, implementation, tests, release, and launch. The owner sees what was done, why, with what evidence, and which decisions need attention.

## Initial scope

The first usable release targets one owner on Windows and Apple Silicon macOS, with local project folders and local data. Linux remains a portability and CI target, with release support following primary-platform validation. It connects to an installed agent harness rather than bundling a model or provider account. The first end-to-end workflow is idea → approved brief → task plan → agent implementation → review → verified release candidate. Deployment and marketing are represented in the model and plan; provider-specific automation follows after the core workflow is reliable.

## Build the current foundation

Install a current Rust toolchain, then run:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p pipeline-app
```

The desktop imports existing folders and Git repositories, creates project folders, and shows projects from a local SQLite database. Portfolio stage, health, blocked count, and verified completion come from persisted state. The Brief and Research views support guided idea intake, versioned briefs, source-aware research, and exact-revision approval. The lower panel has project-scoped PTY terminal tabs. Planning and agent workspaces are still placeholders. Set `PIPELINE_DATA_DIR` to use a separate local data directory for a test profile. The `spikes/` directory contains the initial UI and PTY prototypes and is excluded from the main Cargo workspace. See [implementation progress](plan/progress.md) for remaining verification.

## Approval boundary

The baseline in [decisions.md](docs/decisions.md) is approved at the file revisions in the [approval record](docs/approval-2026-10-08.md). Material scope, provider, security, or release-policy changes create a new decision record before execution. Publishing, spending, production deployment, and credential changes need separate scoped approval.
