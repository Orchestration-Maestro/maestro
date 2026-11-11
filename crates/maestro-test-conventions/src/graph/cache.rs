//! Cargo metadata already read by this thread, keyed by everything Cargo reads.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use cargo_metadata::Metadata;

/// What a read depends on: the workspace root, the host filter and the workspace's inputs.
type Key = (PathBuf, Option<String>, u64);

thread_local! {
    /// Documents this thread has read, so a workspace whose inputs did not change is read once.
    static READ: RefCell<BTreeMap<Key, Metadata>> = const { RefCell::new(BTreeMap::new()) };
}

/// Return the document for the workspace's current inputs, running `read` only on a change.
///
/// Cargo reads manifests, the lock file, `.cargo` configuration and the file layout (which
/// decides each package's targets), so those are the inputs. A workspace that cannot be
/// fingerprinted is read every time, and a failed read is never kept.
pub(super) fn read(
    root: &Path,
    host: Option<&str>,
    read: impl FnOnce() -> Result<Metadata, String>,
) -> Result<Metadata, String> {
    let Some(inputs) = fingerprint(root) else {
        return read();
    };
    let key = (root.to_path_buf(), host.map(str::to_owned), inputs);
    if let Some(document) = READ.with_borrow(|documents| documents.get(&key).cloned()) {
        return Ok(document);
    }
    let document = read()?;
    READ.with_borrow_mut(|documents| documents.insert(key, document.clone()));
    Ok(document)
}

/// Hash the layout of `root` and the bytes of every file Cargo parses, or `None` on an I/O error.
///
/// Build output (`target`) and repository data (`.git`) are not inputs.
fn fingerprint(root: &Path) -> Option<u64> {
    let mut hasher = DefaultHasher::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let mut entries = std::fs::read_dir(&directory)
            .ok()?
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in &entries {
            hash_entry(root, entry, &mut hasher, &mut pending)?;
        }
    }
    Some(hasher.finish())
}

/// Hash one entry's place in the layout and, for a file Cargo parses, its bytes; queue a directory.
fn hash_entry(
    root: &Path,
    entry: &std::fs::DirEntry,
    hasher: &mut DefaultHasher,
    pending: &mut Vec<PathBuf>,
) -> Option<()> {
    let name = entry.file_name();
    let path = entry.path();
    let kind = entry.file_type().ok()?;
    if kind.is_dir() && (name == "target" || name == ".git") {
        return Some(());
    }
    path.strip_prefix(root).ok()?.hash(hasher);
    if kind.is_dir() {
        pending.push(path);
    } else if kind.is_file() && parsed_by_cargo(&path) {
        std::fs::read(&path).ok()?.hash(hasher);
    }
    Some(())
}

/// Whether Cargo reads the file's bytes: a manifest, the lock file or `.cargo` configuration.
fn parsed_by_cargo(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "toml")
        || path.file_name().is_some_and(|name| name == "Cargo.lock")
        || path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|parent| parent == ".cargo")
}

#[cfg(test)]
mod tests;
