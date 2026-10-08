# Windows foundation build evidence

Environment: Windows x86_64, Rust/Cargo 1.96.1, 8 October 2026.

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed for P04 scaffold; rerun after P05 |
| `cargo test --workspace` | Passed for P04 scaffold; rerun after P05 |
| `cargo build -p pipeline-app` | Passed |
| Native executable launch | Window title `Project Pipeline`, version `0.1.0`, portfolio/management/lower area visible |
| `cargo test -p pipeline-store` | Six integrity/recovery tests passed |

After P05, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` passed again. The app currently uses explicit prototype data and a terminal placeholder. The three-OS workflow is configured in `.github/workflows/rust.yml` but has not run yet. CI execution is a P19 release gate under D-011.
