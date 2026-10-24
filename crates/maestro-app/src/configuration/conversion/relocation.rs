//! Source-retaining relocation and final referent settlement.

#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::{symlink_dir, symlink_file};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs, io,
    path::{Path, PathBuf},
};

#[cfg(test)]
mod tests;

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

/// Indexed dependency changes retained for one synchronous relocation.
struct Batch {
    /// Monotonically retained sources.
    held: Vec<bool>,
    /// Captured reverse edges, built before effects.
    dependants: Vec<Vec<usize>>,
    /// Entries whose selected publication may have changed.
    pending: VecDeque<usize>,
    /// Membership preventing duplicate pending work.
    queued: Vec<bool>,
    /// Last successfully selected binding for each publication.
    selected: Vec<Option<PathBuf>>,
    /// Non-link destinations still expected to move.
    planned: BTreeMap<PathBuf, PathBuf>,
}

impl Batch {
    /// Index the captured batch once and seed preparation.
    fn new(entries: &[PreparedMove], links: &[Option<Link>]) -> Self {
        let mut batch = Self {
            held: vec![false; entries.len()],
            dependants: vec![Vec::new(); entries.len()],
            pending: (0..entries.len()).collect(),
            queued: vec![true; entries.len()],
            selected: vec![None; entries.len()],
            planned: entries
                .iter()
                .filter(|entry| entry.referent.is_none())
                .map(|entry| (entry.path.clone(), entry.target.clone()))
                .collect(),
        };
        for (index, link) in links.iter().enumerate() {
            let Some(link) = link else {
                continue;
            };
            for dependency in &link.dependencies {
                batch.dependants[*dependency].push(index);
            }
        }
        for (index, entry) in entries.iter().enumerate() {
            if entry.referent.is_some() && links[index].is_none() {
                batch.hold(index, entries, links);
            }
        }
        batch
    }

    /// Propagate only newly retained sources and invalidate affected bindings.
    fn hold(&mut self, index: usize, entries: &[PreparedMove], links: &[Option<Link>]) {
        let mut pending = vec![index];
        while let Some(index) = pending.pop() {
            if std::mem::replace(&mut self.held[index], true) {
                continue;
            }
            #[cfg(test)]
            tests::record("held", index);
            if self.planned.remove(&entries[index].path).is_some() {
                self.invalidate(index);
            }
            let Some(link) = &links[index] else {
                continue;
            };
            for dependency in &link.dependencies {
                #[cfg(test)]
                tests::record("held-edge", index);
                pending.push(*dependency);
            }
        }
    }

    /// Queue only captured dependants of a changed planned destination.
    fn invalidate(&mut self, index: usize) {
        for dependant in &self.dependants[index] {
            if !std::mem::replace(&mut self.queued[*dependant], true) {
                self.pending.push_back(*dependant);
            }
        }
    }
}

/// Create an exclusive source-adjacent stage and publish without retiring the source.
fn publish(entry: &PreparedMove, link: &mut Link, referent: &Path) -> io::Result<()> {
    #[cfg(test)]
    tests::record_publication(&entry.path);
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
fn prepare(entries: &[PreparedMove], links: &mut [Option<Link>], batch: &mut Batch) {
    while let Some(index) = batch.pending.pop_front() {
        batch.queued[index] = false;
        if batch.held[index] {
            continue;
        }
        let entry = &entries[index];
        let Some(link) = &mut links[index] else {
            continue;
        };
        let Some(referent) = &entry.referent else {
            continue;
        };
        let selected = relocated_referent(referent, &batch.planned);
        if batch.selected[index].as_ref() == Some(&selected) {
            continue;
        }
        if link.publication == Publication::Published {
            let _ = remove_link_entry(&entry.target);
        }
        link.publication = if link.stable
            && fs::read_link(&entry.path).is_ok_and(|payload| payload.is_absolute())
        {
            Publication::Native
        } else {
            Publication::Pending
        };
        if link.publication == Publication::Native || publish(entry, link, &selected).is_ok() {
            batch.selected[index] = Some(selected);
        } else {
            batch.hold(index, entries, links);
        }
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
    let mut batch = Batch::new(&entries, &links);
    prepare(&entries, &mut links, &mut batch);
    let (actual, count) = move_referents(&entries, &links, &mut batch);
    let settled = settle(&entries, &mut links, &mut batch, &actual);
    discard(&entries, &mut links);
    count + settled
}

/// Move unpinned non-links and record only observed successful destinations.
fn move_referents(
    entries: &[PreparedMove],
    links: &[Option<Link>],
    batch: &mut Batch,
) -> (BTreeMap<PathBuf, PathBuf>, usize) {
    let mut actual = BTreeMap::new();
    let mut count = 0;
    for (i, entry) in entries.iter().enumerate() {
        if entry.referent.is_none() && !batch.held[i] {
            if fs::rename(&entry.path, &entry.target).is_ok()
                && fs::canonicalize(&entry.target).is_ok_and(|p| p == entry.target)
            {
                actual.insert(entry.path.clone(), entry.target.clone());
                count += 1;
            } else {
                batch.hold(i, entries, links);
            }
        }
    }
    (actual, count)
}

/// Verify provisional bindings against actual moves before retiring any link.
fn verify(
    entries: &[PreparedMove],
    links: &[Option<Link>],
    batch: &mut Batch,
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
            if !batch.held[i] && fs::canonicalize(observed).ok() != expected {
                batch.hold(i, entries, links);
            }
        }
    }
}

/// Retire dependants before their dependencies and count each verified destination.
fn settle(
    entries: &[PreparedMove],
    links: &mut [Option<Link>],
    batch: &mut Batch,
    actual: &BTreeMap<PathBuf, PathBuf>,
) -> usize {
    verify(entries, links, batch, actual);
    let mut count = 0;
    let (mut dependants, mut ready) = readiness(links, &batch.held);
    while let Some(index) = ready.pop_front() {
        #[cfg(test)]
        tests::record("ready", index);
        let entry = &entries[index];
        let Some(link) = &links[index] else {
            continue;
        };
        if batch.held[index] {
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
            #[cfg(test)]
            tests::record("retired", index);
            if let Some(link) = &mut links[index] {
                link.publication = Publication::Pending;
            }
        } else {
            batch.hold(index, entries, links);
        }
        for dependency in links[index].iter().flat_map(|link| &link.dependencies) {
            #[cfg(test)]
            tests::record("ready-edge", index);
            dependants[*dependency] -= 1;
            if dependants[*dependency] == 0 && links[*dependency].is_some() {
                ready.push_back(*dependency);
            }
        }
    }
    count
}

/// Count eligible incoming edges and seed dependant-free links once.
fn readiness(links: &[Option<Link>], held: &[bool]) -> (Vec<usize>, VecDeque<usize>) {
    let mut dependants = vec![0usize; links.len()];
    for (i, link) in links.iter().enumerate() {
        if let Some(link) = link
            && !held[i]
        {
            for dependency in &link.dependencies {
                dependants[*dependency] += 1;
            }
        }
    }
    let ready: VecDeque<_> = links
        .iter()
        .enumerate()
        .filter(|(i, link)| link.is_some() && !held[*i] && dependants[*i] == 0)
        .map(|(i, _)| i)
        .collect();
    (dependants, ready)
}
