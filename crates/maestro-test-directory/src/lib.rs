//! Exclusive disposable directory ownership for native tests.
#![cfg(not(target_arch = "wasm32"))]

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// Sequence shared by all callers in this process.
static NEXT: AtomicU64 = AtomicU64::new(0);

/// The platform temporary directory, for tests that observe it without owning it.
#[must_use]
pub fn temporary_root() -> PathBuf {
    std::env::temp_dir()
}

/// An exclusively created directory whose removal is attempted on drop.
#[derive(Debug)]
pub struct Scratch {
    /// The spelling selected when this owner was created.
    path: PathBuf,
}

impl Scratch {
    /// Create a directory under the native temporary root and retain its canonical path.
    /// On Windows the path drops the verbatim `\\?\` prefix of a drive path, which would
    /// make `/` an ordinary character and differs from the paths users see.
    ///
    /// # Errors
    /// Returns a directory creation or canonicalization failure.
    pub fn new() -> io::Result<Self> {
        let mut scratch = Self::new_in(&temporary_root())?;
        scratch.path = without_verbatim_disk(fs::canonicalize(&scratch.path)?);
        Ok(scratch)
    }

    /// Create a directory under an existing parent using native path joining, without canonicalization.
    ///
    /// # Errors
    /// Returns a directory creation failure other than a candidate name collision.
    pub fn new_in(parent: &Path) -> io::Result<Self> {
        allocate(parent, &NEXT)
    }

    /// The owned directory, with the spelling selected at creation.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Create `names` together in a new directory below this owner and report whether the
    /// running host stored that many distinct entries. A name the host folded onto an earlier
    /// one, or refused to store, reports `false`. Callers pass leaf names, so a `false` answer
    /// states a premise the host cannot exercise; like `deny` and `enter_unreadable`, the
    /// answer describes the host rather than an expectation.
    ///
    /// # Errors
    /// Returns a creation or removal failure that is not the host's answer about a name.
    pub fn keeps_distinct(&self, names: &[&OsStr]) -> io::Result<bool> {
        let probe = self.path.join("name-admission");
        fs::create_dir(&probe)?;
        let stored = names.iter().try_fold(0_usize, |stored, name| {
            match fs::File::create_new(probe.join(name)) {
                Ok(_) => Ok(stored + 1),
                Err(error) if name_refused(&probe, &error) => Ok(stored),
                Err(error) => Err(error),
            }
        });
        let removed = fs::remove_dir_all(&probe);
        let stored = stored?;
        removed.map(|()| stored == names.len())
    }

    /// The authored name groups this host does not keep apart, one group per directory that
    /// holds names it folded onto one entry or refused to store.
    ///
    /// Callers pass relative paths separated by `/`. Every component of every path is compared
    /// with the other names its own directory holds, so a fold or a refusal through a directory
    /// component is reported like one at a leaf. An empty answer states a premise the host can
    /// exercise; each reported group names the spellings it could not store together.
    ///
    /// # Errors
    /// Returns a creation or removal failure that is not the host's answer about a name.
    pub fn folded_names(&self, paths: &[&str]) -> io::Result<Vec<Vec<String>>> {
        let mut groups: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
        for path in paths {
            let mut parent = Vec::new();
            for name in path.split('/').filter(|name| !name.is_empty()) {
                groups.entry(parent.join("/")).or_default().insert(name);
                parent.push(name);
            }
        }
        let mut folded = Vec::new();
        for names in groups.into_values() {
            let spellings: Vec<&OsStr> = names.iter().map(OsStr::new).collect();
            if !self.keeps_distinct(&spellings)? {
                folded.push(names.into_iter().map(str::to_owned).collect());
            }
        }
        Ok(folded)
    }

    /// Create `name` in this directory, make it the process working directory and remove it.
    /// Reading the working directory then fails with `NotFound`, while `..` still leads back
    /// into this directory. Returns the removed directory. The working directory belongs to
    /// the whole process, so only a process running one test may call this.
    ///
    /// # Errors
    /// Returns a directory creation, working-directory change or removal failure.
    #[cfg(unix)]
    pub fn enter_removed(&self, name: &str) -> io::Result<PathBuf> {
        let removed = self.path.join(name);
        fs::create_dir(&removed)?;
        std::env::set_current_dir(&removed)?;
        fs::remove_dir(&removed)?;
        Ok(removed)
    }

