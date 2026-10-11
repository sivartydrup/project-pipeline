# P19 three-OS CI — 11 October 2026

[Rust checks run 38106963959](https://github.com/sivartydrup/project-pipeline/actions/runs/38106963959) completed successfully for pushed commit `a363d932d7616268d8f978e3d2a6bc1db9421e6f` on `codex/lifecycle-release-hardening`.

| Job | Result |
| --- | --- |
| Windows Server 2025 x86_64 | Passed formatting, workspace Clippy, workspace tests, app/CLI build, and binary artifact upload. |
| macOS 15 ARM64 | Passed runner architecture assertion, formatting, workspace Clippy, workspace tests, app/CLI build, and binary artifact upload. |
| Ubuntu 24.04 x86_64 | Passed formatting, workspace Clippy, workspace tests, app/CLI build, and binary artifact upload. |

Retained GitHub artifacts: `project-pipeline-windows-x86_64`, `project-pipeline-macOS-ARM64`, and `project-pipeline-Linux-X64`. CI proves compilation and tests on Apple Silicon; it does not replace the native M1 Max interactive install, PTY, harness, review, and accessibility release checks.
