//! Bootstrap preservation through native effects and public resolution.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::native_support;
use crate::sandbox;
use crate::scratch;
use maestro_packages::PackageManager;
use maestro_resources::{SourceOrigin, SourceScope};
use maestro_settings::{Settings, SettingsManager};
use sandbox::{Fixture, Op, block_on};
use serde::Deserialize;
use std::{
    io,
    sync::{
        Arc, Barrier,
        mpsc::{Receiver, Sender, channel},
    },
};

/// Bytes deliberately longer and different from either generated bootstrap file.
const SENTINEL: &str = "first creator owns these distinctive bytes, which must remain intact even after the competing resolution completes\n";

/// Complete texts recorded through the source bootstrap helpers.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapFiles {
    /// Ignore text, including its final newline.
    gitignore: String,
    /// Manifest text, without a final newline.
    package_json: String,
}
/// Reads the fixture; both fields are compared as complete strings.
fn texts() -> BootstrapFiles {
    serde_json::from_str(include_str!("fixtures/bootstrap_files.json")).unwrap()
}

/// Installs one deterministic extension fixture into a package directory.
fn extension(base: &str) -> String {
    native_support::write(
        &format!("{base}/package.json"),
        "{\"maestro\":{\"extensions\":[\"entry.ts\"]}}",
    );
    let entry = format!("{base}/entry.ts");
    native_support::write(&entry, "export default () => {};\n");
    entry
}
/// Installs one deterministic extension fixture from an npm child invocation.
fn install_resource(call: &sandbox::Call) -> String {
    extension(&format!(
        "{}/node_modules/{}",
        call.args.last().unwrap(),
        call.args[1]
    ))
}
/// Checks the resource and provenance returned by one public resolution.
fn resolve_source(fixture: &Fixture, source: &str, scope: SourceScope) -> io::Result<String> {
    let result = block_on(
        fixture
            .manager
            .resolve_extension_sources(&[source.into()], Some(scope)),
    )?;
    assert_eq!(result.extensions.len(), 1);
    let entry = &result.extensions[0];
    assert!(entry.enabled);
    assert_eq!(entry.metadata.source, source);
    assert_eq!(entry.metadata.scope, scope);
    assert_eq!(entry.metadata.origin, SourceOrigin::Package);
    assert_eq!(
        entry.metadata.base_dir.as_deref(),
        std::path::Path::new(&entry.path).parent().unwrap().to_str()
    );
    Ok(entry.path.clone())
}

/// One participant's owned paths and synchronization, with no manager crossing threads.
struct Participant {
    /// Shared input and temporary base.
    base: String,
    /// Scope being resolved.
    scope: SourceScope,
    /// Bootstrap file selected for the race.
    filename: &'static str,
    /// Both writers have observed absence before opening.
    barrier: Arc<Barrier>,
    /// A receiver identifies the loser and holds it until the winner's effects finish.
    completion: Option<Receiver<()>>,
    /// Sent after the winning sentinel and resource have been written.
    done: Sender<()>,
}
/// Constructs and drives a manager inside its participant thread.
fn participant(input: Participant) -> String {
    let Participant {
        base,
        scope,
        filename,
        barrier,
        completion,
        done,
    } = input;
    let winner = completion.is_none();
    let fixture = Fixture::over(
        SettingsManager::in_memory(Settings::default()),
        Some(&base),
        false,
    );
    *fixture.script.temp_root.borrow_mut() = Some(base);
    *fixture.script.before_effect.borrow_mut() = Some((
        Op::Write,
        filename.into(),
        Box::new(move |_| {
            barrier.wait();
            if let Some(completion) = completion {
                completion.recv().unwrap();
            }
            Ok(())
        }),
    ));
    fixture
        .script
        .hooks
        .borrow_mut()
        .push_back(Box::new(move |call| {
            let selected = format!("{}/{filename}", call.args.last().unwrap());
            if winner {
                native_support::write(&selected, SENTINEL);
            } else {
                assert_eq!(
                    sandbox::read(&selected),
                    SENTINEL,
                    "loser install must observe winner bytes"
                );
            }
            install_resource(call);
            if winner {
                done.send(()).unwrap();
            }
        }));
    let name = if winner { "winner" } else { "loser" };
    let entry = resolve_source(&fixture, &format!("npm:{name}"), scope)
        .expect("both public resolutions must succeed");
    assert!(entry.ends_with(&format!("/node_modules/{name}/entry.ts")));
    fixture.calls()[0].args.last().unwrap().clone()
}

