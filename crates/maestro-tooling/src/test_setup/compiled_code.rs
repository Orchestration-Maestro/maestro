//! The code the setup example compiled, carried in the bundle from preparation to a shard.
//!
//! The pinned engine keeps compiled components in one namespace of the user cache, named for the
//! engine and the modification time of the executable that ran. Preparation collects that
//! namespace; a shard installs it under the time its own executables share, so the example and
//! the test processes find every component already compiled.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use super::{at, copy, digest, verify};

/// The compiled-code files of one cache namespace.
#[derive(Serialize, Deserialize)]
pub(super) struct CompiledCode {
    /// The namespace name without the modification time that ends it.
    namespace: String,
    /// Digest of each cache file, by name.
    files: BTreeMap<String, String>,
}

impl CompiledCode {
    /// Stage the namespace that the example left under `cache_home` in `staged`.
    ///
    /// Returns nothing when the host kept no compiled code there, as a host with its own cache
    /// location does.
    ///
    /// # Errors
    /// Returns an error for more than one namespace, for a namespace that does not end in the
    /// example's modification time, and for a failed copy.
    pub(super) fn collect(
        cache_home: &Path,
        example: &Path,
        staged: &Path,
    ) -> io::Result<Option<Self>> {
        let modules = cache_home.join("wasmtime/modules");
        let namespaces = directories(&modules)?;
        let [namespace] = namespaces.as_slice() else {
            return if namespaces.is_empty() {
                Ok(None)
            } else {
                Err(at(&modules, "more than one compiled-code namespace"))
            };
        };
        let name = namespace.file_name().and_then(|name| name.to_str());
        let time = format!("-{}", milliseconds(example)?);
        let Some(prefix) = name.and_then(|name| name.strip_suffix(&time)) else {
            return Err(at(
                namespace,
                format_args!("the name does not follow the example's time {time}"),
            ));
        };
        let mut files = BTreeMap::new();
        for entry in std::fs::read_dir(namespace)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_file() && path.extension() != Some(OsStr::new("stats")) {
                std::fs::create_dir_all(staged)?;
                let copied = staged.join(entry.file_name());
                copy(&path, &copied)?;
                files.insert(
                    entry.file_name().to_string_lossy().into_owned(),
                    digest(&std::fs::read(&copied)?),
                );
            }
        }
        Ok((!files.is_empty()).then(|| Self {
            namespace: prefix.to_owned(),
            files,
        }))
    }

    /// Check every staged file against the digest recorded for it.
    ///
    /// # Errors
    /// Returns an error naming a file that is missing, changed or not a plain file name.
    pub(super) fn verify(&self, staged: &Path) -> io::Result<()> {
        for (name, expected) in &self.files {
            let mut parts = Path::new(name).components();
            if !matches!(
                (parts.next(), parts.next()),
                (Some(Component::Normal(_)), None)
            ) {
                return Err(at(
                    staged,
                    format_args!("invalid compiled-code file {name}"),
                ));
            }
            verify(&staged.join(name), expected)?;
        }
        Ok(())
    }

    /// Install the staged files into the namespace that ends in `example`'s modification time.
    ///
    /// # Errors
    /// Returns an error when the time or a copy fails.
    pub(super) fn install(
        &self,
        staged: &Path,
        user_cache: &Path,
        example: &Path,
    ) -> io::Result<()> {
        let namespace = user_cache.join("wasmtime/modules").join(format!(
            "{}-{}",
            self.namespace,
            milliseconds(example)?
        ));
        std::fs::create_dir_all(&namespace)?;
        for name in self.files.keys() {
            copy(&staged.join(name), &namespace.join(name))?;
        }
        Ok(())
    }
}

/// The directories directly under `parent`, none when it does not exist.
fn directories(parent: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match std::fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(at(parent, &error)),
    };
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            found.push(entry.path());
        }
    }
    Ok(found)
}

/// Milliseconds from the epoch to the modification time of `path`, as the engine names it.
fn milliseconds(path: &Path) -> io::Result<u128> {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map_err(|error| at(path, &error))?
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_millis())
        .map_err(|error| at(path, &error))
}
