//! Recorded resource recipes separated from reusable filesystem witnesses.
#![cfg(test)]
#[cfg(unix)]
use super::resolution_support::manager;
use super::{resolution_support::output, scratch::Scratch, support};
use maestro_packages::{DefaultPackageManager, PackageManager, PackageManagerOptions, SourceScope};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, rc::Rc};
#[cfg(unix)]
use std::{io, path::PathBuf};
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
    /// Native files and directories nobody may read, when this recipe needs them.
    #[serde(default)]
    denied: Vec<String>,
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
        if !case.denied.is_empty() {
            continue;
        }
        setup(case, &dir);
        #[cfg(unix)]
        let _denied: Vec<_> = case
            .denied
            .iter()
            .map(|path| Denied::new(dir.path().join(path)))
            .collect();
        let base = dir.path().to_string_lossy().into_owned();
        let (_, settings, _) = support::manager(&json!({}));
        let effects = Rc::new(support::Effects {
            native_exists: true,
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
        let path = dir.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    for name in &case.dirs {
        std::fs::create_dir_all(dir.path().join(name)).unwrap();
    }
    #[cfg(unix)]
    for (name, target) in &case.links {
        let path = dir.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target, path).unwrap();
    }
    #[cfg(not(unix))]
    assert!(case.links.is_empty());
}

/// Restores a native permission fixture before its directory is dropped.
#[cfg(unix)]
pub struct Denied(PathBuf);
#[cfg(unix)]
impl Denied {
    /// Removes all access to a file or directory and independently observes the native
    /// failure.
    pub fn new(path: PathBuf) -> Self {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o0)).unwrap();
        let guard = Self(path);
        let error = if guard.0.is_dir() {
            std::fs::read_dir(&guard.0).err()
        } else {
            std::fs::read(&guard.0).err()
        };
        assert_eq!(error.unwrap().kind(), io::ErrorKind::PermissionDenied);
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
#[cfg(unix)]
pub async fn first_authored_metadata() {
    let dir = Scratch::new().unwrap();
    let folder = dir.path().join("extensions");
    std::fs::create_dir(&folder).unwrap();
    let path = folder.join("a.ts");
    std::fs::write(&path, "").unwrap();
    let base = dir.path().to_string_lossy().into_owned();
    let source = path.to_string_lossy().into_owned();
    let (manager, _) = manager(&base);
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

/// Relative installed roots consult ambient state only when a manifest operand needs it;
/// `process` is the unreadable working directory.
#[cfg(unix)]
pub async fn ambient_entry_resolution(process: &std::path::Path) {
    let dir = Scratch::new().unwrap();
    let extensions = dir.path().join("pkg/extensions");
    std::fs::create_dir_all(&extensions).unwrap();
    let entry = extensions.join("file.ts");
    std::fs::write(&entry, "").unwrap();
    let manifest = extensions.join("package.json");
    std::fs::write(
        &manifest,
        json!({"maestro":{"extensions":[entry]}}).to_string(),
    )
    .unwrap();
    let base = dir.path().to_string_lossy().into_owned();
    let (manager, effects) = manager(&base);
    let relative = maestro_path::relative(
        &process.to_string_lossy(),
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
    std::fs::write(manifest, r#"{"maestro":{"extensions":["file.ts"]}}"#).unwrap();
    let error = manager
        .resolve_extension_sources(&sources, None)
        .await
        .err()
        .unwrap();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}
