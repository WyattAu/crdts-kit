//! Replicated Growable Array (RGA) text CRDT.
//!
//! [`RgaString`] maintains a sequence of per-character nodes, each tagged with
//! a unique [`OperationId`]. Deletions are tombstones (nodes are retained but
//! hidden), and concurrent inserts at the same origin are ordered by
//! [`OperationId`], so every replica converges on the same text — replicas
//! must receive operations in causal order (an operation after the operations
//! that created its origin characters), which is the standard RGA delivery
//! model.

use std::fmt;

/// Unique identifier for a single character operation.
///
/// The pair `(site_id, counter)` is totally ordered and forms the
/// tie-breaker for concurrent inserts sharing the same origin.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OperationId {
    /// Site (replica) that created the operation.
    pub site_id: u32,
    /// Per-site logical clock value at creation time.
    pub counter: u64,
}

/// The kind of a [`TextOperation`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TextOperationType {
    /// Inserts a character at a position.
    Insert,
    /// Tombstones the character targeted by `id`.
    Delete,
}

/// A single insert or delete operation on an [`RgaString`].
///
/// Operations are commutative and idempotent-free by design: applying the
/// same set of operations to replicas in different orders yields the same
/// final text.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TextOperation {
    /// Insert one character between `origin_left` and `origin_right`.
    Insert {
        /// Unique id assigned to the inserted character.
        id: OperationId,
        /// Position the character occupied in the authoring replica's view.
        position: usize,
        /// The inserted character (always a single `char`).
        content: String,
        /// Id of the character this insert goes after (`None` = start).
        origin_left: Option<OperationId>,
        /// Id of the character this insert goes before (`None` = end).
        origin_right: Option<OperationId>,
    },
    /// Tombstone the character identified by `target`.
    Delete {
        /// Unique id of the delete operation itself.
        id: OperationId,
        /// Id of the character being deleted.
        target: OperationId,
    },
}

impl TextOperation {
    /// Returns whether this is an [`TextOperationType::Insert`] or
    /// [`TextOperationType::Delete`] operation.
    pub fn op_type(&self) -> TextOperationType {
        match self {
            TextOperation::Insert { .. } => TextOperationType::Insert,
            TextOperation::Delete { .. } => TextOperationType::Delete,
        }
    }
}

/// Internal sequence node: one character plus its RGA metadata.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct RgaChar {
    char: char,
    id: OperationId,
    #[cfg_attr(not(feature = "serde"), allow(dead_code))]
    origin_left: Option<OperationId>,
    #[cfg_attr(not(feature = "serde"), allow(dead_code))]
    origin_right: Option<OperationId>,
    tombstone: bool,
}

/// A replicated string backed by an RGA (Replicated Growable Array).
///
/// Local edits are made through [`insert`](RgaString::insert) and
/// [`delete`](RgaString::delete), which return the operations to broadcast.
/// Remote edits are ingested with [`apply`](RgaString::apply) in any order;
/// replicas that have applied the same operation set always render the same
/// [`text`](RgaString::text).
#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RgaString {
    nodes: Vec<RgaChar>,
    clock: u64,
}

impl RgaString {
    /// Creates an empty replicated string.
    pub fn new() -> Self {
        RgaString {
            nodes: Vec::new(),
            clock: 0,
        }
    }

    fn next_id(&mut self, site_id: u32) -> OperationId {
        self.clock += 1;
        OperationId {
            site_id,
            counter: self.clock,
        }
    }

    fn visible_id_at(&self, mut index: usize) -> Option<OperationId> {
        for node in &self.nodes {
            if !node.tombstone {
                if index == 0 {
                    return Some(node.id.clone());
                }
                index -= 1;
            }
        }
        None
    }

    /// Computes the vector index at which the character `id` is inserted.
    ///
    /// Uses the canonical RGA integration: start after `origin_left` (or at
    /// the beginning when `None`), then skip every node whose id is greater
    /// than `id`, stopping at the first node with a smaller id. Skipping by
    /// total id order (rather than same-origin comparisons) makes the result
    /// independent of the order in which concurrent operations arrived.
    fn find_insert_position(&self, id: &OperationId, origin_left: &Option<OperationId>) -> usize {
        let mut pos = 0;

        if let Some(left) = origin_left {
            pos = match self.nodes.iter().position(|node| node.id == *left) {
                Some(i) => i + 1,
                None => 0,
            };
        }

        while pos < self.nodes.len() && self.nodes[pos].id > *id {
            pos += 1;
        }

        pos
    }

