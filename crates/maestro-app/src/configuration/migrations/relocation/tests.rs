//! Operation counts paired with real relocation results.

use super::*;
use std::cell::RefCell;

thread_local! {
    /// Visits from the current synchronous invocation on this test thread.
    static WORK: RefCell<Vec<(&'static str, usize)>> = const { RefCell::new(Vec::new()) };
    /// Publication attempts from the current invocation.
    static PUBLICATIONS: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
}

/// Record a visit at the operation being measured.
pub(super) fn record(operation: &'static str, index: usize) {
    WORK.with(|work| work.borrow_mut().push((operation, index)));
}

/// Take the current invocation's visits.
#[cfg(unix)]
fn take_work() -> Vec<(&'static str, usize)> {
    WORK.with(|work| std::mem::take(&mut *work.borrow_mut()))
}

/// Invocation-local native filesystem fixture.
#[cfg(unix)]
struct Fixture(maestro_test_directory::Scratch);

#[cfg(unix)]
impl Fixture {
    /// Create distinct source and destination directories with canonical identities.
    fn new() -> Self {
        let root = maestro_test_directory::Scratch::new().unwrap();
        fs::create_dir(root.path().join("old")).unwrap();
        fs::create_dir(root.path().join("new")).unwrap();
        Self(root)
    }

    /// Create one admitted entry, capturing link identity before any effects.
    fn entry(&self, name: &str) -> PreparedMove {
        let path = self.0.path().join("old").join(name);
        PreparedMove {
            referent: fs::read_link(&path)
                .ok()
                .map(|_| fs::canonicalize(&path).unwrap()),
            path,
            target: self.0.path().join("new").join(name),
        }
    }

    /// Create an independently bound relative link.
    fn independent(&self, index: usize) -> PreparedMove {
        let name = format!("alias{index}");
        symlink("../payload", self.0.path().join("old").join(&name)).unwrap();
        self.entry(&name)
    }
}

#[cfg(unix)]
#[cfg(unix)]
#[test]
fn independent_links_use_one_settlement_visit_each() {
    for n in [0, 1, 32, 64] {
        let fixture = Fixture::new();
        fs::write(fixture.0.path().join("payload"), b"original").unwrap();
        let entries = (0..n).map(|i| fixture.independent(i)).collect();
        take_work();
        assert_eq!(relocate(entries), n);
        for i in 0..n {
            let target = fixture.0.path().join("new").join(format!("alias{i}"));
            assert_eq!(fs::read(&target).unwrap(), b"original");
            assert!(fs::symlink_metadata(&target).unwrap().is_symlink());
            assert!(
                fs::symlink_metadata(fixture.0.path().join("old").join(format!("alias{i}")))
                    .is_err()
            );
        }
        let visits = take_work()
            .into_iter()
            .filter(|(op, _)| *op == "ready")
            .collect::<Vec<_>>();
        assert_eq!(
            visits,
            (0..n).map(|i| ("ready", i)).collect::<Vec<_>>(),
            "independent settlement must visit each ready candidate once"
        );
    }
}

/// Record a real publication attempt with its source identity.
pub(super) fn record_publication(path: &Path) {
    PUBLICATIONS.with(|attempts| attempts.borrow_mut().push(path.to_path_buf()));
}

/// Drain publication attempts between invocations.
#[cfg(unix)]
fn take_publications() -> Vec<PathBuf> {
    PUBLICATIONS.with(|attempts| std::mem::take(&mut *attempts.borrow_mut()))
}

#[cfg(unix)]
#[test]
fn blocked_stages_leave_unaffected_publications_in_place() {
    for n in [32, 64] {
        let fixture = Fixture::new();
        fs::write(fixture.0.path().join("payload"), b"original").unwrap();
        let entries = (0..2 * n)
            .map(|i| fixture.independent(i))
            .collect::<Vec<_>>();
        for entry in &entries[n..] {
            fs::write(entry.path.with_extension("migrating"), b"occupied").unwrap();
        }
        take_publications();
        assert_eq!(relocate(entries), n);
        let attempts = take_publications();
        for i in 0..2 * n {
            let source = fixture.0.path().join("old").join(format!("alias{i}"));
            assert_independent(&fixture, i, i < n);
            assert_eq!(
                attempts.iter().filter(|path| **path == source).count(),
                1,
                "each independent publication must be attempted once"
            );
        }
        let retry = (n..2 * n)
            .map(|i| {
                let entry = fixture.entry(&format!("alias{i}"));
                fs::remove_file(entry.path.with_extension("migrating")).unwrap();
                entry
            })
            .collect();
        assert_eq!(relocate(retry), n);
        for i in n..2 * n {
            assert_eq!(
                fs::read(fixture.0.path().join("new").join(format!("alias{i}"))).unwrap(),
                b"original"
            );
            assert!(
                fs::symlink_metadata(fixture.0.path().join("old").join(format!("alias{i}")))
                    .is_err()
            );
        }
    }
}

#[cfg(unix)]
impl Fixture {
    /// Create a link within the captured source directory.
    fn link(&self, name: &str, payload: &str) {
        symlink(payload, self.0.path().join("old").join(name)).unwrap();
    }

    /// Read admitted entries in the requested encounter order.
    fn entries(&self, names: &[&str]) -> Vec<PreparedMove> {
        names.iter().map(|name| self.entry(name)).collect()
    }
}

#[cfg(unix)]
#[test]
fn held_changes_are_processed_once() {
    for n in [1, 32, 64] {
        let fixture = Fixture::new();
        fs::write(fixture.0.path().join("payload"), b"original").unwrap();
        let entries = (0..n).map(|i| fixture.independent(i)).collect::<Vec<_>>();
        let links = capture(&entries);
        let mut batch = Batch::new(&entries, &links);
        take_work();
        for i in 0..n {
            batch.hold(i, &entries, &links);
            batch.hold(i, &entries, &links);
        }
        assert_eq!(batch.held, vec![true; n]);
        assert_eq!(take_work(), (0..n).map(|i| ("held", i)).collect::<Vec<_>>());
        for entry in entries {
            assert_eq!(fs::read(entry.path).unwrap(), b"original");
        }
    }
    let fixture = Fixture::new();
    fs::write(fixture.0.path().join("old/file"), b"shared").unwrap();
    fixture.link("middle", "file");
    fixture.link("a", "middle");
    fixture.link("b", "middle");
    let entries = fixture.entries(&["a", "b", "middle", "file"]);
    let links = capture(&entries);
    let mut batch = Batch::new(&entries, &links);
    take_work();
    for i in [0, 1, 0, 2, 3] {
        batch.hold(i, &entries, &links);
    }
    assert_eq!(batch.held, [true; 4]);
    let work = take_work();
    let mut held = work
        .iter()
        .filter(|(op, _)| *op == "held")
        .map(|(_, i)| *i)
        .collect::<Vec<_>>();
    held.sort_unstable();
    assert_eq!(held, [0, 1, 2, 3]);
    assert_eq!(work.iter().filter(|(op, _)| *op == "held-edge").count(), 5);
    for entry in entries {
        assert_eq!(fs::read(entry.path).unwrap(), b"shared");
    }
}

#[cfg(unix)]
#[test]
fn changed_referents_invalidate_only_their_publications() {
    for mut names in [
        vec!["unaffected", "changed", "blocked", "f", "g"],
        vec!["unaffected", "changed", "blocked", "f", "g", "middle"],
        vec!["unaffected", "changed", "blocked", "f", "g", "blocked2"],
        vec![
            "unaffected",
            "changed",
            "blocked",
            "f",
            "g",
            "middle",
            "blocked2",
        ],
    ] {
        changed_bindings(&names);
        names.reverse();
        changed_bindings(&names);
    }
}

/// Exercise affected and unaffected bindings through the same captured batch.
#[cfg(unix)]
fn changed_bindings(names: &[&str]) {
    let shared = names.contains(&"blocked2");
    let intermediate = names.contains(&"middle");
    let reverse = names[0] != "unaffected";
    let fixture = Fixture::new();
    fs::write(fixture.0.path().join("old/f"), b"retained-f").unwrap();
    fs::write(fixture.0.path().join("old/g"), b"moved-g").unwrap();
    fixture.link("unaffected", "g");
    fixture.link("changed", "f");
    fixture.link("middle", "f");
    fixture.link("blocked", if intermediate { "middle" } else { "f" });
    fixture.link("blocked2", "f");
    let entries = fixture.entries(names);
    for name in if shared {
        &["blocked", "blocked2"][..]
    } else {
        &["blocked"][..]
    } {
        fs::write(
            fixture
                .0
                .path()
                .join("old")
                .join(name)
                .with_extension("migrating"),
            b"occupied",
        )
        .unwrap();
    }
    take_publications();
    assert_eq!(relocate(entries), 3);
    let attempts = take_publications();
    assert_changed_bindings(&fixture, &attempts, reverse);
    assert_eq!(
        fs::read(fixture.0.path().join("old/blocked")).unwrap(),
        b"retained-f"
    );
    assert_eq!(
        fs::read(fixture.0.path().join("old/blocked.migrating")).unwrap(),
        b"occupied"
    );
    if intermediate {
        assert_eq!(
            fs::read(fixture.0.path().join("old/middle")).unwrap(),
            b"retained-f"
        );
    }
}

#[cfg(unix)]
#[test]
fn dependency_readiness_tracks_fan_in_and_fan_out() {
    for names in [
        vec!["a", "middle", "file"],
        vec!["a", "b", "file"],
        vec!["a", "b", "middle", "file"],
    ] {
        for reverse in [false, true] {
            let mut names = names.clone();
            if reverse {
                names.reverse();
            }
            for blocked in [false, true] {
                ready_dependencies(&names, blocked);
            }
        }
    }
    let fixture = Fixture::new();
    fixture.link("dangling", "missing");
    assert_eq!(
        relocate(vec![PreparedMove {
            path: fixture.0.path().join("old/dangling"),
            target: fixture.0.path().join("new/dangling"),
            referent: Some(fixture.0.path().join("old/missing")),
        }]),
        0
    );
    assert_eq!(
        fs::read_link(fixture.0.path().join("old/dangling")).unwrap(),
        Path::new("missing")
    );
    assert!(fs::symlink_metadata(fixture.0.path().join("new/dangling")).is_err());
}

/// Observe source retention, edge consumption and dependant-first retirement together.
#[cfg(unix)]
fn ready_dependencies(names: &[&str], blocked: bool) {
    let fixture = Fixture::new();
    fs::write(fixture.0.path().join("old/file"), b"shared").unwrap();
    let intermediate = names.contains(&"middle");
    fixture.link("a", if intermediate { "middle" } else { "file" });
    if names.contains(&"b") {
        fixture.link("b", if intermediate { "middle" } else { "file" });
    }
    if intermediate {
        fixture.link("middle", "file");
    }
    let entries = fixture.entries(names);
    let links = capture(&entries);
    if blocked {
        fs::write(fixture.0.path().join("old/a.migrating"), b"occupied").unwrap();
    }
    take_work();
    assert_eq!(
        relocate(entries),
        if blocked {
            usize::from(names.contains(&"b"))
        } else {
            names.len()
        }
    );
    let work = take_work();
    let retired = work
        .iter()
        .filter(|(op, _)| *op == "retired")
        .map(|(_, i)| *i)
        .collect::<Vec<_>>();
    let expected_edges = match (intermediate, names.contains(&"b"), blocked) {
        (true, true, false) => 5,
        (true, false, false) => 3,
        (true, true, true) | (false, true, false) => 2,
        (false, true, true) => 1,
        _ => 0,
    };
    assert_eq!(
        work.iter().filter(|(op, _)| *op == "ready-edge").count(),
        expected_edges
    );
    assert_retirement_order(&links, &retired);
    assert_ready_files(&fixture, names, blocked);
}

/// Check successful or retained independent entries without hiding attempt counts.
#[cfg(unix)]
fn assert_independent(fixture: &Fixture, index: usize, successful: bool) {
    let name = format!("alias{index}");
    let source = fixture.0.path().join("old").join(&name);
    if successful {
        assert!(fs::symlink_metadata(&source).is_err());
        assert_eq!(
            fs::read(fixture.0.path().join("new").join(&name)).unwrap(),
            b"original"
        );
        assert!(fs::symlink_metadata(source.with_extension("migrating")).is_err());
    } else {
        assert_eq!(fs::read(&source).unwrap(), b"original");
        assert_eq!(
            fs::read(source.with_extension("migrating")).unwrap(),
            b"occupied"
        );
    }
}

/// Check selected identities and per-entry attempts after the completed batch.
#[cfg(unix)]
fn assert_changed_bindings(fixture: &Fixture, attempts: &[PathBuf], reverse: bool) {
    for (name, identity, bytes, count) in [
        ("unaffected", "new/g", &b"moved-g"[..], 1),
        (
            "changed",
            "old/f",
            &b"retained-f"[..],
            if reverse { 1 } else { 2 },
        ),
    ] {
        let target = fixture.0.path().join("new").join(name);
        assert_eq!(
            fs::canonicalize(&target).unwrap(),
            fixture.0.path().join(identity)
        );
        assert_eq!(fs::read(&target).unwrap(), bytes);
        let source = fixture.0.path().join("old").join(name);
        assert!(fs::symlink_metadata(&source).is_err());
        assert_eq!(
            attempts.iter().filter(|path| **path == source).count(),
            count
        );
        assert!(fs::symlink_metadata(source.with_extension("migrating")).is_err());
    }
}

/// Check bytes, identity, link kind and source retirement for each admitted entry.
#[cfg(unix)]
fn assert_ready_files(fixture: &Fixture, names: &[&str], blocked: bool) {
    for name in names {
        let retained = blocked && *name != "b";
        let path = fixture
            .0
            .path()
            .join(if retained { "old" } else { "new" })
            .join(name);
        assert_eq!(fs::read(&path).unwrap(), b"shared");
        assert_eq!(
            fs::canonicalize(&path).unwrap(),
            fixture
                .0
                .path()
                .join(if blocked { "old/file" } else { "new/file" })
        );
        if !retained {
            assert!(fs::symlink_metadata(fixture.0.path().join("old").join(name)).is_err());
        }
        if *name != "file" {
            assert!(fs::symlink_metadata(path).unwrap().is_symlink());
        }
    }
}

/// Assert ordering only where both links actually retired.
#[cfg(unix)]
fn assert_retirement_order(links: &[Option<Link>], retired: &[usize]) {
    for (index, link) in links.iter().enumerate() {
        let Some(link) = link else {
            continue;
        };
        for dependency in &link.dependencies {
            if let (Some(first), Some(second)) = (
                retired.iter().position(|j| *j == index),
                retired.iter().position(|j| j == dependency),
            ) {
                assert!(first < second, "dependants retire before dependencies");
            }
        }
    }
}
