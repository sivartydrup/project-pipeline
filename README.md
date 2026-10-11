# Project Pipeline

Project Pipeline is a planned cross-platform, local-first desktop application for taking software products from idea to launch and ongoing operation. It combines a project portfolio, a detailed product workspace, an integrated terminal, and supervised AI agent runs.

**Status:** Windows foundation through launch and operations is implemented. Release hardening is in progress; native M1 Max verification and release candidate approval remain open. See [implementation progress](plan/progress.md).

## Read first

1. [Product specification](docs/product-spec.md): users, workflows, interface, requirements, and acceptance criteria.
2. [Technical architecture](docs/architecture.md): Rust desktop design, data model, harness adapters, portability, and security.
3. [Agent contract](docs/agent-contract.md): task protocol, write rules, evidence, approval gates, and recovery.
4. [Delivery plan](docs/delivery-plan.md): phased implementation, vertical slices, test gates, and release criteria.
5. [Machine-readable backlog](plan/backlog.json): dependency-ordered work items for an agent runner.
6. [Decision register](docs/decisions.md): approved baseline choices and later implementation decisions.

## Product promise

The owner can capture an idea, approve a product brief and executable plan, then let agents make bounded progress across research, design, implementation, tests, release, and launch. The owner sees what was done, why, with what evidence, and which decisions need attention.

## Current scope

The first usable release targets one owner on Windows and Apple Silicon macOS, with local project folders and local data. Linux remains a portability and CI target. The app connects to an installed OpenCode or Pi harness; it does not bundle a model, provider account, or credentials. The workflow covers idea → approved brief → task plan → agent implementation → evidence review → design/release/launch/operation records. Deployment and publication are recorded manually behind separate scoped approvals; the app does not execute them.

## Build and run

Install a current Rust toolchain, then run:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p pipeline-app -p pipeline-cli
cargo run -p pipeline-app
```

Keep `pipeline-app` and `pipeline-cli` in the same directory. The desktop has a local SQLite portfolio, guided brief/plan approval, a project-scoped terminal, supervised OpenCode and Pi runs, evidence review, and design, release, launch, and operations workspaces. Set `PIPELINE_DATA_DIR` to use a separate local data directory for a test profile. Agent runs require the selected harness executable and its provider/model configuration; prompts incur the provider's own terms and costs. The `spikes/` directory contains the initial UI and PTY prototypes and is excluded from the main Cargo workspace.

On Windows, `powershell -File scripts/package-windows.ps1` builds a portable release archive and SHA-256 checksum under ignored `dist/` from a clean commit. Its `BUILD-INFO.txt` identifies the exact revision. Extract both executables to one folder and run `pipeline-app.exe`. Back up the app data directory before upgrading. Native Apple Silicon installation and smoke verification remain a release gate; a macOS binary should be built on macOS with `cargo build --release -p pipeline-app -p pipeline-cli` and both executables kept together.

The current Windows archive is unsigned and intended for local evaluation. Public distribution awaits release approval and, where available, Windows signing and macOS notarization.

## Approval boundary

The baseline in [decisions.md](docs/decisions.md) is approved at the file revisions in the [approval record](docs/approval-2026-10-08.md). Material scope, provider, security, or release-policy changes create a new decision record before execution. Publishing, spending, production deployment, and credential changes need separate scoped approval.