    /// Inserts `content` at visible character `index` on behalf of `site_id`.
    ///
    /// Returns one operation per inserted character, already applied locally.
    /// Broadcast these to other replicas, which ingest them with
    /// [`apply`](RgaString::apply).
    pub fn insert(&mut self, site_id: u32, index: usize, content: &str) -> Vec<TextOperation> {
        if content.is_empty() {
            return Vec::new();
        }

        let left_id = if index == 0 {
            None
        } else {
            self.visible_id_at(index - 1)
        };
        let right_id = self.visible_id_at(index);

        let mut ops = Vec::new();
        let mut prev_id = left_id;

        for c in content.chars() {
            let id = self.next_id(site_id);
            let op = TextOperation::Insert {
                id: id.clone(),
                position: index,
                content: c.to_string(),
                origin_left: prev_id.clone(),
                origin_right: right_id.clone(),
            };
            prev_id = Some(id.clone());
            ops.push(op);
        }

        for op in &ops {
            self.apply(op);
        }

        ops
    }

    /// Deletes `len` visible characters starting at `index` on behalf of
    /// `site_id`.
    ///
    /// Returns one delete operation per tombstoned character, already applied
    /// locally. Deleted characters remain as tombstones so concurrent
    /// operations can still reference them.
    pub fn delete(&mut self, site_id: u32, index: usize, len: usize) -> Vec<TextOperation> {
        let targets: Vec<OperationId> = {
            let mut result = Vec::new();
            let mut remaining = len;
            let mut current_idx = 0;

            for node in &self.nodes {
                if !node.tombstone {
                    if current_idx >= index && remaining > 0 {
                        result.push(node.id.clone());
                        remaining -= 1;
                    }
                    current_idx += 1;
                }
                if remaining == 0 {
                    break;
                }
            }
            result
        };

        let mut ops = Vec::new();
        for target in targets {
            let del_id = self.next_id(site_id);
            ops.push(TextOperation::Delete { id: del_id, target });
        }

        for op in &ops {
            self.apply(op);
        }

        ops
    }

    /// Applies a remote operation.
    ///
    /// # Causal delivery
    ///
    /// Operations must be delivered causally: an insert may only be applied
    /// after the operations that created its `origin_left` and
    /// `origin_right` characters, and a delete after its target's insert.
    /// This is the standard RGA delivery model — concurrent operations may
    /// be applied in any interleaving and replicas still converge, but
    /// applying an operation before its origins breaks convergence (real
    /// transports such as WebSocket sessions or sync protocols provide
    /// causal ordering, e.g. via per-site log sequence numbers).
    pub fn apply(&mut self, op: &TextOperation) {
        match op {
            TextOperation::Insert {
                id,
                content,
                origin_left,
                origin_right,
                ..
            } => {
                if id.counter > self.clock {
                    self.clock = id.counter;
                }

                let c = match content.chars().next() {
                    Some(c) => c,
                    None => return,
                };

                let pos = self.find_insert_position(id, origin_left);
                self.nodes.insert(
                    pos,
                    RgaChar {
                        char: c,
                        id: id.clone(),
                        origin_left: origin_left.clone(),
                        origin_right: origin_right.clone(),
                        tombstone: false,
                    },
                );
            }
            TextOperation::Delete { id, target } => {
                if id.counter > self.clock {
                    self.clock = id.counter;
                }

                for node in &mut self.nodes {
                    if node.id == *target {
                        node.tombstone = true;
                        break;
                    }
                }
            }
        }
    }

    /// Renders the visible text, skipping tombstoned characters.
    pub fn text(&self) -> String {
        self.nodes
            .iter()
            .filter(|n| !n.tombstone)
            .map(|n| n.char)
            .collect()
    }
}

impl fmt::Display for RgaString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_insert() {
        let mut rga = RgaString::new();
        rga.insert(1, 0, "Hello");
        assert_eq!(rga.text(), "Hello");

        rga.insert(1, 5, " World");
        assert_eq!(rga.text(), "Hello World");

