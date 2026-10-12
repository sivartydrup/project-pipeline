# PR #4 review refinement · Windows native smoke

12 October 2026 · Windows console session · `pipeline-app.exe` built from this branch · isolated sample portfolio under ignored `target/ux-review-smoke`. Alpha and Beta were created through the application's project setup. The native window was inspected with Windows UI Automation/AccessKit and screenshots from the running egui renderer. These observations apply to Windows only.

| Check | Observed result |
| --- | --- |
| Project switching and drafts | Typed `Beta private draft` in the compact composer. The expanded Guidance editor displayed the same text; editing it to `Beta private draft revised` updated the compact composer. Switching to Alpha displayed an empty draft. Typing `Alpha draft`, opening Home, and typing `Portfolio draft` kept those contexts separate. Returning to Beta restored `Beta private draft revised`. |
| Drawer and Escape | Guidance opened with `Context: Beta` and the draft. Escape closed it; AccessKit reported focus on the header Guidance button. Reopening preserved the draft. |
| Keyboard | Tab moved focus from Guidance to `A−`. Initial Ctrl+K attempts did not focus the composer. After changing the shortcut to inspect modifiers on the key event, Ctrl+K from the window focused the compact editor (`edit` in AccessKit). |
| Advanced and terminal | Advanced exposed the existing portfolio controls and all eight management tabs. `+ New shell` created `Shell 1`; the visible PowerShell prompt was in Beta's project folder. No commands were entered through UI automation. Existing automated PTY parser, working-directory, output, and close tests passed in the workspace suite. |
| Default shell | With `PIPELINE_MODERN_SHELL` unset, the app opened the prior portfolio/management/terminal interface. |
| Light, dark and scaling | Both themes were inspected in the native window. [Dark 1024 × 900 at 100%](ux02-ux03-review-narrow-100-dark.png) and [light 1024 × 900 at 200%](ux02-ux03-review-narrow-200-light.png) were captured by egui. The 200% drawer and editor were readable; the drawer overlays page content until closed, and content below the card requires scrolling. |
| Basic contrast | Calculated sRGB ratios for selected tokens: light body text/card 13.33:1, dark body text/card 12.86:1, light brand/panel 4.81:1, dark brand/panel 7.48:1. The dark brand and hover fills were adjusted after inspection. These sampled ratios are not a full contrast audit of every widget state. |
| AccessKit | Buttons, project switcher, composer, expanded editor, tabs and status text appeared in the accessibility tree. Focus changes for Tab, Escape and Ctrl+K were observed. Reading order appeared logical in the sampled tree, but a screen reader was not run; spoken announcements and dynamic update behavior remain unverified. |

No M1 Max was available. Narrower than 1024 physical pixels, other OS scaling values, long translated strings, a complete focus order, a full contrast inventory and screen-reader announcements remain open verification items. The native session did not exercise a live AI provider or any consequential action.
