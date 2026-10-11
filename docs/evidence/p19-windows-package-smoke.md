# Windows portable package smoke — 11 October 2026

Built the final source with `pwsh -NoProfile -File scripts/package-windows.ps1`; the [build log](p19-windows-package-final.log) ends with successful optimized builds of `pipeline-app` and `pipeline-cli`.

Archive: `dist/project-pipeline-0.1.0-windows-x86_64-20261011-134155.zip` (local ignored output). SHA-256: `8b1f91fdf76852817411848bcf74495e963d8be0f4f32c0378b1ac1db438d7c0`. The checksum sidecar matched an independent `Get-FileHash` calculation. The archive contains `pipeline-app.exe`, `pipeline-cli.exe`, `README.md`, and `LICENSE` in one folder.

Extracted the archive to a fresh ignored `dist/smoke-…` directory and launched its `pipeline-app.exe` on Windows. The app opened the existing local portfolio and rendered Overview and the Launch & Ops create form. It remained responsive while switching tabs and closed normally. No sample project data, deployment, or publication was changed in this smoke.

This confirms package extraction and launch on this Windows machine. Full packaged onboarding/import, terminal input, agent run/recovery, and assistive-technology journeys remain open release checks.