#[test]
fn concurrent_resolution_preserves_first_creator_bytes() {
    for scope in [SourceScope::Project, SourceScope::Temporary] {
        for filename in [".gitignore", "package.json"] {
            let scratch = scratch::Scratch::new().unwrap();
            let base = scratch.path().to_string_lossy().into_owned();
            let barrier = Arc::new(Barrier::new(2));
            let (done, completed) = channel();
            let threads: Vec<_> = [None, Some(completed)]
                .into_iter()
                .map(|completion| {
                    let input = Participant {
                        base: base.clone(),
                        scope,
                        filename,
                        barrier: barrier.clone(),
                        completion,
                        done: done.clone(),
                    };
                    std::thread::spawn(move || participant(input))
                })
                .collect();
            let joined: Vec<_> = threads
                .into_iter()
                .map(std::thread::JoinHandle::join)
                .collect();
            let roots: Vec<_> = joined.into_iter().map(Result::unwrap).collect();
            assert_eq!(roots[0], roots[1]);
            assert_eq!(
                sandbox::read(&format!("{}/{filename}", roots[0])),
                SENTINEL,
                "final same-file bytes must remain the winner's"
            );
        }
    }
}

#[test]
fn resolution_creates_exact_bootstrap_files() {
    let expected = texts();
    for scope in [SourceScope::Project, SourceScope::Temporary] {
        for existing_root in [false, true] {
            let fixture = Fixture::new(&serde_json::json!({}));
            *fixture.script.temp_root.borrow_mut() = Some(fixture.path("temp"));
            let root = match scope {
                SourceScope::Project => fixture.project(".maestro/npm"),
                _ => format!("{}/maestro-extensions/npm/f35b2129", fixture.path("temp")),
            };
            if existing_root {
                std::fs::create_dir_all(&root).unwrap();
            }
            fixture
                .script
                .hooks
                .borrow_mut()
                .push_back(Box::new(|call| {
                    install_resource(call);
                }));
            let entry = resolve_source(&fixture, "npm:fresh", scope).unwrap();
            assert_eq!(entry, format!("{root}/node_modules/fresh/entry.ts"));
            assert_eq!(
                sandbox::read(&format!("{root}/.gitignore")),
                expected.gitignore
            );
            assert_eq!(
                sandbox::read(&format!("{root}/package.json")),
                expected.package_json
            );
        }
    }
    for scope in [SourceScope::User, SourceScope::Project] {
        let fixture = Fixture::new(&serde_json::json!({}));
        let root = if scope == SourceScope::User {
            fixture.agent("git")
        } else {
            fixture.project(".maestro/git")
        };
        fixture
            .script
            .hooks
            .borrow_mut()
            .push_back(Box::new(|call| {
                extension(call.args.last().unwrap());
            }));
        let entry = resolve_source(&fixture, "https://github.com/team/repo", scope).unwrap();
        assert_eq!(entry, format!("{root}/github.com/team/repo/entry.ts"));
        assert_eq!(
            sandbox::read(&format!("{root}/.gitignore")),
            expected.gitignore
        );
        assert!(!std::path::Path::new(&format!("{root}/package.json")).exists());
    }
}

/// An occupied destination produced by ordinary filesystem users.
#[derive(Clone, Copy, Debug)]
enum Entry {
    /// Distinctive ordinary bytes.
    Text,
    /// An empty ordinary file.
    Empty,
    /// An existing directory.
    Directory,
    /// A link to an existing file.
    #[cfg(unix)]
    Symlink,
    /// A link to a missing file.
    #[cfg(unix)]
    Dangling,
}
/// Enumerates native destination classes, including links on Unix.
fn entries() -> Vec<Entry> {
    vec![
        Entry::Text,
        Entry::Empty,
        Entry::Directory,
        #[cfg(unix)]
        Entry::Symlink,
        #[cfg(unix)]
        Entry::Dangling,
    ]
}
/// Creates an occupied entry and records its original metadata.
fn occupied(path: &str, kind: Entry) -> std::fs::Metadata {
    std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
    match kind {
        Entry::Text => native_support::write(path, SENTINEL),
        Entry::Empty => native_support::write(path, ""),
        Entry::Directory => std::fs::create_dir(path).unwrap(),
        #[cfg(unix)]
        Entry::Symlink | Entry::Dangling => {
            let target = format!("{path}.target");
            if matches!(kind, Entry::Symlink) {
                native_support::write(&target, SENTINEL);
            }
            std::os::unix::fs::symlink(target, path).unwrap();
        }
    }
    std::fs::symlink_metadata(path).unwrap()
}
/// Checks the original entry's identity and complete contents after preservation.
fn preserved(path: &str, kind: Entry, original: &std::fs::Metadata) {
    let current = std::fs::symlink_metadata(path).unwrap();
    assert_eq!(current.file_type(), original.file_type());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(
            (current.dev(), current.ino()),
            (original.dev(), original.ino())
        );
    }
    match kind {
        Entry::Text => assert_eq!(sandbox::read(path), SENTINEL),
        Entry::Empty => assert_eq!(sandbox::read(path), ""),
        Entry::Directory => assert!(std::path::Path::new(path).is_dir()),
        #[cfg(unix)]
        Entry::Symlink | Entry::Dangling => {
            let target = format!("{path}.target");
            assert_eq!(
                std::fs::read_link(path).unwrap(),
                std::path::Path::new(&target)
            );
            if matches!(kind, Entry::Symlink) {
                assert_eq!(sandbox::read(&target), SENTINEL);
            } else {
                assert!(!std::path::Path::new(&target).exists());
            }
        }
    }
}

