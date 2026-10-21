//! Bootstrap preservation through native effects and public resolution.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::native_support;
use crate::sandbox;
use maestro_packages::PackageManager;
use maestro_resources::{SourceOrigin, SourceScope};
use sandbox::{Fixture, block_on};
use serde::Deserialize;
use std::io;

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

#[test]
fn resolution_creates_exact_bootstrap_files() {
    let temporary = |base: &str| vec![("TMPDIR", Some(base.to_owned()))];
    if crate::resolution_support::case_in_child("temporary", temporary).is_none() {
        return;
    }
    let expected = texts();
    for scope in [SourceScope::Project, SourceScope::Temporary] {
        for existing_root in [false, true] {
            let fixture = Fixture::new(&serde_json::json!({}));
            let root = match scope {
                SourceScope::Project => fixture.project(".maestro/npm"),
                _ => format!(
                    "{}/maestro-extensions/npm/f35b2129",
                    std::env::temp_dir().to_str().unwrap()
                ),
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

/// One error boundary before child admission.
#[cfg(unix)]
#[derive(Clone, Copy)]
enum Boundary {
    /// Recursive directory creation.
    Directory,
    /// The first bootstrap writer.
    Ignore,
}
/// Checks that the operating system's error and the completed effect prefix reach the caller.
#[cfg(unix)]
fn failing_resolution(boundary: Boundary) {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new(&serde_json::json!({"npmCommand":[""]}));
    let root = fixture.project(".maestro/npm");
    let locked = match boundary {
        Boundary::Directory => fixture.project(".maestro"),
        Boundary::Ignore => root.clone(),
    };
    std::fs::create_dir_all(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    let expected = std::fs::create_dir(format!("{locked}/probe")).unwrap_err();
    let error = resolve_source(&fixture, "npm:failure", SourceScope::Project).unwrap_err();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(error.raw_os_error(), expected.raw_os_error());
    assert!(
        fixture.calls().is_empty(),
        "filesystem failure must precede invalid command selection and child admission"
    );
    let exists = std::path::Path::new(&root).exists();
    assert_eq!(exists, matches!(boundary, Boundary::Ignore));
    if exists {
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    }
}

#[test]
fn resolution_propagates_original_bootstrap_failures() {
    #[cfg(unix)]
    for boundary in [Boundary::Directory, Boundary::Ignore] {
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
