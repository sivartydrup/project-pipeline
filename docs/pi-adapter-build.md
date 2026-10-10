# P15 Pi RPC adapter — work in progress

Started 10 October 2026 after [owner acceptance of P14](approval-p14-2026-10-10.md). P15 is not yet verified or accepted.

The first adapter slice adds byte-level LF-delimited JSON framing and request-ID response correlation. Fixtures cover partial reads splitting a UTF-8 character, Unicode line and paragraph separators within JSON strings, CRLF, truncated/invalid records, interleaved events, out-of-order responses, and response command mismatch. The [fixture log](evidence/p15-rpc-fixtures.log) and `cargo fmt --all -- --check` pass on Windows.

An installed global `pi` command is unavailable. A no-inference probe of `@earendil-works/pi-coding-agent` 1.1.0 via `npx` confirmed the RPC `get_state` response, using disabled tools, extensions, MCP, skills, context files, and offline mode. Its selected default model was paid, so P15 must require an explicit provider/model and a scoped one-use prompt approval before any model dispatch. This probe did not send a prompt.

Next work: owned subprocess lifecycle, version/capability probe, normalized event mapping, persistent session recovery, a fail-closed tool policy integrated with the application action service, run worker and review UI integration, and the approved Windows sample task. The M1 Max task journey remains a release check.

Protocol references: [Pi RPC mode](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/rpc.md), [commands](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/rpc-commands.md), and [event stream](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/json.md).
