//! Collaborative document wrapper.
//!
//! [`CrdtDocument`] pairs an [`RgaString`](crate::text::RgaString) with a
//! [`DocumentId`], a monotonically increasing `version`, and a participant
//! registry, providing the session-level API for collaborative editing.

use std::collections::HashMap;
use std::time::Instant;

#[cfg(feature = "uuid")]
use uuid::Uuid;

use crate::text::{RgaString, TextOperation};

/// Unique identifier for a [`CrdtDocument`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DocumentId(
    /// The unique document identifier string.
    pub String,
);

#[cfg(feature = "uuid")]
impl Default for DocumentId {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentId {
    /// Creates a new identifier backed by a random UUID v4.
    #[cfg(feature = "uuid")]
    pub fn new() -> Self {
        DocumentId(Uuid::new_v4().to_string())
    }
}

/// Identifies a participant (replica) collaborating on a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParticipantId(
    /// The participant's site id; also the `site_id` used in operations.
    pub u32,
);

/// Presence information for a document participant.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParticipantInfo {
    /// Site id used when this participant authors operations.
    pub site_id: u32,
    /// Human-readable display name.
    pub name: String,
    /// Last time this participant was observed active. Not serialized;
    /// defaults to the current time on deserialization.
    #[cfg_attr(feature = "serde", serde(skip, default = "default_instant"))]
    pub last_seen: Instant,
}

#[cfg_attr(not(feature = "serde"), allow(dead_code))]
fn default_instant() -> Instant {
    Instant::now()
}

/// A collaborative text document with participants and a version counter.
///
/// Edits made through [`insert_text`](CrdtDocument::insert_text) and
/// [`delete_text`](CrdtDocument::delete_text) return the operations to
/// broadcast; remote operations are ingested with
/// [`apply_ops`](CrdtDocument::apply_ops). Documents that have applied the
/// same operation set render the same
/// [`get_text`](CrdtDocument::get_text).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CrdtDocument {
    /// Unique identifier of this document.
    pub id: DocumentId,
    content: RgaString,
    /// Monotonic version, bumped on every local edit or remote ingestion.
    pub version: u64,
    /// Registered participants keyed by [`ParticipantId`].
    pub participants: HashMap<ParticipantId, ParticipantInfo>,
}

impl CrdtDocument {
    /// Creates an empty document with the given id.
    pub fn new(id: DocumentId) -> Self {
        CrdtDocument {
            id,
            content: RgaString::new(),
            version: 0,
            participants: HashMap::new(),
        }
    }

    /// Registers a participant with a display name and returns its id.
    pub fn join(&mut self, participant_id: ParticipantId, name: &str) -> ParticipantId {
        self.participants.insert(
            participant_id,
            ParticipantInfo {
                site_id: participant_id.0,
                name: name.to_string(),
                last_seen: Instant::now(),
            },
        );
        participant_id
    }

    /// Removes a participant from the registry.
    pub fn leave(&mut self, participant_id: &ParticipantId) {
        self.participants.remove(participant_id);
    }

    /// Inserts `text` at `index` on behalf of `participant_id`.
    ///
    /// Returns the operations to broadcast plus the new document version.
    pub fn insert_text(
        &mut self,
        participant_id: ParticipantId,
        index: usize,
        text: &str,
    ) -> (Vec<TextOperation>, u64) {
        let site_id = self
            .participants
            .get(&participant_id)
            .map(|p| p.site_id)
            .unwrap_or(participant_id.0);

        let ops = self.content.insert(site_id, index, text);
        self.version += 1;
        (ops, self.version)
    }

    /// Deletes `len` characters at `index` on behalf of `participant_id`.
    ///
    /// Returns the operations to broadcast plus the new document version.
    pub fn delete_text(
        &mut self,
        participant_id: ParticipantId,
        index: usize,
        len: usize,
    ) -> (Vec<TextOperation>, u64) {
        let site_id = self
            .participants
            .get(&participant_id)
            .map(|p| p.site_id)
            .unwrap_or(participant_id.0);

        let ops = self.content.delete(site_id, index, len);
        self.version += 1;
        (ops, self.version)
    }

