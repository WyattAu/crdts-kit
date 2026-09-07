#![cfg(kani)]
//! Kani bounded model-checking harness for RGA text convergence
//! (`crdts_kit::text::RgaString`).
//!
//! # Property
//!
//! Three *concurrent* inserts at the **same position** (all three share
//! the same `origin_left` — after base character `"A"` of the shared base
//! text `"AB"`), with arbitrary characters from a small alphabet and
//! arbitrary distinct site ids, are applied to fresh replicas in every
//! one of the **6 permutations** of their delivery order. All six
//! resulting texts must be identical.
//!
//! Same-position concurrent inserts are the worst case for RGA: the
//! integration rule (`find_insert_position` walks past every node with a
//! greater id, stopping at the first smaller one) must resolve the
//! conflict purely from the total order on [`OperationId`]. If that
//! tie-break were delivery-order-dependent, two replicas applying the
//! same three ops in different orders would diverge — which is exactly
//! what this harness rules out.
//!
//! # Bounding / causal delivery
//!
//! - Site ids: `site_id ∈ 2..=5`, pairwise distinct. Bounded because
//!   site ids are the symbolic input to every id comparison: a full-range
//!   `u32` per insert (96 symbolic comparison bits across three inserts)
//!   makes the CBMC formula intractable, while a 4-value domain keeps all
//!   6 relative orderings of the three ids representable. With distinct
//!   sites the `(site_id, counter)` ids are unique regardless of counters
//!   (unique ids are an RGA precondition), so the per-site counters are
//!   pinned to 1 — `OperationId`'s `Ord` compares `site_id` first, and
//!   counter-ordering only matters for sequential same-site ops.
//! - Characters ∈ {`"x"`, `"y"`}: the character value never influences
//!   merge order (ids do), so the alphabet is trimmed purely to keep
//!   symbolic execution over `String` cheap.
//! - Base text `"AB"` (ids `(1,1)`, `(1,2)`) is concrete and applied
//!   first in every replica, so every one of the 6 delivery orders is a
//!   causally-legal application order (see the causal-delivery contract
//!   on `RgaString::apply`).
//!
//! Run with:
//! ```text
//! cargo kani --tests
//! ```

use crdts_kit::text::{OperationId, RgaString, TextOperation};

/// Map an arbitrary byte to a small fixed one-char string (2-char
/// alphabet).
fn pick_char(b: u8) -> &'static str {
    match b % 2 {
        0 => "x",
        _ => "y",
    }
}

/// The shared insert position: immediately after base character `"A"`.
fn shared_origin() -> Option<OperationId> {
    Some(OperationId {
        site_id: 1,
        counter: 1,
    })
}

/// A fresh replica that applies the base, then the three inserts in the
/// given delivery order, and renders its visible text.
fn delivered_text(
    base: &[TextOperation],
    first: &[TextOperation],
    second: &[TextOperation],
    third: &[TextOperation],
) -> String {
    let mut rga = RgaString::new();
    for op in base {
        rga.apply(op);
    }
    for op in first {
        rga.apply(op);
    }
    for op in second {
        rga.apply(op);
    }
    for op in third {
        rga.apply(op);
    }
    rga.text()
}

#[kani::proof]
#[kani::unwind(10)]
#[kani::solver(kissat)]
fn kani_same_position_concurrent_inserts_converge_over_all_permutations() {
    // Concrete shared base: "AB".
    let mut base_rga = RgaString::new();
    let base_ops = base_rga.insert(1, 0, "AB");

    // Nondeterministic, pairwise-distinct site ids (bounded to 2..=5 so
    // no site collides with the base's site 1).
    let site1 = u32::from(kani::any::<u8>() % 4 + 2);
    let site2 = u32::from(kani::any::<u8>() % 4 + 2);
    let site3 = u32::from(kani::any::<u8>() % 4 + 2);
    kani::assume(site1 != site2);
    kani::assume(site1 != site3);
    kani::assume(site2 != site3);

    let mk = |site_id: u32, char_byte: u8| TextOperation::Insert {
        id: OperationId {
            site_id,
            counter: 1,
        },
        position: 1,
        content: pick_char(char_byte).to_string(),
        origin_left: shared_origin(),
        origin_right: None,
    };
    let op1 = mk(site1, kani::any::<u8>());
    let op2 = mk(site2, kani::any::<u8>());
    let op3 = mk(site3, kani::any::<u8>());

    // All 6 permutations of {op1, op2, op3}, enumerated explicitly.
    let t1 = delivered_text(&base_ops, &[op1.clone()], &[op2.clone()], &[op3.clone()]);
    let t2 = delivered_text(&base_ops, &[op1.clone()], &[op3.clone()], &[op2.clone()]);
    let t3 = delivered_text(&base_ops, &[op2.clone()], &[op1.clone()], &[op3.clone()]);
    let t4 = delivered_text(&base_ops, &[op2.clone()], &[op3.clone()], &[op1.clone()]);
    let t5 = delivered_text(&base_ops, &[op3.clone()], &[op1.clone()], &[op2.clone()]);
    let t6 = delivered_text(&base_ops, &[op3.clone()], &[op2.clone()], &[op1.clone()]);

    kani::assert(
        t1 == t2 && t1 == t3 && t1 == t4 && t1 == t5 && t1 == t6,
        "all 6 delivery permutations of 3 concurrent same-position inserts converge",
    );
}
