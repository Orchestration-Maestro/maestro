//! The areas of a release's manifest root, read in order and combined.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

/// Read access to the files under a release's manifest root.
pub trait ReleaseFiles {
    /// Names of the folders directly under a root-relative folder, sorted; empty when it is absent.
    fn folders(&self, folder: &str) -> Vec<String>;

    /// Text of a root-relative file, or `None` when it is absent.
    ///
    /// # Errors
    /// Returns the reason an existing file could not be read.
    fn text(&self, path: &str) -> Result<Option<String>, String>;
}

/// The settings and locks of every area a manifest loads.
#[derive(Debug, Default, PartialEq)]
pub struct Composition {
    /// The areas' settings documents, combined in area order.
    pub settings: Map<String, Value>,
    /// The key paths and lock names the areas' policies lock.
    pub locked: BTreeSet<String>,
}

/// Reads the common, standards and core areas, then the listed languages and capabilities.
///
/// `requested` is the user's or project's `manifestAreas` value; without one the manifest's own
/// value applies.
///
/// # Errors
/// Returns the problem with the first area file or entry that makes the manifest invalid.
pub fn read(_files: &impl ReleaseFiles, _requested: Option<&Value>) -> Result<Composition, String> {
    Err("not read".into())
}