    /// Ingests remote operations and returns the new document version.
    pub fn apply_ops(&mut self, ops: &[TextOperation]) -> u64 {
        for op in ops {
            self.content.apply(op);
        }
        self.version += 1;
        self.version
    }

    /// Renders the visible document text.
    pub fn get_text(&self) -> String {
        self.content.text()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // test assertions unwrap by design
    use super::*;

    #[test]
    fn test_create() {
        let doc = CrdtDocument::new(DocumentId("test-doc".to_string()));
        assert_eq!(doc.id.0, "test-doc");
        assert_eq!(doc.version, 0);
        assert!(doc.participants.is_empty());
        assert_eq!(doc.get_text(), "");
    }

    #[test]
    #[cfg(feature = "uuid")]
    fn test_join_leave() {
        let mut doc = CrdtDocument::new(DocumentId::new());

        let p1 = ParticipantId(1);
        doc.join(p1, "Alice");
        assert!(doc.participants.contains_key(&p1));
        assert_eq!(doc.participants[&p1].name, "Alice");
        assert_eq!(doc.participants[&p1].site_id, 1);

        let p2 = ParticipantId(2);
        doc.join(p2, "Bob");
        assert_eq!(doc.participants.len(), 2);

        doc.leave(&p1);
        assert_eq!(doc.participants.len(), 1);
        assert!(!doc.participants.contains_key(&p1));
        assert!(doc.participants.contains_key(&p2));
    }

    #[test]
    #[cfg(feature = "uuid")]
    fn test_insert_and_version() {
        let mut doc = CrdtDocument::new(DocumentId::new());
        doc.join(ParticipantId(1), "Alice");

        let (ops, v1) = doc.insert_text(ParticipantId(1), 0, "Hello");
        assert_eq!(v1, 1);
        assert!(!ops.is_empty());
        assert_eq!(doc.get_text(), "Hello");
        assert_eq!(doc.version, 1);

        let (_, v2) = doc.insert_text(ParticipantId(1), 5, " World");
        assert_eq!(v2, 2);
        assert_eq!(doc.get_text(), "Hello World");
        assert_eq!(doc.version, 2);
    }

    #[test]
    fn test_apply_remote_ops() {
        let mut doc1 = CrdtDocument::new(DocumentId("shared".to_string()));
        doc1.join(ParticipantId(1), "Alice");
        let (ops, v1) = doc1.insert_text(ParticipantId(1), 0, "Hello");
        assert_eq!(v1, 1);

        let mut doc2 = CrdtDocument::new(DocumentId("shared".to_string()));
        doc2.join(ParticipantId(2), "Bob");

        let v = doc2.apply_ops(&ops);
        assert_eq!(v, 1);
        assert_eq!(doc2.get_text(), "Hello");
        assert_eq!(doc2.version, 1);
    }

    #[test]
    #[cfg(feature = "uuid")]
    fn test_multiple_participants() {
        let mut doc = CrdtDocument::new(DocumentId::new());
        doc.join(ParticipantId(1), "Alice");
        doc.join(ParticipantId(2), "Bob");

        let (ops1, v1) = doc.insert_text(ParticipantId(1), 0, "Hello");
        assert_eq!(v1, 1);

        let (ops2, v2) = doc.insert_text(ParticipantId(2), 5, " World");
        assert_eq!(v2, 2);

        assert_eq!(doc.get_text(), "Hello World");

        let mut doc2 = CrdtDocument::new(DocumentId::new());
        doc2.join(ParticipantId(3), "Charlie");

        let v3 = doc2.apply_ops(&ops1);
        assert_eq!(v3, 1);
        let v4 = doc2.apply_ops(&ops2);
        assert_eq!(v4, 2);
        assert_eq!(doc2.get_text(), "Hello World");
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serialization() {
        let mut doc = CrdtDocument::new(DocumentId("serde-test".to_string()));
        doc.join(ParticipantId(1), "Alice");
        doc.insert_text(ParticipantId(1), 0, "Hello");

        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: CrdtDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(doc.id, deserialized.id);
        assert_eq!(doc.version, deserialized.version);
        assert_eq!(doc.get_text(), deserialized.get_text());
    }
}
