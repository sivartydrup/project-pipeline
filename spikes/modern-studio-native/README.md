# Modern Product Studio native feasibility spike

This executable is an isolated egui scenario. Northstar, Fieldnotes, checks, approvals, conversations, and work states are illustrative memory-only data. It never opens the production database, starts an agent, or calls a provider.

From the repository root:

```text
cargo run --manifest-path spikes/modern-studio-native/Cargo.toml
```

Optional arguments: `--width 1024 --height 900 --scale 2 --dark --project fieldnotes --screen review --state paused`. Width and height are egui logical window points; physical pixels also depend on the OS display scale. `--scale` is egui zoom. Add `--assistant` to start with the panel open, or `--capture path.png` to save a native window screenshot and exit. For a fresh scenario, restart the executable. Use the project buttons to switch, the screen buttons to navigate, the status selector to inspect seven distinct states, the theme button to switch light/dark, and Advanced to inspect the existing technical workspace concept. `Ctrl+K` or `Cmd+K` focuses the compact composer. Escape closes the assistant and returns focus to its toggle. A narrow window opens the assistant as a drawer.

Tests and compile check:

```text
cargo test --manifest-path spikes/modern-studio-native/Cargo.toml
cargo clippy --manifest-path spikes/modern-studio-native/Cargo.toml --all-targets -- -D warnings
```

See [native spike results](../../docs/ux-discovery/native-spike-results.md) for the validation matrix and limits.
