# Foundation spike results

Updated 8 October 2026. These are implementation observations, not release certification. MacBook Pro M1 Max validation remains pending until the machine is available.

## P01 — Rust desktop UI

- Prototype: `spikes/desktop` using `eframe`/`egui` 0.36.2 with its `accesskit` feature.
- Layout: resizable left portfolio panel, upper management workspace, resizable lower terminal region; F1/F2 toggles; dark/light toggle; text scaling.
- Large-list approach: virtualized 10,000-row project list through `ScrollArea::show_rows`.
- `cargo check --manifest-path spikes/desktop/Cargo.toml`: passed on Windows x86_64. Native executable launched; visual inspection confirmed portfolio, management tabs, and lower terminal area.
- A first visual pass found that nested egui panels did not reserve the intended lower area; the spike now uses an explicit split fraction. The terminal region is still a placeholder pending P07 integration.
- Keyboard focus is supplied by egui controls; F1/F2 explicitly toggle the side and terminal regions. Text scale is adjustable from 0.8× to 2.0×. The 10,000-row list is virtualized. `accesskit` is enabled, but a screen-reader pass and real high-DPI test are still required in P19; custom terminal rendering is the main accessibility risk.
- Recommendation: proceed with egui for implementation. Keep accessibility checks in the release gate and revisit the toolkit if those checks expose a fundamental blocker. Repeat the visual/PTY checks on the M1 Max before release.

## P02 — native PTY

- Prototype: `spikes/pty` using `portable-pty` 0.9.0 and `vt100` 0.16.2.
- `cargo test --manifest-path spikes/pty/Cargo.toml`: passed; parser unit test covers ANSI red and Unicode lambda.
- `cargo run --manifest-path spikes/pty/Cargo.toml`: passed with `cmd.exe`; repeated with `PIPELINE_USE_POWERSHELL=1` and passed with PowerShell. Both checks launch a ConPTY, send input, parse output, resize from 80×24 to 100×30, send `exit`, and verify child termination.
- Windows ConPTY sent a cursor-position query (`ESC[6n`) before shell output. The spike had to answer it (`ESC[1;1R`), an implementation requirement for the terminal host.
- A corrected PowerShell run produced actual ANSI and Unicode output; the probe avoids mistaking echoed command text for command output. Scrollback retention passes a parser test. Interactive UI rendering and M1 Max repeat belong to P07/P19.

## P03 — harness capability probe

| Capability | OpenCode 1.18.30 on Windows | Pi on Windows |
| --- | --- | --- |
| Executable/version | Present: `opencode --version` returned 1.18.30 | No global `pi`; temporary `npx` probe of `@earendil-works/pi-coding-agent` 1.1.0 succeeded |
| Local server/process mode | `opencode serve --hostname 127.0.0.1 --port 4099` started; loopback only | `pi --mode rpc --no-session` ran through `npx` |
| Health | `GET /global/health` returned healthy and version | `get_state` RPC command returned a successful response without inference |
| Session create/read/status | Temporary session created/read; status endpoint responded | Pending |
| Abort/delete | Temporary session aborted and deleted successfully | `new_session` and idle `abort` RPC responses succeeded |
| Event stream | `GET /global/event` yielded `server.connected`, `session.created`, and `session.deleted` SSE events in a live probe | Event stream documented; run events require a model run |
| Diff/recovery | Empty session diff returned zero items; session survived a server restart and was then deleted | `--no-session` is intentionally nonpersistent; persistent session recovery requires a later adapter test |
| Permissions | Endpoint and event semantics documented; a real permission request was not generated without an agent run | Extension/UI permission behavior documented; live request not generated |

The OpenCode test server was stopped after the probe. It warned that no server password was set; this is acceptable only for this loopback spike. The actual app must use its scoped bridge and adapter security policy. No model inference or paid provider call was made during either harness probe. Pi's modern npm package is under `@earendil-works`; the earlier `@mariozechner` package is deprecated. Live tool, permission, and diff event mapping remains an acceptance check for P13/P15.

## Phase 0 conclusion

P01, P02, and P03 have enough evidence to proceed to P04 on Windows. The macOS and full accessibility/permission tests remain explicit later gates. No release claim is made from these spikes.

References: [OpenCode server API](https://dev.opencode.ai/docs/server/), [Pi RPC protocol](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/rpc.md).
