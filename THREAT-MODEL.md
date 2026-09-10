# Threat Model — crdts-kit

Reference: STRIDE. Scope: the crate's public API surface (`RgaString`,
`TextOperation`, `apply`, `CrdtDocument`, serde operation serialization) as
used by a collaborative application. Trust boundaries: (1) operations
received from other replicas (`apply`, deserialized via serde), (2) document
identity/participant identity claims, (3) the dependency tree (serde, uuid).

The CRDT core guarantees **convergence**, not authenticity: it is designed
to survive arbitrary delivery orders of *honest* operations, not malicious
ones.

## Assets

| ID | Asset | Example |
|----|-------|---------|
| A1 | Convergence (all replicas reach identical text) | A crafted operation sequence making replicas diverge permanently |
| A2 | Availability of a replica | A flood of garbage operations exhausting memory (tombstones never GC) |
| A3 | Integrity of authorship attribution | An operation spoofing another participant's `ParticipantId` |

## STRIDE Analysis

| # | Threat | Category | Surface | Mitigation | Verifying test |
|---|--------|----------|---------|------------|----------------|
| T1 | Out-of-order / concurrent delivery breaks convergence | Tampering | `RgaString::apply` | RGA design: origin-based sibling ordering + tombstones make convergence order-independent for causally-delivered ops; Kani proof checks *all* permutations of two concurrent inserts | `convergence_under_arbitrary_delivery_orders` (`tests/proptest.rs`), `kani_same_position_concurrent_inserts_converge_over_all_permutations` (`tests/kani.rs`), `test_apply_operations_from_multiple_sites`, `shared_origin` |
| T2 | Forged operation attributed to another participant | Spoofing | `TextOperation` site/participant IDs | **Not mitigated** — participant identity is a plain `ParticipantId` carried in the operation; any replica can claim any ID. Documented residual risk: authenticate ops at the transport | Code review; no signature/auth field exists on operations (API surface audit) |
| T3 | Tombstone/insert flood exhausting memory | DoS | `apply`, `insert` | **Not mitigated** — no operation-size cap, no replica-level rate limit, and deleted characters persist as tombstones indefinitely (no GC). Documented residual risk | `tombstone_never_resurrects` (correctness, not bounds) |
| T4 | Duplicate/replayed operation corrupting state | Replay | `apply` | Idempotent by identity: re-applying an operation with an existing `OperationId` is a no-op (identity-checked apply), so transport-level duplicates and replays are harmless to state | `test_apply_remote_ops`, `test_tombstone_behavior`; convergence proptest covers arbitrary redelivery orders |
| T5 | serde roundtrip losing convergence (schema drift) | Tampering | serde operation format | Roundtrip property asserts deserialized state converges identically to the original — format drift cannot silently fork history | `serde_roundtrip_preserves_future_convergence` (`tests/proptest.rs`) |
| T6 | Malformed operation bytes panicking deserialization | DoS | serde `TextOperation` input | Errors are `Result` (serde); `#![forbid(unsafe_code)]`; no `unsafe` reinterpretation of untrusted bytes | `serde_roundtrip_preserves_future_convergence` (valid path); malformed-input behavior inherited from serde — no dedicated fuzz target (documented gap) |

## Repudiation

Not supported: with forgeable participant IDs (T2) there is no
non-repudiable attribution of edits. The document's version counter tracks
state, not actors. Accepted — attribution requires transport-level identity.

## Out of Scope

- Transport security and causal-delivery guarantees: the crate requires the
  standard RGA delivery model (ops after their origins' ops); real
  transports provide this. Malicious reordering that violates causality is
  out of model.
- Document access control: `DocumentId` possession is not a capability.
- Offline tombstone GC / history compaction (not implemented).

## Residual Risks

- **R1 (Medium, accepted):** Unauthenticated operations (T2). The CRDT will
  faithfully converge on attacker-authored edits; every participant with
  transport access can mutate any document. Mitigate with authenticated
  channels and app-level authorization before `apply`.
- **R2 (Medium, accepted):** Unbounded memory growth: tombstones plus
  unbounded insert sizes mean a hostile peer can balloon replica memory with
  valid-shaped operations (T3). Cap operation size and rate at ingress.
- **R3 (Low, accepted):** No fuzz target for deserialization of hostile
  bytes (T6); coverage relies on serde's test record. Adding `cargo-fuzz`
  for `TextOperation` is cheap future hardening.
- **R4 (Low, accepted):** `DocumentId::new()` (uuid feature) depends on the
  host RNG; wasm builds without a `getrandom` backend must supply IDs
  application-side (documented in crate docs).
