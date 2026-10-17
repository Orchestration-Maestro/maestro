//! Source-retaining relocation and final referent settlement.

#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::{symlink_dir, symlink_file};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
};

/// An admitted relocation with its final link identity captured before any move.
pub(super) struct PreparedMove {
    /// Canonical parent plus original entry name, without following the entry.
    pub(super) path: PathBuf,
    /// Caller-selected destination.
    pub(super) target: PathBuf,
    /// Canonical final target for a live symbolic link.
    pub(super) referent: Option<PathBuf>,
}

/// Resolve a captured target through successfully moved entries or their ancestors.
fn relocated_referent(referent: &Path, moved: &BTreeMap<PathBuf, PathBuf>) -> PathBuf {
    for ancestor in referent.ancestors() {
        if let Some(target) = moved.get(ancestor)
            && let Ok(suffix) = referent.strip_prefix(ancestor)
        {
            return if suffix.as_os_str().is_empty() {
                target.clone()
            } else {
                target.join(suffix)
            };
        }
    }
    referent.to_path_buf()
}

/// Remove an entry without following directory links on Windows.
pub(super) fn remove_link_entry(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileTypeExt;
        if fs::symlink_metadata(path)?.file_type().is_symlink_dir() {
            return fs::remove_dir(path);
        }
    }
    fs::remove_file(path)
}

/// Observe traversed entries through native directory and link resolution.
fn observe(
    path: &Path,
    sources: &BTreeMap<PathBuf, usize>,
    dependencies: &mut BTreeSet<usize>,
) -> io::Result<PathBuf> {
    let mut components = path.components();
    let Some(std::path::Component::Normal(name)) = components.next_back() else {
        if path.ends_with("..") {
            let parent = observe(components.as_path(), sources, dependencies)?;
            return fs::canonicalize(parent.join(".."));
        }
        return fs::canonicalize(path);
    };
    let parent = observe(
        path.parent().unwrap_or(Path::new(".")),
        sources,
        dependencies,
    )?;
    let entry = parent.join(name);
    if let Some(index) = sources.get(&entry) {
        dependencies.insert(*index);
    }
    match fs::read_link(&entry) {
        Ok(payload) => observe(&parent.join(payload), sources, dependencies),
        Err(_) => fs::canonicalize(entry),
    }
}

/// Captured link observations and invocation-owned publication state.
struct Link {
    /// Entries required by the original source's resolution.
    dependencies: BTreeSet<usize>,
    /// Whether native unchanged rename is safe without any batch dependency.
    stable: bool,
    /// Directory-link kind captured before any moves.
    directory: bool,
    /// Publication selected or owned by this invocation.
    publication: Publication,
}

/// Exclusive alternatives for a link's transient publication.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Publication {
    /// No destination is owned.
    Pending,
    /// Unchanged native rename awaits settlement.
    Native,
    /// A newly created destination awaits verification.
    Published,
}

/// Prove an unchanged binding using native resolution, never absolute syntax alone.
fn stable_binding(payload: &Path, target: &Path, referent: &Path) -> bool {
    target.parent().is_some_and(|parent| {
        fs::canonicalize(parent.join(payload)).is_ok_and(|actual| actual == referent)
    })
}

/// Windows privilege failure permits only an already-proven stable binding.
#[cfg(any(windows, test))]
pub(super) fn privilege_rename(error: &io::Error, stable: bool) -> bool {
    error.raw_os_error() == Some(1314) && stable
}

/// Capture dependencies while every admitted source remains available.
fn capture(entries: &[PreparedMove]) -> Vec<Option<Link>> {
    let sources = entries
        .iter()
        .enumerate()
        .map(|(i, entry)| (entry.path.clone(), i))
        .collect();
    entries
        .iter()
        .map(|entry| {
            let referent = entry.referent.as_ref()?;
            let payload = fs::read_link(&entry.path).ok()?;
            let mut dependencies = BTreeSet::new();
            let parent = entry.path.parent()?;
            observe(&parent.join(&payload), &sources, &mut dependencies).ok()?;
            Some(Link {
                stable: dependencies.is_empty()
                    && stable_binding(&payload, &entry.target, referent),
                dependencies,
                directory: entry.path.is_dir(),
                publication: Publication::Pending,
            })
        })
        .collect()
}

/// Pin all entries needed by retained original sources.
fn pin(held: &mut BTreeSet<usize>, links: &[Option<Link>]) {
    loop {
        let before = held.len();
        for index in held.clone() {
            if let Some(link) = &links[index] {
                held.extend(&link.dependencies);
            }
        }
        if held.len() == before {
            break;
        }
    }
}

/// Create an exclusive source-adjacent stage and publish without retiring the source.
fn publish(entry: &PreparedMove, link: &mut Link, referent: &Path) -> io::Result<()> {
    let mut stage = entry.path.as_os_str().to_os_string();
    stage.push(".migrating");
    let stage = PathBuf::from(stage);
    #[cfg(unix)]
    let created = {
        let _ = link.directory;
        symlink(referent, &stage)
    };
    #[cfg(windows)]
    let created = if link.directory {
        symlink_dir(referent, &stage)
    } else {
        symlink_file(referent, &stage)
    };
    #[cfg(windows)]
    if created
        .as_ref()
        .is_err_and(|error| privilege_rename(error, link.stable))
    {
        link.publication = Publication::Native;
        return Ok(());
    }
    created?;
    let result = fs::rename(&stage, &entry.target);
    if result.is_err() {
        let _ = remove_link_entry(&stage);
    }
    result?;
    link.publication = Publication::Published;
    Ok(())
}