        rga.insert(1, 0, "> ");
        assert_eq!(rga.text(), "> Hello World");
    }

    #[test]
    fn test_local_delete() {
        let mut rga = RgaString::new();
        rga.insert(1, 0, "Hello World");
        rga.delete(1, 5, 1);
        assert_eq!(rga.text(), "HelloWorld");

        rga.delete(1, 5, 5);
        assert_eq!(rga.text(), "Hello");

        rga.delete(1, 0, 5);
        assert_eq!(rga.text(), "");
    }

    #[test]
    fn test_concurrent_inserts_same_position() {
        let mut rga1 = RgaString::new();
        rga1.insert(1, 0, "AB");

        let mut rga2 = RgaString::new();
        rga2.insert(1, 0, "AB");

        let ops1 = rga1.insert(1, 1, "x");
        let ops2 = rga2.insert(2, 1, "y");

        for op in &ops2 {
            rga1.apply(op);
        }
        for op in &ops1 {
            rga2.apply(op);
        }

        assert_eq!(rga1.text(), rga2.text());
        assert_eq!(rga1.text(), "AyxB");
    }

    #[test]
    fn test_apply_operations_from_multiple_sites() {
        let mut rga1 = RgaString::new();
        rga1.insert(1, 0, "Hello");

        let mut rga2 = RgaString::new();
        rga2.insert(1, 0, "Hello");

        let ops = rga2.insert(2, 5, " World");

        for op in &ops {
            rga1.apply(op);
        }

        assert_eq!(rga1.text(), "Hello World");
    }

    #[test]
    fn test_tombstone_behavior() {
        let mut rga = RgaString::new();
        rga.insert(1, 0, "ABC");
        rga.delete(1, 1, 1);
        assert_eq!(rga.text(), "AC");
        assert_eq!(rga.nodes.len(), 3);
        assert!(
            rga.nodes
                .iter()
                .find(|n| n.id.counter == 2)
                .unwrap()
                .tombstone
        );

        rga.insert(1, 1, "X");
        assert_eq!(rga.text(), "AXC");
        assert_eq!(rga.nodes.len(), 4);
        assert!(
            rga.nodes
                .iter()
                .find(|n| n.id.counter == 2)
                .unwrap()
                .tombstone
        );
    }

    #[test]
    fn test_empty_string() {
        let mut rga = RgaString::new();
        assert_eq!(rga.text(), "");

        let ops = rga.insert(1, 0, "");
        assert!(ops.is_empty());
        assert_eq!(rga.text(), "");

        let del_ops = rga.delete(1, 0, 0);
        assert!(del_ops.is_empty());
        assert_eq!(rga.text(), "");
    }

    #[test]
    fn test_unicode_characters() {
        let mut rga = RgaString::new();
        rga.insert(1, 0, "Hello");
        rga.insert(1, 5, " ");
        rga.insert(1, 6, "世界");
        assert_eq!(rga.text(), "Hello 世界");

        rga.insert(1, 6, "beautiful ");
        assert_eq!(rga.text(), "Hello beautiful 世界");

        rga.delete(1, 6, 10);
        assert_eq!(rga.text(), "Hello 世界");

        rga.delete(1, 5, 1);
        rga.delete(1, 5, 2);
        assert_eq!(rga.text(), "Hello");
    }

    #[test]
    fn test_commutativity() {
        let mut base = RgaString::new();
        let init_ops = base.insert(1, 0, "AB");

        let ops_x = {
            let mut rga = RgaString::new();
            for op in &init_ops {
                rga.apply(op);
            }
            rga.insert(2, 1, "x")
        };

        let ops_y = {
            let mut rga = RgaString::new();
            for op in &init_ops {
                rga.apply(op);
            }
            rga.insert(3, 1, "y")
        };

        let mut rga_a = RgaString::new();
        let mut rga_b = RgaString::new();
        for op in &init_ops {
            rga_a.apply(op);
            rga_b.apply(op);
        }

        for op in &ops_x {
            rga_a.apply(op);
        }
        for op in &ops_y {
            rga_a.apply(op);
        }

        for op in &ops_y {
            rga_b.apply(op);
        }
        for op in &ops_x {
            rga_b.apply(op);
        }

        assert_eq!(
            rga_a.text(),
            rga_b.text(),
            "commutativity violated: x-then-y gives {:?}, y-then-x gives {:?}",
            rga_a.text(),
            rga_b.text()
        );
    }

    #[test]
    fn test_display() {
        let mut rga = RgaString::new();
        rga.insert(1, 0, "Hi");
        let rendered = format!("{}", rga);
        assert_eq!(rendered, "Hi");
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serialization() {
        let op = TextOperation::Insert {
            id: OperationId {
                site_id: 1,
                counter: 42,
            },
            position: 5,
            content: "x".to_string(),
            origin_left: Some(OperationId {
                site_id: 0,
                counter: 1,
            }),
            origin_right: Some(OperationId {
                site_id: 0,
                counter: 2,
            }),
        };

        let json = serde_json::to_string(&op).unwrap();
        let deserialized: TextOperation = serde_json::from_str(&json).unwrap();
        assert_eq!(op, deserialized);
    }
}