    /// Create `name` in this directory and make a directory below it the process working
    /// directory, deeper than the kernel's path limit, then take read permission from
    /// `name`. Reading the working directory then fails with `PermissionDenied` while paths
    /// relative to it keep working. Returns the working directory and the guard that gives
    /// `name` its permissions back; drop the guard before this owner. The working directory
    /// belongs to the whole process, so only a process running one test may call this.
    /// Returns `None` when the process still reads `name`, as a privileged process does.
    ///
    /// # Errors
    /// Returns a directory creation, working-directory change or permission failure.
    #[cfg(unix)]
    pub fn enter_unreadable(&self, name: &str) -> io::Result<Option<(PathBuf, Restore)>> {
        let locked = self.path.join(name);
        fs::create_dir(&locked)?;
        std::env::set_current_dir(&locked)?;
        let mut process = locked.clone();
        let segment = "d".repeat(250);
        for _ in 0..20 {
            fs::create_dir(&segment)?;
            std::env::set_current_dir(&segment)?;
            process.push(&segment);
        }
        Ok(deny(&locked, 0o311)?.map(|restore| (process, restore)))
    }
}

/// Set `mode` on `path` and check that this process is now refused what the mode takes
/// from the owner: reading or listing without read, writing or creating an entry without
/// write, and looking up an entry without search. Returns the guard that gives `path` its
/// earlier permissions back, or `None`, with those permissions already back, when the
/// process is not refused, as in a privileged run.
///
/// # Errors
/// Returns a metadata or permission-change failure.
#[cfg(unix)]
pub fn deny(path: impl AsRef<Path>, mode: u32) -> io::Result<Option<Restore>> {
    use std::os::unix::fs::PermissionsExt;
    let path = path.as_ref();
    let metadata = fs::metadata(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    let restore = Restore(path.to_owned(), metadata.permissions());
    Ok(refused(path, mode, metadata.is_dir()).then_some(restore))
}

/// Whether each owner access that `mode` lacks is refused to this process.
#[cfg(unix)]
fn refused(path: &Path, mode: u32, directory: bool) -> bool {
    let denied = |result: io::Result<()>| {
        result.is_err_and(|error| error.kind() == io::ErrorKind::PermissionDenied)
    };
    let probe = path.join(".maestro-deny-probe");
    let read = || {
        if directory {
            fs::read_dir(path).map(drop)
        } else {
            fs::File::open(path).map(drop)
        }
    };
    let write = || {
        if directory {
            fs::create_dir(&probe).and_then(|()| fs::remove_dir(&probe))
        } else {
            fs::OpenOptions::new().append(true).open(path).map(drop)
        }
    };
    (mode & 0o400 != 0 || denied(read()))
        && (mode & 0o200 != 0 || denied(write()))
        && (mode & 0o100 != 0 || !directory || denied(fs::symlink_metadata(&probe).map(drop)))
}

/// Gives a denied path its earlier permissions back on drop.
#[cfg(unix)]
#[derive(Debug)]
pub struct Restore(PathBuf, fs::Permissions);

#[cfg(unix)]
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, self.1.clone());
    }
}

/// Whether a failed creation means the running host did not store the name: it folded the
/// name onto an earlier entry, or it refused this name while a control name in the same
/// directory was stored.
fn name_refused(directory: &Path, error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::AlreadyExists
        || fs::write(directory.join("control"), "control").is_ok()
}

/// `path` with a verbatim drive prefix (`\\?\C:`) replaced by the plain drive (`C:`).
fn without_verbatim_disk(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        let mut components = path.components();
        if let Some(Component::Prefix(prefix)) = components.next()
            && let Prefix::VerbatimDisk(disk) = prefix.kind()
        {
            let mut plain = PathBuf::from(format!("{}:", char::from(disk)));
            plain.push(components.as_path());
            return plain;
        }
    }
    path
}

/// Retry only existing candidate names, retaining every pre-existing entry.
fn allocate(parent: &Path, sequence: &AtomicU64) -> io::Result<Scratch> {
    loop {
        let path = parent.join(format!(
            "maestro-test-{}-{}",
            std::process::id(),
            sequence.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(Scratch { path }),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests;
