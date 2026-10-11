# P16 design and UX review implementation

Design artifacts now have a Design workspace tab. The owner can record flows, wireframes, screenshots, component notes, copy, assets, and accessibility reviews, with optional active task and decision links. A file reference must resolve inside the project folder. The workspace displays the stored SHA-256 digest, source file, task, decision, run, Git commit, author, version, and review history.

Each save creates an immutable content version. The owner may approve the current version or request a revision with a required note. A later version begins in review and leaves older decisions visible. The application service performs project/link checks and records each create, revision, and review in the activity timeline. Design choices are recorded in D-033 of `docs/decisions.md`.

The sample design-to-implementation journey is executable as `cargo test -p pipeline-store design::tests`. The three Windows tests pass, including revision conflict, owner review, project path, link validation, full kind coverage, export, and retained history. Workspace format and Clippy checks and the desktop app build also pass. Evidence is [the journey log](evidence/p16-design-journey.log), [artifact set](evidence/p16-artifact-set.md), [format check](evidence/p16-fmt.log), [Clippy check](evidence/p16-clippy.log), and [app build](evidence/p16-build.log).

The test uses a temporary sample project. A [Windows desktop visual/form smoke](evidence/p16-p17-ui-smoke.md) confirmed the Design workspace renders and remains responsive. A full hands-on UI creation/review recording and the Apple Silicon smoke check remain unverified, so this record does not claim final cross-platform acceptance.
