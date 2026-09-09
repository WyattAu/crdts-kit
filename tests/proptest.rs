// Property tests assert invariants directly; unwraps keep failures loud.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Property-based tests for CRDT convergence guarantees.
//!
//! These complement the unit tests by exercising random operation sequences
//! across multiple replicas with concurrent delivery permutations — the
//! scenarios hand-written tests cannot cover exhaustively.

use std::collections::HashSet;

use proptest::prelude::*;

use crdts_kit::document::{CrdtDocument, DocumentId, ParticipantId};
use crdts_kit::text::{OperationId, RgaString, TextOperation};

/// Max visible length a replica is expected to reach; insert positions are
/// generated below this bound (positions past the end are clamped by the
/// RGA's origin lookup, which is also exercised deliberately).
const MAX_POS: usize = 24;

/// A single random edit command: which site performs what.
#[derive(Debug, Clone)]
enum Cmd {
    Insert { pos: usize, ch: char },
    Delete { pos: usize, len: usize },
}

fn cmd_strategy(num_sites: u32) -> impl Strategy<Value = (u32, Cmd)> {
    let site = 0..num_sites;
    let insert = (site.clone(), 0..MAX_POS, "[A-Z]").prop_map(|(site, pos, s)| {
        (
            site,
            Cmd::Insert {
                pos,
                ch: s.chars().next().unwrap(),
            },
        )
    });
    let delete =
        (site, 0..MAX_POS, 1usize..4).prop_map(|(site, pos, len)| (site, Cmd::Delete { pos, len }));
    prop_oneof![3 => insert, 2 => delete]
}

/// Extracts the authoring site of an operation.
fn op_site(op: &TextOperation) -> u32 {
    match op {
        TextOperation::Insert { id, .. } | TextOperation::Delete { id, .. } => id.site_id,
    }
}

/// An insert's character id, used for causal-readiness checks.
fn insert_id(op: &TextOperation) -> Option<&OperationId> {
    match op {
        TextOperation::Insert { id, .. } => Some(id),
        TextOperation::Delete { .. } => None,
    }
}

/// True when all characters an operation references are already present,
/// i.e. the op may be applied next under causal delivery.
fn is_ready(op: &TextOperation, present: &HashSet<OperationId>) -> bool {
    match op {
        TextOperation::Insert {
            origin_left,
            origin_right,
            ..
        } => {
            origin_left.as_ref().is_none_or(|o| present.contains(o))
                && origin_right.as_ref().is_none_or(|o| present.contains(o))
        }
        TextOperation::Delete { target, .. } => present.contains(target),
    }
}

