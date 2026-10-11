# Windows portable package smoke — 11 October 2026

Built the clean commit `3f30b8393533b35efe3f689532bb5de3d657a16b` with `pwsh -NoProfile -File scripts/package-windows.ps1`; the [build log](p19-windows-package-commit.log) ends with successful optimized builds of `pipeline-app` and `pipeline-cli`.

Archive: `dist/project-pipeline-0.1.0-windows-x86_64-20261011-135739.zip` (local ignored output). SHA-256: `b61bec8bda05dc5531204ea390f70ec46fab0883ea2c7b044d52461dc19fc226`. The checksum sidecar matched an independent `Get-FileHash` calculation. The archive contains `pipeline-app.exe`, `pipeline-cli.exe`, `README.md`, `LICENSE`, and `BUILD-INFO.txt` in one folder. The manifest reports version `0.1.0`, the exact commit above, `windows-x86_64`, UTC build time, and Rust 1.96.1.

Extracted the archive to a fresh ignored `dist/smoke-…` directory and launched its `pipeline-app.exe` on Windows. The app opened the existing local portfolio and rendered Overview, then closed normally. No sample project data, deployment, or publication was changed in this final smoke. The earlier [form smoke](p16-p17-ui-smoke.md) and P18 visual check covered the lifecycle tabs.

This confirms package extraction and launch on this Windows machine. Full packaged onboarding/import, terminal input, agent run/recovery, and assistive-technology journeys remain open release checks.