#[test]
fn resolution_preserves_existing_bootstrap_entries() {
    let expected = texts();
    for filename in [".gitignore", "package.json"] {
        for kind in entries() {
            let fixture = Fixture::new(&serde_json::json!({}));
            let root = fixture.project(".maestro/npm");
            let selected = format!("{root}/{filename}");
            let original = occupied(&selected, kind);
            fixture
                .script
                .hooks
                .borrow_mut()
                .push_back(Box::new(|call| {
                    install_resource(call);
                }));
            let entry = resolve_source(&fixture, "npm:preserved", SourceScope::Project).unwrap();
            assert_eq!(entry, format!("{root}/node_modules/preserved/entry.ts"));
            preserved(&selected, kind, &original);
            let (sibling, text) = if filename == ".gitignore" {
                ("package.json", &expected.package_json)
            } else {
                (".gitignore", &expected.gitignore)
            };
            assert_eq!(sandbox::read(&format!("{root}/{sibling}")), *text);
        }
    }
    let fixture = Fixture::new(&serde_json::json!({}));
    let ignore = fixture.project(".maestro/npm/.gitignore");
    let manifest = fixture.project(".maestro/npm/package.json");
    native_support::write(&ignore, "distinct ignore");
    native_support::write(&manifest, "distinct manifest");
    fixture
        .script
        .hooks
        .borrow_mut()
        .push_back(Box::new(|call| {
            install_resource(call);
        }));
    resolve_source(&fixture, "npm:both", SourceScope::Project).unwrap();
    assert_eq!(sandbox::read(&ignore), "distinct ignore");
    assert_eq!(sandbox::read(&manifest), "distinct manifest");
}

/// Distinguishes the original error payload from a display-only reconstruction.
#[derive(Debug)]
struct Failure(Arc<()>);
impl std::fmt::Display for Failure {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output.write_str("controlled filesystem failure")
    }
}
impl std::error::Error for Failure {}
/// One error boundary before child admission.
#[derive(Clone, Copy)]
enum Boundary {
    /// Recursive directory creation.
    Directory,
    /// The first bootstrap writer.
    Ignore,
    /// Manifest opening or writing.
    Manifest,
    /// Manifest writing after a prefix was published.
    Partial,
}
/// Checks original error ownership and the completed effect prefix.
fn failing_resolution(boundary: Boundary) {
    let fixture = Fixture::new(&serde_json::json!({"npmCommand":[""]}));
    let root = fixture.project(".maestro/npm");
    let identity = Arc::new(());
    let payload = Failure(identity.clone());
    let expected = texts();
    let (operation, suffix) = match boundary {
        Boundary::Directory => (Op::Create, "/npm"),
        Boundary::Ignore => (Op::Write, ".gitignore"),
        Boundary::Manifest | Boundary::Partial => (Op::Write, "package.json"),
    };
    let prefix = expected.package_json[..7].to_owned();
    let partial = prefix.clone();
    *fixture.script.before_effect.borrow_mut() = Some((
        operation,
        suffix.into(),
        Box::new(move |path| {
            if matches!(boundary, Boundary::Partial) {
                native_support::write(path, &partial);
            }
            Err(io::Error::new(io::ErrorKind::PermissionDenied, payload))
        }),
    ));
    let error = resolve_source(&fixture, "npm:failure", SourceScope::Project).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    let original = error
        .get_ref()
        .unwrap()
        .downcast_ref::<Failure>()
        .expect("original payload survives public resolution");
    assert!(Arc::ptr_eq(&original.0, &identity));
    assert!(
        fixture.calls().is_empty(),
        "filesystem failure must precede invalid command selection and child admission"
    );
    prior_effects(&root, boundary, &expected, &prefix);
}

