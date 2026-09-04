# crdts-kit

CRDT toolkit for Rust — an RGA (Replicated Growable Array) replicated string with tombstones, participant presence, and convergence properties suitable for real-time collaborative editing.

```toml
[dependencies]
crdts-kit = "0.1"
```

```rust
use crdts_kit::text::RgaString;

let mut doc = RgaString::new();
let ops = doc.insert(1, 0, "Hello");

// Ship `ops` to other replicas; they apply them in any order
// and converge on the same text.
for op in &ops {
    remote.apply(op);
}
```

## What's inside

| Module | Purpose |
|---|---|
| `text` | `RgaString` — RGA replicated string with per-character `OperationId {site_id, counter}`, origin-left/right sibling ordering, and tombstone-based deletion |
| `document` | `CrdtDocument` — versioned document wrapper with `DocumentId`, participants (`ParticipantId` + name + presence), and operation production/ingestion |

## Features

- Default: `std`, `uuid`
- `uuid` — random `DocumentId::new()` via uuid v4 (uses `getrandom`, which needs an application-configured backend on wasm32; wasm builds usually opt out with `default-features = false` and build ids from application-provided strings)
- `serde` — Serialize/Deserialize for all CRDT state and operations (JSON-friendly)
- `wasm` — documentation flag for `wasm32` target compatibility; the CRDT logic is pure and host-free. For wasm builds:

```toml
crdts-kit = { version = "0.1", default-features = false, features = ["std", "serde", "wasm"] }
```

## Guarantees

- **Convergence** — replicas that have applied the same operation set end with identical text, regardless of how *concurrent* operations interleave (property-tested with `proptest`). Operations must be delivered causally — each operation after the operations that created its origin characters — which is the standard RGA delivery model and what real transports (WebSocket sessions, sync protocols) provide.
- **Tombstones never resurrect** — deleted characters stay deleted even when concurrent inserts target the deleted position
- **Commutativity** — concurrent operations commute; `OperationId` total ordering resolves concurrent inserts at the same origin deterministically

## Safety

`#![forbid(unsafe_code)]`.

## License

MIT OR Apache-2.0
