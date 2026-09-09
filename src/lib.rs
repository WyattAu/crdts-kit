//! # crdts-kit
//!
//! Conflict-free Replicated Data Types for collaborative applications.
//!
//! The centerpiece is an RGA (Replicated Growable Array) text CRDT:
//! [`RgaString`] maintains a replicated string with per-character identity,
//! tombstone-based deletion, and origin-based sibling ordering, so replicas
//! that have applied the same operation set converge on identical text.
//! [`CrdtDocument`] wraps it with document identity, a version counter, and
//! participant presence.
//!
//! Operations must be delivered causally (each operation after the
//! operations that created its origin characters — concurrent operations
//! may interleave freely); this is the standard RGA delivery model and is
//! what real transports provide.
//!
//! ## Example
//!
//! ```
//! use crdts_kit::text::RgaString;
//!
//! let mut alice = RgaString::new();
//! let mut bob = RgaString::new();
//!
//! let ops = alice.insert(1, 0, "Hello");
//! for op in &ops {
//!     bob.apply(op);
//! }
//!
//! let ops = bob.insert(2, 5, "!");
//! for op in &ops {
//!     alice.apply(op);
//! }
//!
//! assert_eq!(alice.text(), "Hello!");
//! assert_eq!(bob.text(), "Hello!");
//! ```
//!
//! ## Features
//!
//! - `std` (default) — standard library support
//! - `uuid` (default) — random `DocumentId::new()` via uuid v4; requires an
//!   application-configured `getrandom` backend on wasm32 targets, so wasm
//!   builds typically use `default-features = false` and construct
//!   `DocumentId`s from application-provided strings
//! - `serde` — Serialize/Deserialize for all state and operations
//! - `wasm` — documentation flag for `wasm32` target compatibility; the CRDT
//!   logic is pure and host-free

// Test code asserts invariants directly; unwrap/expect keeps failures loud.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod document;
pub mod text;

pub use document::{CrdtDocument, DocumentId, ParticipantId, ParticipantInfo};
pub use text::{OperationId, RgaString, TextOperation, TextOperationType};