/// Checks the complete filesystem prefix left by a failed bootstrap effect.
fn prior_effects(root: &str, boundary: Boundary, expected: &BootstrapFiles, prefix: &str) {
    let names = if std::path::Path::new(root).exists() {
        let mut names: Vec<_> = std::fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        names
    } else {
        Vec::new()
    };
    let expected_names: Vec<std::ffi::OsString> = match boundary {
        Boundary::Directory | Boundary::Ignore => vec![],
        Boundary::Manifest => vec![".gitignore".into()],
        Boundary::Partial => vec![".gitignore".into(), "package.json".into()],
    };
    assert_eq!(names, expected_names);
    assert_eq!(
        std::path::Path::new(root).exists(),
        !matches!(boundary, Boundary::Directory)
    );
    if matches!(boundary, Boundary::Manifest | Boundary::Partial) {
        assert_eq!(
            sandbox::read(&format!("{root}/.gitignore")),
            expected.gitignore
        );
    }
    if matches!(boundary, Boundary::Partial) {
        assert_eq!(sandbox::read(&format!("{root}/package.json")), prefix);
    }
}

#[test]
fn resolution_propagates_original_bootstrap_failures() {
    for boundary in [
        Boundary::Directory,
        Boundary::Ignore,
        Boundary::Manifest,
        Boundary::Partial,
    ] {
        failing_resolution(boundary);
    }
    let fixture = Fixture::new(&serde_json::json!({"npmCommand":[""]}));
    let root = fixture.project(".maestro/npm");
    native_support::write(&root, "root is a file");
    let expected = std::fs::write(format!("{root}/.gitignore"), "probe").unwrap_err();
    assert!(expected.raw_os_error().is_some());
    let error = resolve_source(&fixture, "npm:failure", SourceScope::Project).unwrap_err();
    assert_eq!(error.raw_os_error(), expected.raw_os_error());
    assert_eq!(sandbox::read(&root), "root is a file");
    assert!(fixture.calls().is_empty());
}

/// Checks replacing mode against each occupied destination class.
#[cfg_attr(not(unix), allow(unused_variables))]
fn replaced(path: &str, kind: Entry, original: &std::fs::Metadata) {
    assert_eq!(sandbox::read(path), "new text");
    #[cfg(unix)]
    if matches!(kind, Entry::Symlink | Entry::Dangling) {
        use std::os::unix::fs::MetadataExt;
        let current = std::fs::symlink_metadata(path).unwrap();
        assert_eq!(
            (current.dev(), current.ino()),
            (original.dev(), original.ino())
        );
        assert_eq!(
            std::fs::read_link(path).unwrap(),
            std::path::Path::new(&format!("{path}.target"))
        );
        assert_eq!(sandbox::read(&format!("{path}.target")), "new text");
    }
}

#[test]
fn native_write_mode_keeps_exclusive_and_replacing_semantics() {
    use maestro_packages::{NativePackageOperations, PackageOperations};
    let operations = NativePackageOperations::new(
        |_| false,
        std::rc::Rc::new(|| false),
        &std::rc::Rc::new(tokio::task::LocalSet::new()),
    );
    for exclusive in [true, false] {
        let scratch = scratch::Scratch::new().unwrap();
        let path = format!("{}/missing", scratch.path().to_string_lossy());
        operations.write_file(&path, "new text", exclusive).unwrap();
        assert_eq!(sandbox::read(&path), "new text");
        let missing_parent = format!("{}/no-parent/entry", scratch.path().to_string_lossy());
        let error = operations
            .write_file(&missing_parent, "new text", exclusive)
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(
            !std::path::Path::new(&missing_parent)
                .parent()
                .unwrap()
                .exists()
        );
        for (index, kind) in entries().into_iter().enumerate() {
            let path = format!("{}/occupied-{index}", scratch.path().to_string_lossy());
            let original = occupied(&path, kind);
            let result = operations.write_file(&path, "new text", exclusive);
            if exclusive {
                assert_eq!(result.unwrap_err().kind(), io::ErrorKind::AlreadyExists);
                preserved(&path, kind, &original);
            } else if matches!(kind, Entry::Directory) {
                assert!(result.is_err());
                preserved(&path, kind, &original);
            } else {
                result.unwrap();
                replaced(&path, kind, &original);
            }
        }
    }
}