/// Delivers every operation authored by other sites to `replica` in an
/// arbitrary causally-valid order: at each step, the ready (origin-satisfied)
/// op with the smallest key from `keys` goes next, so different key vectors
/// produce different concurrent interleavings.
fn deliver_causally(replica: &mut RgaString, site: u32, network: &[TextOperation], keys: &[u64]) {
    let mut present: HashSet<OperationId> = HashSet::new();
    for op in network {
        if op_site(op) == site {
            if let Some(id) = insert_id(op) {
                present.insert(id.clone());
            }
        }
    }

    let mut delivered = vec![false; network.len()];
    loop {
        let next = network
            .iter()
            .enumerate()
            .filter(|(i, op)| !delivered[*i] && op_site(op) != site && is_ready(op, &present))
            .min_by_key(|(i, _)| keys.get(*i).copied().unwrap_or(u64::MAX))
            .map(|(i, _)| i);
        match next {
            Some(i) => {
                replica.apply(&network[i]);
                delivered[i] = true;
                if let Some(id) = insert_id(&network[i]) {
                    present.insert(id.clone());
                }
            }
            None => break,
        }
    }
    debug_assert!(
        network
            .iter()
            .zip(&delivered)
            .all(|(op, &d)| d || op_site(op) == site),
        "causal deadlock: unsatisfiable origins"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// RGA convergence: replicas that generate random, interleaved, concurrent
    /// edits and then receive every operation in different orders must all end
    /// with identical text.
    #[test]
    fn convergence_under_arbitrary_delivery_orders(
        cmds in proptest::collection::vec(cmd_strategy(3), 1..40),
        order_keys in proptest::collection::vec(
            proptest::collection::vec(any::<u64>(), 0..120),
            3,
        ),
    ) {
        // Each site keeps its own replica; local ops apply immediately and are
        // broadcast to the network.
        let mut replicas: Vec<RgaString> = (0..3).map(|_| RgaString::new()).collect();
        let mut network: Vec<TextOperation> = Vec::new();

        for (site, cmd) in cmds {
            let replica = &mut replicas[site as usize];
            let ops = match cmd {
                Cmd::Insert { pos, ch } => {
                    let text = ch.to_string();
                    replica.insert(site + 1, pos, &text)
                }
                Cmd::Delete { pos, len } => replica.delete(site + 1, pos, len),
            };
            network.extend(ops);
        }

        // Deliver every remote operation to every replica in a per-replica
        // causally-valid order (concurrent ops permute arbitrarily; an op
        // never overtakes the operations that created its origin characters).
        for (idx, replica) in replicas.iter_mut().enumerate() {
            let site = idx as u32 + 1;
            deliver_causally(replica, site, &network, &order_keys[idx]);
        }

        // Convergence: identical visible text everywhere.
        let texts: Vec<String> = replicas.iter().map(|r| r.text()).collect();
        for (i, text) in texts.iter().enumerate().skip(1) {
            prop_assert_eq!(&texts[0], text, "replica {} diverged from replica 0", i);
        }
    }

    /// A tombstoned character never resurrects: a concurrent insert targeting
    /// the deleted position must not bring the deleted character back, and
    /// both replicas must agree on the outcome.
    #[test]
    fn tombstone_never_resurrects(
        chars in proptest::collection::vec(
            proptest::sample::select(
                "abcdefghijklmnopqrstuvwxyz0123456789".chars().collect::<Vec<_>>(),
            ),
            4..16,
        )
        .prop_filter("chars must be distinct", |v| {
            let set: HashSet<&char> = v.iter().collect();
            set.len() == v.len()
        })
        .prop_flat_map(|chars| (Just(chars.clone()), 0..chars.len())),
    ) {
        let (chars, del_pos) = chars;
        let initial: String = chars.iter().collect();
        let len = initial.chars().count();
        let deleted_char = initial.chars().nth(del_pos).unwrap();

        let mut a = CrdtDocument::new(DocumentId("prop-a".to_string()));
        a.join(ParticipantId(1), "A");
        let (init_ops, _) = a.insert_text(ParticipantId(1), 0, &initial);

        let mut b = CrdtDocument::new(DocumentId("prop-b".to_string()));
        b.join(ParticipantId(2), "B");
        b.apply_ops(&init_ops);
        prop_assert_eq!(b.get_text(), initial);

        // Concurrent edits: A deletes at del_pos while B (unaware) inserts
        // at the same position.
        let (del_ops, _) = a.delete_text(ParticipantId(1), del_pos, 1);
        let (ins_ops, _) = b.insert_text(ParticipantId(2), del_pos, "Z");

        a.apply_ops(&ins_ops);
        b.apply_ops(&del_ops);

        let a_text = a.get_text();
        let b_text = b.get_text();
        prop_assert_eq!(&a_text, &b_text, "replicas diverged after concurrent delete+insert");
        prop_assert!(!a_text.contains(deleted_char), "deleted char {:?} resurrected", deleted_char);
        prop_assert!(a_text.contains('Z'), "concurrent insert at deleted position was lost");
        prop_assert_eq!(a_text.chars().count(), len, "expected exactly one delete and one insert");
    }
}

/// Serde roundtrip: a serialized/deserialized replica must remain equivalent
/// and continue evolving identically to the original.
#[cfg(feature = "serde")]
mod roundtrip {
    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn serde_roundtrip_preserves_future_convergence(
            pre in proptest::collection::vec(cmd_strategy(1), 1..30),
            post in proptest::collection::vec(cmd_strategy(1), 0..30),
        ) {
            let mut original = RgaString::new();
            for (site, cmd) in pre {
                match cmd {
                    Cmd::Insert { pos, ch } => {
                        original.insert(site + 1, pos, &ch.to_string());
                    }
                    Cmd::Delete { pos, len } => {
                        original.delete(site + 1, pos, len);
                    }
                }
            }

            let json = serde_json::to_string(&original).unwrap();
            let mut restored: RgaString = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(original.text(), restored.text(), "state diverged after roundtrip");

            // Continue editing both replicas identically; they must stay equal.
            for (site, cmd) in post {
                let ops = match cmd {
                    Cmd::Insert { pos, ch } => {
                        original.insert(site + 1, pos, &ch.to_string())
                    }
                    Cmd::Delete { pos, len } => original.delete(site + 1, pos, len),
                };
                for op in &ops {
                    restored.apply(op);
                }
            }

            prop_assert_eq!(original.text(), restored.text(), "post-roundtrip evolution diverged");
        }
    }
}
