# Modern Product Studio native feasibility spike

12 October 2026 · branch `codex/native-ux-feasibility` · Windows x86_64 · isolated `eframe`/`egui` 0.36.2 crate. This report records a feasibility experiment, not UX02–UX08 acceptance or a production change.

## Scope and audit

The approved browser prototype, UX00/UX01 audit and refinement, implementation handoff, product spec, architecture, agent contract, delivery plan, approved backlog, and progress record were reviewed before implementation. The current `pipeline-app` already uses egui 0.36.2 with AccessKit, a resizable portfolio and terminal, and a single UI shell. Its management tabs, evidence and terminal are retained. The native spike resides at [`spikes/modern-studio-native`](../../spikes/modern-studio-native/README.md) outside the production workspace and uses only illustrative, memory-only Northstar and Fieldnotes state.

The experiment covers portfolio focus, project overview and recommendation, compact composer, contextual assistant, project-scoped drafts/conversations and proposal previews, seven distinct work states, review summary and Advanced entry. It has no provider, scheduler, domain service, SQLite or terminal connection. Review acceptance controls are disabled. Submission does not alter verified completion.

## Windows rendering matrix

The Windows desktop has 125% OS display scaling. The arguments `--width 1152 --height 720` produced **1440 × 900 physical pixels**, and `--width 819 --height 720` produced **1024 × 900 physical pixels**. `--scale 1` and `--scale 2` below are egui zoom factors on top of that OS scale. Dimensions were read from the saved PNGs. Every listed image is a screenshot emitted by the running egui renderer, not a browser capture.

| Physical size / egui zoom | Result | Native screenshot |
| --- | --- | --- |
| 1440 × 900 / 100%, light | Portfolio focus and review summary readable; compact composer stays at bottom. | [Portfolio](../evidence/native-spike/portfolio-1440-light.png), [review](../evidence/native-spike/review-1440-light.png) |
| 1440 × 900 / 100%, dark | Fieldnotes overview and right assistant coexist. Work and progress cards stack so neither clips. | [Dark assistant](../evidence/native-spike/fieldnotes-1440-dark-assistant.png) |
| 1024 × 900 / 100%, light | Project rail collapses; project buttons remain visible. Assistant opens as a right anchored drawer. | [Narrow drawer](../evidence/native-spike/northstar-1024-light-drawer.png) |
| 1024 × 900 / 200%, light | Review paragraphs wrap inside the card; lower evidence requires vertical scroll. Composer and navigation remain visible. | [Scaled review](../evidence/native-spike/review-1024-zoom200.png) |
| 1440 × 900 / 200%, dark | Assistant becomes a drawer at the effective logical width; content remains accessible behind it after closing. | [Scaled dark drawer](../evidence/native-spike/fieldnotes-1440-zoom200.png) |

Additional 1440 px native state captures: [Pause requested](../evidence/native-spike/pause-requested-1440.png), [Paused](../evidence/native-spike/paused-1440.png), [Blocked](../evidence/native-spike/blocked-1440.png), and [Offline](../evidence/native-spike/offline-1440-dark.png). Ready, Working and Awaiting review appear in the matrix above. All seven states were rendered; their transitions were not backed by a scheduler.

The narrowest measured usable viewport was **1024 physical px at 200% egui zoom**. The review remains readable with vertical scrolling. This is a tested floor, not a universal minimum; narrower widths, long translated strings and OS scaling other than 125% need further checks. An initial capture showed horizontal clipping in the review and two-card overview; the final breakpoint, width and explicit wrap rules fixed those captured failures. The final screenshots above are after the fix.

## Interaction and accessibility checks

| Check | Result / evidence |
| --- | --- |
| Project switching and in-session draft/conversation isolation | Three unit tests pass, including project-scoped drafts and proposals. The assistant context label and compact composer change with selection. |
| Seven states | `Ready`, `Working`, `Pause requested`, `Paused`, `Blocked`, `Awaiting review`, `Offline` have distinct labels and explanations. The state selector permits local visual inspection. Pause transitions are illustrative; no run is changed. |
| Keyboard hooks | `Ctrl/Cmd+K` requests composer focus; Escape closes the assistant and requests focus on its toggle. A unit test covers the return request. Built-in egui buttons, tabs, combo box and text input remain keyboard focusable. End-to-end keyboard traversal and visible focus were **not manually verified** in this capture session. |
| AccessKit and screen reader | The crate enables egui's `accesskit` feature and uses built-in controls. No Windows screen-reader session or AccessKit tree inspection was run. Announcements, reading order, drawer semantics and dynamic conversation remain **unverified**. |
| Light/dark | Both themes captured. Dark text contrast was raised after visual inspection; formal contrast measurement remains open. |
| Apple Silicon | No Mac hardware session was available. Native M1 Max validation remains a release gate. |

Verification run on Windows: `cargo fmt --manifest-path spikes/modern-studio-native/Cargo.toml --check`, `cargo clippy --manifest-path spikes/modern-studio-native/Cargo.toml --all-targets -- -D warnings`, `cargo test --manifest-path spikes/modern-studio-native/Cargo.toml` (3 passed), `cargo build --manifest-path spikes/modern-studio-native/Cargo.toml`, and root `cargo fmt --all --check` passed. [Command log](../evidence/native-spike/windows-checks.log). Ten native screenshot commands exited successfully. The production crate was not modified.

## Design and technical limitations

- The native result proves that the shell can be composed in egui, while its default widgets still look plainer than the approved browser visual direction. Production styling needs a focused design-system pass with native typography, spacing, buttons, focus styling and contrast measurements.
- At 1024 px and 200% zoom, the card is readable but its lower content requires scrolling; keeping the composer fixed leaves less vertical room. The drawer covers part of the page until dismissed. These are acceptable spike behaviors to evaluate with users, not release-level accessibility evidence.
- The sample recommendations, statuses, checks and progress are fixtures. They cannot establish service correctness or approval-policy behavior. The proposed production recommendation must cite persisted records, and any proposal apply/review must use `ProjectEngine` with revision and policy enforcement.
- Assistant messages and drafts last only while the native process is open. Storage, export, retention and provider integration remain UX04 scope and require separate approval.
- Advanced shows an entry point and inventory of technical tools; it does not embed the production terminal or raw evidence. The production shell must preserve those existing views.

## Recommendation for owner decision

Retain the approved Rust/egui architecture for the first production stage. Start with a **grounded shell and deterministic next-action model** together: create domain-backed view models for portfolio focus, project home and typed destination, then implement the shell, compact composer placement, responsive rail/drawer and Advanced access around the existing production views. Keep the assistant read-only and fixture-free until UX04 is separately authorized. Before replacing the production interface, obtain owner approval of a scoped UX02/UX03 sequence and validate keyboard focus, AccessKit/screen-reader behavior, contrast and 200% zoom on Windows; repeat the native journey on the M1 Max before release. UX05–UX08 and P16–P19 retain their existing gates.
