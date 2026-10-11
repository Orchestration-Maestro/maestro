//! Direct native expansion before collectors and canonical deduplication.
#![cfg(target_os = "linux")]
mod mixed_case;
use super::glob_sync;
use maestro_resources::NativeResourceOperations;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
/// One original native setup, not a fake filesystem.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Setup {
    /// Text used by collectors in the companion public cases.
    files: BTreeMap<String, String>,
    /// Empty directories.
    dirs: Vec<String>,
    /// Authored links and targets.
    links: BTreeMap<String, String>,
}
/// Complete result for one direct invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Failure label.
    case: String,
    /// Expansion base.
    cwd: String,
    /// Authored pattern.
    pattern: String,
    /// Ordered authored absolute matches.
    expected: Vec<String>,
}
/// Removes the disposable native tree after the last observation.
struct Scene(PathBuf);
impl Drop for Scene {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
/// Creates a native tree exclusively.
fn setup(root: &Path, setup: Setup) {
    for (file, text) in setup.files {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
    }
    for dir in setup.dirs {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    for (name, target) in setup.links {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, path).unwrap();
        #[cfg(windows)]
        if root.join(&target).is_dir() {
            std::os::windows::fs::symlink_dir(target, path).unwrap();
        } else {
            std::os::windows::fs::symlink_file(target, path).unwrap();
        }
    }
}
/// Every direct observation retains its ordered path identities.
#[cfg(unix)]
#[test]
fn synchronous_walk_matches_native_filesystem_cases() {
    use std::os::unix::fs::PermissionsExt;
    let inputs: Setup = serde_json::from_str(include_str!(
        "../../../tests/fixtures/glob_native_setup.json"
    ))
    .unwrap();
    let rows: Vec<Row> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/glob_native_walk.json"
    ))
    .unwrap();
    let scene =
        Scene(std::env::temp_dir().join(format!("maestro-glob-native-{}", std::process::id())));
    fs::create_dir(&scene.0).unwrap();
    setup(&scene.0, inputs);
    let base = scene.0.to_string_lossy();
    for row in rows {
        if row.case == "denied_read" {
            fs::set_permissions(scene.0.join("denied"), fs::Permissions::from_mode(0o0)).unwrap();
            assert!(fs::read_dir(scene.0.join("denied")).is_err());
        }
        let actual = glob_sync(
            &row.pattern.replace("$SCENE", &base),
            &row.cwd.replace("$SCENE", &base),
            &NativeResourceOperations,
        )
        .unwrap();
        if row.case == "denied_read" {
            fs::set_permissions(scene.0.join("denied"), fs::Permissions::from_mode(0o755)).unwrap();
        }
        let actual: Vec<_> = actual
            .iter()
            .map(|s| s.replace(base.as_ref(), "$SCENE"))
            .collect();
        assert_eq!(actual, row.expected, "{}", row.case);
    }
}

/// Pending literal components use native case policy without changing identities.
#[test]
fn globstar_literals_keep_platform_case_policy() {
    use maestro_pattern::{Minimatch, MinimatchOptions, Platform};
    let scene =
        Scene(std::env::temp_dir().join(format!("maestro-glob-case-{}", std::process::id())));
    fs::create_dir(&scene.0).unwrap();
    for name in ["main.ts", "maint.ts"] {
        fs::write(scene.0.join(name), "fixture").unwrap();
    }
    let root = scene.0.to_str().unwrap();
    for platform in [Platform::Darwin, Platform::Windows, Platform::Posix] {
        let matcher = Minimatch::new("**/MAIN.ts", MinimatchOptions::Filesystem(platform)).unwrap();
        let actual =
            super::walker::walk(&matcher, root, platform, &NativeResourceOperations).unwrap();
        let expected = if platform == Platform::Posix {
            vec![]
        } else {
            vec![format!("{root}/main.ts")]
        };
        assert_eq!(actual, expected);
    }
}
