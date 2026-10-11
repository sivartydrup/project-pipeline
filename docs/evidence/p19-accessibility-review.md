# P19 accessibility review — Windows code and visual evidence

Reviewed 11 October 2026. This is a code inspection against the approved specification, supplemented by existing Windows layout screenshots and terminal tests. It is not a screen-reader or high-DPI hands-on certification.

| Check | Result | Evidence / remaining action |
| --- | --- | --- |
| Keyboard region controls | Present | F1 toggles portfolio and F2 toggles terminal; both have labelled toolbar controls in `pipeline-app/src/main.rs`. |
| Text size and theme | Present in code | Toolbar offers 0.8×–2.0× zoom and light/dark themes, persisted in layout preferences. A real high-DPI visual check is pending. |
| Status conveyed by words | Present | Portfolio stage, health, completion, and blockers are text labels. Terminal exit and errors are textual. |
| Terminal keyboard input | Implemented, hands-on check pending | Custom terminal canvas takes focus on click and handles text/paste/keys. The P07 native PTY test exercises shell input; a full UI keyboard and screen-reader pass remains. |
| Screen-reader terminal output | Fallback added; hands-on check pending | Terminal output is painted on a custom egui canvas. A collapsible, selectable **Readable terminal transcript** now exposes the current visible screen through an egui label and offers a copy button. It cannot send process input. Confirm reading order and focus behavior with assistive technology before final sign-off. |
| Apple Silicon accessibility | Pending | Native M1 Max desktop and terminal checks require that machine. |

The existing [toolkit spike](../spike-results.md) identified the custom terminal as the main accessibility risk. The transcript fallback passed workspace format and Clippy checks and an app build; logs are [format](p19-transcript-fmt.log), [Clippy](p19-transcript-clippy.log), and [build](p19-transcript-build.log). A screen-reader pass remains on the P19 release gate.
