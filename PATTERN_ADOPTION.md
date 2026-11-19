# PATTERN_ADOPTION

crdts-kit declares its relationship to the WyattAu pattern canon
(pattern-standards). Format: references/adoption-format.md; validated by
scripts/adoption-check.py.

| Pattern | Status | Evidence |
|---|---|---|
| T23 | partial | src/text.rs:99 (RgaString RGA text CRDT with per-character identity and tombstones; counters and sets are in scope per REQUIREMENTS.md:6 but not yet implemented) |
| T36 | n/a | no I/O seams to simulate in a pure library; exhaustive model checking covers concurrent-insert convergence instead (tests/kani.rs:92) |
| P64 | adopted | tests/proptest.rs:118 (convergence under arbitrary delivery orders plus tombstone resurrection, 512 cases per run; nightly enlarged soak with PROPTEST_CASES=4096 per the canon adoption format not yet scheduled, target 2027-Q1) |
| X5 | adopted | src/text.rs:162 (insert, delete, apply are the only mutators and apply is the merge; convergence holds by algebra, not post-hoc validation; the canon names CRDT semilattices a proven X5 instance) |