/// Remove only provisional links created by this invocation.
fn discard(entries: &[PreparedMove], links: &mut [Option<Link>]) {
    for (entry, link) in entries.iter().zip(links) {
        if let Some(link) = link {
            if link.publication == Publication::Published {
                let _ = remove_link_entry(&entry.target);
            }
            link.publication = Publication::Pending;
        }
    }
}

/// Publish every viable dependant before any non-link referent moves.
fn prepare(entries: &[PreparedMove], links: &mut [Option<Link>], held: &mut BTreeSet<usize>) {
    loop {
        pin(held, links);
        let planned = entries
            .iter()
            .enumerate()
            .filter(|(i, e)| !held.contains(i) && e.referent.is_none())
            .map(|(_, e)| (e.path.clone(), e.target.clone()))
            .collect();
        let mut failed = None;
        for (i, (entry, link)) in entries.iter().zip(links.iter_mut()).enumerate() {
            let Some(link) = link else {
                continue;
            };
            if held.contains(&i) {
                continue;
            }
            link.publication =
                if link.stable && fs::read_link(&entry.path).is_ok_and(|p| p.is_absolute()) {
                    Publication::Native
                } else {
                    Publication::Pending
                };
            if link.publication != Publication::Native
                && let Some(referent) = &entry.referent
                && publish(entry, link, &relocated_referent(referent, &planned)).is_err()
            {
                failed = Some(i);
                break;
            }
        }
        let Some(index) = failed else {
            break;
        };
        held.insert(index);
        discard(entries, links);
    }
}

/// Execute admitted moves with counts only after verified settlement.
pub(super) fn relocate(entries: Vec<PreparedMove>) -> usize {
    let entries = entries
        .into_iter()
        .filter_map(|mut entry| {
            entry.target = super::canonical_entry_path(&entry.target).ok()?;
            Some(entry)
        })
        .collect::<Box<[_]>>();
    let mut links = capture(&entries);
    let mut held: BTreeSet<_> = entries
        .iter()
        .enumerate()
        .filter(|(i, e)| e.referent.is_some() && links[*i].is_none())
        .map(|(i, _)| i)
        .collect();
    prepare(&entries, &mut links, &mut held);
    let (actual, count) = move_referents(&entries, &mut held);
    let settled = settle(&entries, &mut links, &mut held, &actual);
    discard(&entries, &mut links);
    count + settled
}

/// Move unpinned non-links and record only observed successful destinations.
fn move_referents(
    entries: &[PreparedMove],
    held: &mut BTreeSet<usize>,
) -> (BTreeMap<PathBuf, PathBuf>, usize) {
    let mut actual = BTreeMap::new();
    let mut count = 0;
    for (i, entry) in entries.iter().enumerate() {
        if entry.referent.is_none() && !held.contains(&i) {
            if fs::rename(&entry.path, &entry.target).is_ok()
                && fs::canonicalize(&entry.target).is_ok_and(|p| p == entry.target)
            {
                actual.insert(entry.path.clone(), entry.target.clone());
                count += 1;
            } else {
                held.insert(i);
            }
        }
    }
    (actual, count)
}

/// Verify provisional bindings against actual moves before retiring any link.
fn verify(
    entries: &[PreparedMove],
    links: &[Option<Link>],
    held: &mut BTreeSet<usize>,
    actual: &BTreeMap<PathBuf, PathBuf>,
) {
    // Failed referent moves invalidate provisional destinations, never original sources.
    for (i, entry) in entries.iter().enumerate() {
        if let Some(link) = &links[i] {
            let expected = entry
                .referent
                .as_ref()
                .map(|p| relocated_referent(p, actual));
            let observed = if link.publication == Publication::Native {
                &entry.path
            } else {
                &entry.target
            };
            if !held.contains(&i) && fs::canonicalize(observed).ok() != expected {
                held.insert(i);
            }
        }
    }
    pin(held, links);
}

/// Retire dependants before their dependencies and count each verified destination.
fn settle(
    entries: &[PreparedMove],
    links: &mut [Option<Link>],
    held: &mut BTreeSet<usize>,
    actual: &BTreeMap<PathBuf, PathBuf>,
) -> usize {
    verify(entries, links, held, actual);
    let mut count = 0;
    let mut remaining: BTreeSet<_> = links
        .iter()
        .enumerate()
        .filter(|(i, link)| link.is_some() && !held.contains(i))
        .map(|(i, _)| i)
        .collect();
    while let Some(index) = remaining.iter().copied().find(|i| {
        !remaining.iter().any(|j| {
            links[*j]
                .as_ref()
                .is_some_and(|l| l.dependencies.contains(i))
        })
    }) {
        remaining.remove(&index);
        let entry = &entries[index];
        let Some(link) = &links[index] else {
            continue;
        };
        if held.contains(&index) {
            continue;
        }
        let retired = if link.publication == Publication::Native {
            fs::rename(&entry.path, &entry.target)
        } else {
            remove_link_entry(&entry.path)
        };
        if retired.is_ok()
            && fs::canonicalize(&entry.target).ok()
                == entry
                    .referent
                    .as_ref()
                    .map(|p| relocated_referent(p, actual))
        {
            count += 1;
            if let Some(link) = &mut links[index] {
                link.publication = Publication::Pending;
            }
        } else {
            held.insert(index);
            pin(held, links);
        }
    }
    count
}
