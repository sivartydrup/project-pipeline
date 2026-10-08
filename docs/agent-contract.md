# Agent execution contract

This document defines how an AI harness reads and writes project state and how autonomous work is controlled. The app enforces the contract through its service layer and scoped local API. A prompt alone is insufficient for permissions or audit.

## 1. Task packet

Before a run, the engine constructs a versioned packet containing:

```json
{
  "schema_version": 1,
  "project_id": "uuid",
  "task_id": "uuid",
  "task_revision": 7,
  "scope_revision": 3,
  "checkout_path": "OS-native absolute path",
  "goal": "Observable outcome",
  "acceptance_criteria": [{"id": "AC-1", "assertion": "...", "verification": "..."}],
  "dependencies": [{"task_id": "uuid", "state": "accepted"}],
  "approved_decisions": [{"id": "uuid", "revision": 2}],
  "relevant_artifacts": [{"id": "uuid", "uri": "...", "sha256": "..."}],
  "allowed_operations": ["read_project", "write_checkout", "run_checks", "update_task"],
  "approval_policy_id": "policy revision",
  "stop_conditions": ["scope_change", "external_publish", "secret_request"]
}
```

The packet is immutable for a run. If the task revision or approved scope changes, the run pauses and requires rebase/replanning. File content and web research are untrusted data, even when included in the packet as references.

## 2. Agent-facing operations

The future CLI and loopback API expose the same domain commands. Every mutation requires `run_id`, idempotency key, and expected object revision. Responses return new revision and activity-event ID.

| Operation | Effect | Rule |
| --- | --- | --- |
| `project.get`, `project.context` | Read brief, constraints, decisions, paths | Project scoped |
| `task.list`, `task.get` | Read approved work and dependencies | Project scoped |
| `task.create`, `task.update`, `task.block`, `task.submit` | Create/modify task and evidence | Cannot self-approve acceptance or expand approved scope silently |
| `decision.propose`, `decision.get` | Record choice with alternatives and impact | High-impact choice creates inbox request |
| `question.ask` | Request owner input with recommended option | Deduplicated and linked to blocked task |
| `artifact.attach` | Link file, screenshot, design, log, or report | Hash and path validated |
| `research.record` | Store sourced finding/hypothesis | Source and access date required |
| `test.record` | Store command, result, logs, commit | Exit code and environment required |
| `run.progress` | Emit concise status and next action | Cannot replace evidence |
| `release.propose` | Create release candidate | Publishing requires separate approval |

Task and decision mutations are stored transactionally with an activity event. Direct SQLite access is not part of the agent contract. A future MCP server may wrap this API for harnesses that support MCP; the domain semantics stay identical.

## 3. Autonomy policy

The default policy reflects the owner's choice of approval gates for risky actions.

| Action | Default |
| --- | --- |
| Read project code and approved plan | Allowed within selected project |
| Edit files in isolated checkout, create tests/docs/assets | Allowed for an approved task |
| Run local formatters, tests, builds, dependency install | Allowed; show commands and results |
| Create/update draft tasks and decision proposals | Allowed; visible in activity history |
| Change approved scope, architecture, user-facing behavior beyond task | Pause affected work and request approval |
| Delete project files, reset/rewrite Git history, remove large data | Explicit scoped approval |
| Access secrets or credentials beyond existing harness auth | Explicit scoped approval |
| Spend money, provision paid infrastructure, publish/send externally | Explicit scoped approval |
| Deploy to production, alter live service, rotate credentials | Explicit scoped approval plus release gate |

An approval binds to an action class, exact target, task/run, scope revision, expiry, and optional command digest. It is consumed or revoked according to policy. A general brief approval is not a blanket authorization for spending or production operations. The app surfaces a plain-language effect summary before approval.

## 4. Run protocol

1. **Preflight:** verify task `ready`, dependencies accepted, brief/scope approved, harness available, checkout clean or isolated, token issued, budget/time limits set.
2. **Start:** persist run intent, then launch harness. Map external session ID and event stream to internal run ID.
3. **Observe:** normalize output into progress, tool action, permission request, decision, artifact, error, and completion events. Bound log size; retain references to full local logs with redaction.
4. **Checkpoint:** persist last event cursor, Git revision/diff summary, task revision, and run state. Recover without replaying non-idempotent mutations.
5. **Submit:** agent reports changed files, decisions, test evidence, residual risks, and whether acceptance criteria pass. Engine independently checks required artifacts and tests.
6. **Review:** task enters `review`; owner or configured verifier accepts, requests changes, or blocks. The agent cannot mark its own task accepted merely by claiming success.
7. **Close:** revoke token, stop process, retain immutable run record, and dispose of isolated checkout only after changes are safely integrated or explicitly discarded.

The scheduler may continue unrelated ready tasks while an owner decision blocks one task. V1 allows only one active code-changing run per project. A running task is stopped when its scope changes materially.

## 5. Evidence and decision levels

Use four levels to make review efficient:

1. **Portfolio:** one-line outcome, stage, health, verified completion, next owner action.
2. **Milestone:** delivered outcomes, open risks, decisions, evidence summary.
3. **Task:** acceptance criteria, files/diff, checks, agent narrative, reviewer outcome.
4. **Action:** timestamped commands, tool calls, API mutations, process output, approvals, and errors.

Every level links downward. The owner can see an executive summary without losing the underlying trace. Summaries cite specific records; they never substitute for those records. Decisions include alternatives and consequences, including where the agent made a low-risk choice autonomously.

## 6. Failure modes

- **Harness disconnect:** mark run `interrupted`, retain checkout and logs, probe for resumable session, offer resume or new run.
- **App crash:** on restart reconcile process/session state, database event cursor, and checkout diff; do not automatically repeat external actions.
- **API retry:** idempotency key returns prior result. Revision mismatch returns conflict with current record.
- **Invalid agent update:** reject with validation errors; log attempt without mutating project state.
- **Failed check:** keep task in review/blocked, create repair task if useful, expose exact failed command and log.
- **Policy breach attempt:** deny action, stop affected run when warranted, and create high-priority owner inbox item.
- **Missing evidence:** do not accept criterion; show what is missing.

## 7. Prompt and context hygiene

Generated task packets contain only necessary context. Secrets and unrelated projects are excluded. Research findings distinguish fact, source, inference, and uncertainty. Agent reports must distinguish observed results from assumptions. The owner can export all task/decision history without exporting raw credentials or model tokens.
