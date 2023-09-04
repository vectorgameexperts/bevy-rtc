# Silk

Silk is a monorepo for networking utilities and scheduling built over Bevy for our games.

## Notes

- A Bevy update and Silk scheduled update are **not** 1-to-1.
- Any network traffic (reading, writing) must be scheduled on the `SilkSchedule`.
- Do not put a rendering loop on the `SilkSchedule`, or you may see frame dropping.

## Versioning

| bevy  | bevy_matchbox |     silk    |
|-------|---------------|-------------|
| 0.11  | 0.7, main     | 0.7, main   |
| 0.10  | 0.6           | unsupported |
| < 0.9 | unsupported   | unsupported |

## Features

All features are opt-in.

```bash
cargo add --git ssh://git@github.com/vectorgameexperts/silk.git silk -F <features>
```

- `server` - Provides networking utilities for server applications
- `client` - Provides networking utilities for client applications
- `binary` - Sends networking packets as binary instead of JSON (default)

## Demos

- Server

```bash
cargo run -p demo-server
```

- Client (Native)

```bash
cargo run -p demo-client
```

- Client (Web)

```bash
cargo install wasm-server-runner
cargo run -p demo-client --target wasm32-unknown-unknown
```