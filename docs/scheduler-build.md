# P12 scheduler, worktree, policy, and recovery evidence

9 October 2026, Windows x86_64. P12 adds schema v7, application scheduling services, owner policy requests, and startup reconciliation. It builds on the P11 scoped command bridge. No harness is launched in P12; P13 attaches OpenCode process and event control to these run states.

## Run preparation and state

`ProjectEngine::prepare_agent_run` checks harness availability supplied by the adapter boundary, positive time and token budgets, the latest approved brief, an active approved plan, a ready task, accepted blocking dependencies, a committed Git HEAD, and absence of custom checkout filters. It records a checkout intent before invoking Git, then creates a detached worktree from that exact commit with hooks disabled and LFS smudge commands disabled. It verifies the resulting worktree identity and marks it prepared. Only then does the application issue a short-lived, task-scoped capability, persist run limits, and transition the run from `queued` to `starting`. The immutable task packet includes the scope and task revisions, checkout path, goal, criteria, dependencies, approved decisions, allowed operations, limits, and stop conditions.

The run state machine accepts `queued → starting → running → waiting_for_input/review → completed`, with `failed`, `cancelled`, and `interrupted` terminal paths. Exact run revisions are required. `completed` requires the task to have been submitted for review; it does not accept the task. The database admits one active code-changing run per project even after a token expires. A new approved plan scope interrupts an active run, revokes its token and pending action approvals, and retains its worktree. A failed or interrupted run blocks a running task; an owner can resume it with an exact-revision action after inspecting the work.

Checkout intent, Git setup outcome, grant, limits, transitions, checkpoint, owner action, and policy consumption are persisted with activity events. Failed worktree setup is recorded and left for inspection. The application does not remove a dirty worktree automatically.

## Policy gates

Actions are classified as local check, checkout edit, destructive files, Git history rewrite, credential access, spending, external publication, production, or unknown external effect. Local actions require an active run and a canonical target inside its checkout, including symlink containment. Risky classes create a high-priority owner inbox request with a plain-language effect summary, exact target, scope revision, and SHA-256 command digest. The Decisions workspace shows the request and offers **Approve once (15 min)** or **Deny**.

An owner decision uses the request's exact revision. Approval is tied to the run, action class, target, digest, scope, and expiry and can be consumed once. A denial remains denied on retry and after service reopening. Changing the target or command creates a new request. Terminating or interrupting a run revokes outstanding approval records and denies pending requests. No policy method executes the proposed external command itself.

## Recovery

The desktop reconciles open runs once at startup. It records the last run event cursor, Git HEAD when readable, task revision, prior run state, and a count of changed worktree entries. It then marks the run interrupted, revokes its token and unconsumed policy approvals, and retains the checkout. A second reconciliation does not replay the run or duplicate its progress event. Pending or failed checkout intents are reported for inspection. The [generated recovery recording](evidence/p12-recovery-recording.log) includes a run with one untracked partial file and event cursor 1.

## Verification

The [P12 test log](evidence/p12-test.log) covers preflight, custom checkout filter rejection, active-run exclusivity, run revisions, worktree retention after failure, restart reconciliation without replay, scope-change interruption, policy denial persistence, one-use approval, path containment, and v6→v7 migration with a pre-upgrade backup. Windows format, workspace Clippy, and [desktop build](evidence/p12-app-build.log) also passed. Harness lifecycle, enforcement of live time and token usage, actual process stopping, event normalization, and adapter capability negotiation are P13 work; the P12 state and policy services are the boundary for those effects.
