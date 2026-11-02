//! Configuration files read once and the observations that tell when they changed.
use super::syntax::{Item, parse};
use crate::workspace::git::{discovery::canonicalize_best_effort_path, native::read_text};
use std::{
    collections::HashMap,
    io::ErrorKind,
    path::{Path, PathBuf},
    rc::Rc,
    time::SystemTime,
};

/// What the file system showed for one path.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Observation {
    /// Modification time and length, absent when the path does not exist.
    stamp: Option<(Option<SystemTime>, u64)>,
    /// Where a symbolic link resolved, kept only for the link's own path.
    target: Option<PathBuf>,
    /// Whether the read failed for a reason other than absence.
    failed: bool,
}

impl Observation {
    /// Observe `path`, resolving it when it is the alias of another file.
    fn of(path: &Path, alias: bool) -> Self {
        let metadata = std::fs::metadata(path);
        Self {
            failed: metadata
                .as_ref()
                .is_err_and(|error| error.kind() != ErrorKind::NotFound),
            stamp: metadata
                .ok()
                .map(|metadata| (metadata.modified().ok(), metadata.len())),
            target: alias.then(|| canonicalize_best_effort_path(path)),
        }
    }
}

/// A path a read depended on and what it showed then.
#[derive(Debug, Clone)]
pub(super) struct Dependency {
    /// The path as the configuration named it.
    path: PathBuf,
    /// What it showed when read.
    seen: Observation,
}

impl Dependency {
    /// Whether the path still shows what it showed and the read had succeeded.
    pub(super) fn is_current(&self) -> bool {
        !self.seen.failed && Observation::of(&self.path, self.seen.target.is_some()) == self.seen
    }
}

/// Parsed files keyed by their resolved path, with the dependencies their reads created.
#[derive(Default)]
pub(super) struct Reader {
    /// Parsed content; absent for a file that does not exist or could not be read.
    parsed: HashMap<PathBuf, Option<Rc<[Item]>>>,
    /// Every path read, in first-read order.
    pub(super) dependencies: Vec<Dependency>,
}

impl Reader {
    /// Resolve `path`, then parse it once; an unreadable file reads as absent.
    pub(super) fn read(&mut self, path: &Path) -> (PathBuf, Option<Rc<[Item]>>) {
        let resolved = canonicalize_best_effort_path(path);
        if resolved != path {
            self.dependencies.push(Dependency {
                path: path.to_path_buf(),
                seen: Observation::of(path, true),
            });
        }
        if let Some(items) = self.parsed.get(&resolved) {
            return (resolved, items.clone());
        }
        let mut seen = Observation::of(&resolved, false);
        let items = match read_text(&resolved) {
            Ok(text) => Some(Rc::from(parse(&text))),
            Err(error) => {
                seen.failed |= error.kind() != ErrorKind::NotFound;
                None
            }
        };
        self.dependencies.push(Dependency {
            path: resolved.clone(),
            seen,
        });
        self.parsed.insert(resolved.clone(), items.clone());
        (resolved, items)
    }
}
