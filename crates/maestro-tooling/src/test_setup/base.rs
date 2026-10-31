//! An earlier bundle that a preparation reuses what it can from.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use super::inputs::Inputs;
use super::plan::own_inputs;
use super::{COMPILED, Manifest, read_manifest, verify};

/// A bundle prepared from the same shared inputs as the checkout.
pub(super) struct Base {
    /// The bundle directory.
    directory: PathBuf,
    /// What the bundle records.
    manifest: Manifest,
}

impl Base {
    /// Read the bundle in `directory`, if it holds a manifest prepared from the `shared` inputs.
    ///
    /// A missing or unreadable bundle, one in an older format, and one prepared from other shared
    /// inputs, a toolchain among them, give nothing: the preparation then builds everything.
    pub(super) fn read(directory: &Path, shared: &Inputs) -> Option<Self> {
        let manifest = read_manifest(directory).ok()?;
        (manifest.shared.digest == shared.digest).then(|| Self {
            directory: directory.to_owned(),
            manifest,
        })
    }

    /// The staged component files whose own source is unchanged, by variable.
    ///
    /// # Errors
    /// Returns a failed listing or read of the example sources.
    pub(super) fn supplied(
        &self,
        checkout: &Path,
        names: &BTreeMap<String, String>,
    ) -> io::Result<Vec<(String, PathBuf)>> {
        let mut supplied = Vec::new();
        for (variable, component) in &self.manifest.components {
            let path = self.directory.join(format!("{variable}.wasm"));
            let own = own_inputs(checkout, names.get(variable).map(String::as_str))?;
            if own.digest == component.inputs.digest && verify(&path, &component.file).is_ok() {
                supplied.push((variable.clone(), path));
            }
        }
        Ok(supplied)
    }

    /// Install the compiled code of the bundle in the user cache `cache_home`, for the modification
    /// time of `example`, and return the namespace it went to.
    ///
    /// # Errors
    /// Returns a failed copy.
    pub(super) fn seed(&self, cache_home: &Path, example: &Path) -> io::Result<Option<PathBuf>> {
        let staged = self.directory.join(COMPILED);
        match &self.manifest.compiled {
            Some(code) if code.verify(&staged).is_ok() => {
                code.install(&staged, cache_home, example).map(Some)
            }
            _ => Ok(None),
        }
    }
}
