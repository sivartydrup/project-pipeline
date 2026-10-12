# UX05 PR #6 Windows native interaction smoke

13 October 2026 · Windows · rebuilt `target/debug/pipeline-app.exe` · `PIPELINE_MODERN_SHELL=1` · isolated `target/ux05-review-native` portfolio. Two sample projects and an unapproved task/milestone plan were created through `ProjectEngine` with `cargo run -p pipeline-engine --example ux05_seed`. No provider was configured and no inference was sent. Native actions used the Computer Use UI Automation/SendInput interface against the single `Project Pipeline` window. The fixture's `--inspect` and `--advance-first-brief` commands use `ProjectEngine` services, not direct database writes.

| Interaction | Windows observation |
| --- | --- |
| Create manual proposal | Entered “A calm owner dashboard” in the native proposal editor and selected Review manual proposal. The card showed the exact original and proposed idea, target, project and revision. |
| Apply without confirmation | Apply was reported disabled by UI Automation. A click on its disabled location did not change the card or project revision. |
| Edit after preview | Replaced the text with “A different owner dashboard.” The validated card and Apply disappeared in the same UI update; the panel requested a new preview. |
| Switch proposal target | Previewed the brief, then selected `Task · first-task`. The old card disappeared, the editor cleared, and no confirmation carried over. |
| Confirm and apply | Previewed `First task` to `Refined first task`. Starting at Ctrl+K, Tab moved through composer, provider, target, editor, review and evidence to the confirmation checkbox; Space enabled Apply; Tab focused Apply; Enter saved plan draft revision 2. UI displayed the new task title and stated that separate plan approval remains required. |
| Project isolation | Previewed another task edit, confirmed it, switched to `Second sample`, then back. Apply was disabled when returning to the first project. The second project showed no first-project editor text. |
| Discard | Selected Cancel / Discard on the unconfirmed task preview. The card disappeared. Service inspection still showed plan revision 2, task title `Refined first task`, and activity count 4; no discard event was added. |
| Stale proposal | Previewed a brief edit; a separate `ProjectEngine::save_brief` advanced the brief from revision 1 to 2. On the next UI update the card cleared with a stale message and no Apply. Repeated on the rebuilt 500 ms polling version from revision 2 to 3: the card cleared automatically without another UI action. |

Service inspection after the first allowed Apply and later Discard: `Sample product: brief_revision=1, plan_revision=2, task_title="Refined first task", activity_count=4`; `Second sample: brief_revision=1, plan_revision=0, activity_count=2`. After the two deliberate external brief saves, `Sample product` had brief revision 3 and activity count 6. The only new events in those latter checks came from the explicit fixture saves; stale preview itself created none.

The keyboard path through confirmation and Apply was completed on Windows, including checkbox focus, Space, Tab and Enter. This does not verify every keyboard path at 200% scaling or screen-reader announcements. Apple Silicon and screen-reader validation remain later release gates. The prior static captures show 1024/1440 and 100/200% layouts; static captures alone were not used to claim interactive success.
