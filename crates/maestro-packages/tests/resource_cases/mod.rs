//! Recorded resource recipes separated from reusable filesystem witnesses.
#![cfg(test)]
use super::{
    resolution_support::{Filesystem, manager, output},
    scratch::Scratch,
    support,
};
use maestro_packages::{DefaultPackageManager, PackageManager, PackageManagerOptions, SourceScope};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, io, path::PathBuf, rc::Rc};
/// One recorded recipe and its independently observed output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Recipe {
    /// Recipe identity.
    case: String,
    /// Files containing setup text.
    files: BTreeMap<String, String>,
    /// Empty directories.
    dirs: Vec<String>,
    /// Link names and targets.
    links: BTreeMap<String, String>,
    /// Native unreadable directory, when this recipe needs one.
    #[serde(default)]
    denied: Option<String>,
    /// Injected operation failures.
    fail: BTreeMap<String, Vec<String>>,
    /// Explicit source operands.
    sources: Vec<String>,
    /// Selected scope.
    scope: SourceScope,
    /// Complete expected result.
    expected: Value,
}
/// Replay selected recipes through the public manager with completed progress observations.
pub async fn cases(names: &[&str]) {
    let cases: Vec<Recipe> =
        serde_json::from_str(include_str!("../fixtures/resource_resolution.json")).unwrap();
    for name in names {
        let case = cases.iter().find(|c| c.case == *name).unwrap();
        let dir = Scratch::new().unwrap();
        #[cfg(not(unix))]
        if case.denied.is_some() {
            continue;
        }
        setup(case, &dir);
        #[cfg(unix)]
        let _denied = case
            .denied
            .as_ref()
            .map(|path| Denied::new(dir.0.join(path)));
        let base = dir.0.to_string_lossy().into_owned();
        let (_, settings, _) = support::manager(&json!({}));
        let effects = Rc::new(support::Effects {
            native_exists: true,
            resources: Some(Rc::new(Filesystem {
                fail: case.fail.clone(),
                ..Default::default()
            })),
            ..Default::default()
        });
        effects.outputs.borrow_mut().push_back(Ok(support::output(
            &format!("{base}/global_node_modules"),
            "",
            Some(0),
        )));
        let manager = DefaultPackageManager::new(
            PackageManagerOptions {
                cwd: base.clone(),
                agent_dir: format!("{base}/user"),
                settings_manager: settings,
            },
            support::Controlled(Rc::clone(&effects)),
        );
        let events = Rc::new(std::cell::RefCell::new(Vec::new()));
        let observed = Rc::clone(&events);
        manager.set_progress_callback(Some(Rc::new(move |e| {
            observed.borrow_mut().push(e.r#type);
            Ok(())
        })));
        let sources: Vec<_> = case
            .sources
            .iter()
            .map(|s| s.replace("$BASE", &base))
            .collect();
        let actual = manager
            .resolve_extension_sources(&sources, Some(case.scope))
            .await
            .unwrap();
        assert_eq!(output(actual, &base), case.expected, "{}", case.case);
        assert!(events.borrow().is_empty(), "{}", case.case);
        assert_eq!(effects.spawns.get(), 0);
        assert_calls(case, &effects);
    }
}

/// Creates only the filesystem inputs selected by the recipe.
fn setup(case: &Recipe, dir: &Scratch) {
    for (name, text) in &case.files {
        let path = dir.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    for name in &case.dirs {
        std::fs::create_dir_all(dir.0.join(name)).unwrap();
    }
    #[cfg(unix)]
    for (name, target) in &case.links {
        let path = dir.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target, path).unwrap();
    }
    #[cfg(not(unix))]
    assert!(case.links.is_empty());
}

/// Restores a native permission fixture before its directory is dropped.
#[cfg(unix)]
struct Denied(PathBuf);
#[cfg(unix)]
impl Denied {
    /// Removes directory access and independently observes the native failure.
    fn new(path: PathBuf) -> Self {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o0)).unwrap();
        let guard = Self(path);
        assert_eq!(
            std::fs::read_dir(&guard.0).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        guard
    }
}
#[cfg(unix)]
impl Drop for Denied {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// Only user npm lookup consumes the already-owned root command.
fn assert_calls(case: &Recipe, effects: &support::Effects) {
    let expected = if case.case == "installed_npm_user" || case.case == "installed_remote_no_layout"
    {
        vec![("npm".into(), vec!["root".into(), "-g".into()])]
    } else {
        vec![]
    };
    assert_eq!(*effects.calls.borrow(), expected, "{}", case.case);
}

/// Later explicit files cannot overwrite metadata already admitted at the same authored path.
pub async fn first_authored_metadata() {
    let dir = Scratch::new().unwrap();
    let folder = dir.0.join("extensions");
    std::fs::create_dir(&folder).unwrap();
    let path = folder.join("a.ts");
    std::fs::write(&path, "").unwrap();
    let base = dir.0.to_string_lossy().into_owned();
    let source = path.to_string_lossy().into_owned();
    let (manager, _) = manager(&base, Filesystem::default());
    let result = manager
        .resolve_extension_sources(&[base.clone(), source.clone()], None)
        .await
        .unwrap();
    assert_eq!(result.extensions.len(), 1);
    let resource = &result.extensions[0];
    assert_eq!(resource.path, source);
    assert_eq!(resource.metadata.source, base);
    assert_eq!(resource.metadata.base_dir.as_deref(), Some(base.as_str()));
}

/// Relative installed roots consult ambient state only when a manifest operand needs it.
#[cfg(unix)]
pub async fn ambient_entry_resolution() {
    let dir = Scratch::new().unwrap();
    let extensions = dir.0.join("pkg/extensions");
    std::fs::create_dir_all(&extensions).unwrap();
    let entry = extensions.join("file.ts");
    std::fs::write(&entry, "").unwrap();
    let manifest = extensions.join("package.json");
    std::fs::write(
        &manifest,
        json!({"maestro":{"extensions":[entry]}}).to_string(),
    )
    .unwrap();
    let reads = Rc::new(std::cell::Cell::new(0));
    let resources = Filesystem {
        fail_current: true,
        current_reads: Rc::clone(&reads),
        ..Default::default()
    };
    let base = dir.0.to_string_lossy().into_owned();
    let (manager, effects) = manager(&base, resources);
    let current = std::env::current_dir().unwrap();
    let relative = maestro_path::relative(
        &current.to_string_lossy(),
        &base,
        &maestro_path::Cwd {
            current: "",
            drive_directories: &[],
        },
    );
    effects
        .outputs
        .borrow_mut()
        .push_back(Ok(support::output(&relative, "", Some(0))));
    let sources = ["npm:pkg".into()];
    let result = manager
        .resolve_extension_sources(&sources, None)
        .await
        .unwrap();
    assert_eq!(result.extensions.len(), 1);
    assert_eq!(result.extensions[0].path, entry.to_string_lossy());
    assert_eq!(
        reads.get(),
        0,
        "absolute manifest operand avoids ambient observation"
    );
    std::fs::write(manifest, r#"{"maestro":{"extensions":["file.ts"]}}"#).unwrap();
    let error = manager
        .resolve_extension_sources(&sources, None)
        .await
        .err()
        .unwrap();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(
        reads.get(),
        1,
        "relative manifest operand requires ambient observation"
    );
}
