# Requirements — crdts-kit

Numbered, testable requirements. Every requirement maps to at least one named
test or doc-comment contract; security-relevant items cite THREAT-MODEL.md rows.

Scope: Conflict-free replicated data types — `RgaString`, counters, sets with merge semantics

## Functional

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-CR-001 | Merge is commutative, associative, idempotent for every CRDT (converge regardless of delivery order/duplication) | MUST |
| REQ-CR-002 | `RgaString` supports concurrent insert/delete with intent preservation (interleaving bounds documented) | MUST |
| REQ-CR-003 | Serialization roundtrips preserve state exactly (fuzz/proptest coverage) | MUST |

## Security

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-CR-100 | Hostile/remote deltas cannot corrupt internal invariants: deserialization validates structure before merge | MUST |
| REQ-CR-101 | Untrusted replicas cannot grow state unboundedly via merge (bounded-element policies documented) | SHOULD |

## Observability & API hygiene

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-CR-900 | All fallible public APIs return typed errors; production `unwrap`/`expect` is denied or explicitly justified with an invariant comment | MUST |
| REQ-CR-901 | Public items carry doc comments with runnable examples where practical | SHOULD |

Reviewed: 2026-09-11
