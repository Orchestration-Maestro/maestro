//! Opaque terminal identities allocated by the topology owner.
use std::fmt;

/// Terminal identity, independent of pane position.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TerminalId(String);

impl TerminalId {
    /// Format a supplied timestamp and owner-local counter.
    pub(crate) fn from_parts(micros: u128, counter: u64) -> Self {
        Self(format!("term_{micros:x}{counter:x}"))
    }

    /// Borrow the wire identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TerminalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
