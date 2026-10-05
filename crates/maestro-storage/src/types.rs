use std::sync::Arc;

/// Explicit identity and caller-owned opaque header bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionHeader {
    /// Identity within one storage adapter.
    pub session_id: String,
    /// Header encoding owned by the caller.
    pub data: Vec<u8>,
}

/// One opaque record with a session-local identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// Lookup identity within the session.
    pub id: String,
    /// Uninterpreted payload.
    pub data: Vec<u8>,
}

/// Detached identity and adapter-provided persistence facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionMetadata {
    /// Caller-provided session header.
    pub header: SessionHeader,
    /// Opaque adapter locator, not necessarily a file path.
    pub persistent_locator: Option<String>,
    /// Whether the adapter supports restart persistence.
    pub resumable: bool,
}

/// Detached, consistent state from one publication point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSnapshot {
    /// Session identity and persistence facts.
    pub metadata: SessionMetadata,
    /// Records in append order.
    pub records: Vec<Record>,
    /// Selected record identity, or the position before records.
    pub selected_position: Option<String>,
}

/// Distinguishes unchanged state, unknown outcomes and closed admission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageError {
    /// The operation definitely made no change.
    Rejected {
        /// Diagnostic text, not a payload or machine-readable code.
        reason: String,
    },
    /// Publication is uncertain; this handle cannot safely mutate further.
    Uncertain {
        /// Diagnostic text without a rollback claim.
        reason: String,
    },
    /// This handle is closing or closed.
    Closed,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected { reason } => write!(f, "storage rejected: {reason}"),
            Self::Uncertain { reason } => write!(f, "storage outcome uncertain: {reason}"),
            Self::Closed => f.write_str("storage handle closed"),
        }
    }
}

impl std::error::Error for StorageError {}

/// Synchronous, replaceable collection of explicitly identified sessions.
pub trait Storage: Send + Sync {
    /// Creates an empty session; duplicate identity is rejected without replacement.
    fn create(&self, header: SessionHeader) -> Result<Arc<dyn RecordSession>, StorageError>;
    /// Opens an existing identity with an independent handle lifecycle.
    fn open(&self, session_id: &str) -> Result<Arc<dyn RecordSession>, StorageError>;
    /// Lists detached metadata, including sessions whose handles were closed.
    fn list(&self) -> Result<Vec<SessionMetadata>, StorageError>;
}

/// Shareable synchronous handle permanently bound to one session identity.
///
/// Mutations serialize across independent opens. Batches and their positions publish
/// atomically. Close stops admission before blocking for admitted writes. Arc clones
/// share admission; independent opens do not. Uncertain mutations must disable further
/// mutation through the handle and its clones, including pending mutations.
pub trait RecordSession: Send + Sync {
    /// Reads detached metadata, ordered records and position atomically.
    fn read(&self) -> Result<SessionSnapshot, StorageError>;
    /// Reads a detached matching record within this identity only.
    fn get(&self, record_id: &str) -> Result<Option<Record>, StorageError>;
    /// Appends a whole batch and replaces its position as one publication.
    ///
    /// Duplicate IDs or an unknown selected ID reject without changing any state.
    /// Empty batches are allowed; None selects the position before all records.
    fn append(
        &self,
        records: Vec<Record>,
        selected_position: Option<String>,
    ) -> Result<(), StorageError>;
    /// Replaces the position without changing records; unknown IDs are rejected.
    fn select(&self, selected_position: Option<String>) -> Result<(), StorageError>;
    /// Stops admission and blocks until admitted writes settle; idempotent.
    fn close(&self) -> Result<(), StorageError>;
}
