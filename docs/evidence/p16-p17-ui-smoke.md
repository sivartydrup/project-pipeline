# Windows desktop visual smoke — 11 October 2026

Launched `target/debug/pipeline-app.exe` on Windows after the integrated P16/P17 build. The selected local project loaded. The Design tab rendered its artifact list, type selector, title, notes, project file, task/decision links, provenance fields, and review note. The Release tab rendered its candidate selector and creation form with version, full Git commit, deployment target, and rollback procedure. Both tabs remained responsive while switching between them.

This is a visual/form smoke check. No artifact or release candidate was created through the desktop in this check; the corresponding state transitions are covered by the P16 journey and P17 release service tests. Terminal command input through Windows UI automation was not attempted.
