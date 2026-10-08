# Brief and research implementation evidence

P08 adds a guided product brief and research workspace through the application service. A brief is stored as an immutable structured revision with a SHA-256 content hash. Owner approval records the exact revision and hash in a transaction with an activity event. A later draft keeps the previous approved revision visible until approved. The UI shows changes by field and lists revision status. Research findings distinguish sourced facts, which require a source URI and access date, from hypotheses.

## Windows journey recording

On 8 October 2026, the debug desktop app ran with `PIPELINE_DATA_DIR` pointing to a separate temporary profile. In its UI, I imported an empty test folder as **P08 Journey**, entered an idea, audience, problem, desired outcome, and constraints, and saved brief revision 1. The UI showed revision 1 as a draft and the portfolio moved to discovery. I approved revision 1 in the UI; its status became approved and the portfolio moved to planning. I then entered a scope, saved revision 2, and expanded the Scope change. The UI showed revision 2 as draft, revision 1 as approved, and the previous and current Scope values. [Windows brief screenshot](evidence/brief-revision-windows.png).

In the Research view, I marked a finding as a hypothesis, entered its claim, summary, and relevance, then recorded it without a source. The finding appeared as a hypothesis with its text and confidence. The same view labels source and access date fields for sourced facts. [Windows research screenshot](evidence/research-windows.png). The test profile and project are outside the repository and do not alter the owner's normal application data.

## Automated verification

Windows `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo build -p pipeline-app` passed. The [test log](evidence/p08-test.log) includes the idea-to-approval journey, database reopen, pending revision, visible field delta, stale revision conflict, source requirement, exact approval hash, and audit rollback checks. M1 Max hands-on testing remains a release gate under P19.
