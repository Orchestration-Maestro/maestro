//! Visible contribution failures.
use std::fmt;
/// Why a contribution could not be loaded, registered or mounted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContributionFailure {
    /// A required host API is not provided.
    MissingApi {
        /// Contribution identifier.
        id: String,
        /// Missing API name.
        api: String,
    },
    /// The slot supplies a different data contract.
    IncompatibleInput {
        /// Contribution identifier.
        id: String,
        /// Consumed contract.
        expected: String,
        /// Supplied contract.
        supplied: Option<String>,
    },
    /// A module registered an identifier it did not declare.
    Undeclared {
        /// Registered identifier.
        id: String,
    },
    /// Loading or initializing a module failed.
    Initialization {
        /// Module entry.
        entry: String,
        /// Browser-reported cause.
        cause: String,
    },
}
impl fmt::Display for ContributionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApi { id, api } => write!(f, "{id}: missing host API {api}"),
            Self::IncompatibleInput {
                id,
                expected,
                supplied,
            } => write!(
                f,
                "{id}: expects {expected}, slot supplies {}",
                supplied.as_deref().unwrap_or("no input")
            ),
            Self::Undeclared { id } => write!(f, "{id}: not declared by its package"),
            Self::Initialization { entry, cause } => write!(f, "{entry}: {cause}"),
        }
    }
}
impl std::error::Error for ContributionFailure {}
