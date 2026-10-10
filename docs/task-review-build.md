# P14 agent task and evidence review loop

10 October 2026, Windows x86_64. P14 implementation is in verification. The disposable model-driven owner journey is pending a separate scoped prompt approval; the P13 one-prompt approval was consumed.

## Implemented path

- A ready task launches from the desktop Runs tab with an explicit OpenCode executable, provider, and model. The owner action records a one-use approval for that exact run packet and model before dispatch; consumption is recorded before the external prompt. Preflight creates a detached worktree, a short-lived scoped bridge token, and an immutable task packet. The packet and its SHA-256 digest persist in schema v9 with an audit event.
- The desktop starts a loopback `pipeline-cli` bridge and a password-protected OpenCode server. The run token remains in the OpenCode child environment and is absent from the prompt and project files. Runtime config denies unspecified tools, allows local reading and editing, and asks before shell commands. The adapter verifies the effective runtime permission settings before sending the prompt. The stream is connected before the prompt, and normalized events, pending permissions, and decisions appear in Runs/Decisions.
- The UI can stop a run, continue an exact permission after an owner decision, inspect the latest 200 recorded events and the submission, preview the checkout diff, review tests and hashed artifacts, and navigate to criterion verification. The worker stops after 15 minutes or when reported usage reaches 20,000 tokens; model usage can overshoot between reports. An interrupted or failed run retains its checkout.
- On agent submission, the worker captures a full tracked Git patch as a hashed artifact and closes the run for owner review. Owner acceptance of an agent-produced task requires the latest run's submission, nonempty changed files, a linked passing test with an intact log artifact, a nonempty intact diff artifact, and criterion references to artifacts or passing tests from that same run. Agent submission cannot call owner acceptance.
- Request changes requires a reason and a closed run, returns the task to ready, clears previous criterion verification, preserves prior run evidence, and adds the reason to the next immutable packet. A new run starts from the approved source commit in a fresh checkout.

## Windows evidence

- [Preflight task packet](evidence/p14-packet.json) from the approved disposable addition task. It includes the exact scope, criterion, operations, stop conditions, and limits; the preflight run was cancelled without model inference.
- [Expected failing sample test](evidence/p14-sample-baseline.log) confirms the disposable task has real work to do.
- [Workspace tests](evidence/p14-test.log), [Clippy](evidence/p14-clippy.log), and [desktop/CLI build](evidence/p14-build.log) passed. Negative tests cover missing submission, tampered test log, evidence-linked acceptance, exact one-use model approval, active-run change rejection, feedback retention, and reset of old criterion verification.
- The installed OpenCode 1.18.30 password-protected server [launched with restrictive inline config](evidence/p14-opencode-no-model.log), reported the expected effective permissions, and stopped on drop without model inference.
- The desktop binary launched against the disposable fixture and remained running after three seconds. Interactive UI layout and full model-driven journey have not yet been recorded.

## Remaining verification

1. Obtain a new recorded, scoped approval for one external P14 model prompt. No paid fallback is authorized.
2. Run the disposable addition task on Windows through the desktop, observe permission decisions, inspect the patch and test log, request changes if needed, rerun under a fresh scoped approval if a second prompt is required, and record the owner review journey. P14 is not accepted until this passes.
3. Repeat the journey on the M1 Max before release, as required by the approved backlog.
