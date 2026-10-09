# P11 agent write bridge and command specification

The CLI and HTTP bridge call the same `ProjectEngine::agent_command` service. The service delegates each command to a SQLite transaction in `pipeline-store`. Agents never write the database directly.

## Grant lifecycle

An owner service or the future scheduler issues a grant for a `ready` task with accepted dependencies in the active scope. `ProjectEngine::issue_agent_grant` requires the selected project checkout, an operation allowlist, and a TTL from 1 to 900 seconds. One unexpired grant may exist per project. The bridge creates a run intent and stores only the token's SHA-256 digest. The raw 256-bit token is returned once. `ProjectEngine::revoke_agent_grant` closes the run and appends an activity event. A changed active scope or expired or revoked grant denies all subsequent calls. P12 will add isolated checkout preflight and run orchestration.

```text
pipeline-cli call <127.0.0.1:port> # JSON command on stdin; PIPELINE_AGENT_TOKEN in environment
pipeline-cli serve <database> <127.0.0.1:port>
```

The server refuses a non-loopback bind and non-loopback peers. It accepts `POST /v1/command` with JSON and `Authorization: Bearer <token>`. The JSON must omit `token`; the server takes it from the header. Body size is limited to 1 MiB. CLI and HTTP expose the same command result. Capability issuance is intentionally absent from the agent-facing CLI and HTTP surface.

## Command envelope

```json
{
  "project_id": "project-id",
  "run_id": "run-uuid",
  "operation": "task.block",
  "idempotency_key": "unique-run-key",
  "expected_revision": 3,
  "target_id": "optional-task-or-decision-id",
  "payload": {"reason": "Need a design decision"}
}
```

Reads omit the idempotency key and expected revision. Every mutation requires both. The key permits ASCII letters, digits, `_`, and `-`, up to 128 characters. A repeat of the exact command with the same key returns the saved result with `replayed: true` and creates no new activity event. Reusing a key for a different command is denied. The response contains `result`, `revision`, `activity_event_id`, and `replayed`. A stale task or project revision returns HTTP 409; invalid or out-of-scope capabilities return 401 or 403. CLI errors are nonzero exits.

| Operation | Payload and behavior |
| --- | --- |
| `project.get`, `project.context` | Project details; approved brief and decisions. |
| `task.list`, `task.get` | Active scope tasks; `task.get` also reads drafts created under the assigned task. |
| `task.create` | `title`, `outcome`; creates a draft in the next scope, with expected project revision. |
| `task.update` | `target_id`, `title`, `outcome`; edits only a draft linked to the assigned task, with expected draft revision. |
| `task.block` | `reason`; moves the assigned task to blocked, with expected task revision. |
| `task.submit` | `summary`, `changed_files`, `test_results`, `residual_risks`, `criteria` arrays; validates local file paths and run-owned test IDs, then moves the assigned task to review. It cannot accept the task. |
| `decision.propose` | `question`, at least two `alternatives`, `recommendation` selected from them, `impact` (`low`, `medium`, `high`, or `blocking`), optional `rationale` and `evidence`; creates a proposed decision linked to the assigned task. |
| `decision.get` | Reads a decision in the selected project by `target_id`. |
| `question.ask` | `question`, `alternatives`, `recommendation`, optional `rationale` and `evidence`; creates a blocking decision proposal and blocks the assigned task. Pending duplicate questions are denied. |
| `artifact.attach` | `path`, `kind`, `sha256`; the existing file must resolve inside the selected checkout and match its SHA-256 digest. |
| `research.record` | `claim`, `source_uri`, `accessed_at`, `confidence`, optional `summary`; records a sourced finding linked to the assigned task in relevance metadata. |
| `test.record` | `command`, integer `exit_code`, object `environment`, run-owned `log_artifact_id`, optional `git_commit`. |
| `run.progress` | `status`, `next_action`; appends a run event. It does not count as test or artifact evidence. |
| `release.propose` | `version`, `git_commit`, `target`, array `checklist`, `rollback_notes`; records a proposal only. Publishing remains a separate approval action. |

Task-bound mutations require the assigned active task and its exact revision. Agent-created drafts cannot become approved work through the bridge. File paths are canonicalized before containment checks; symlink escapes fail. Authorization and mutation occur in the domain transaction, followed by an activity event and saved idempotent response. Failed attempts enter a separate denial log with a hashed correlation ID and no token or payload. Capability and idempotency tables are omitted from project export. Payloads with credential-like fields, bearer strings, or the live token are denied before persistence. Agent read responses redact credential-like fields and bearer or token text.

## Verification

The [security test log](evidence/p11-security-test.log) covers operation contract, cross-project and cross-task authorization, revocation, scope drift, stale revision, retry/replay, path and hash, and secret redaction. The [workspace test log](evidence/p11-test.log) includes the live HTTP route tests. The [CLI build log](evidence/p11-cli-build.log) confirms the executable builds. Format and [workspace Clippy](evidence/p11-clippy.log) checks also passed on Windows.
