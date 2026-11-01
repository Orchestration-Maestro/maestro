//! Recorded resource recipes separated from reusable filesystem witnesses.
#![cfg(test)]
#[cfg(unix)]
use super::resolution_support::manager;
use super::{resolution_support::recorded_output, scratch::Scratch, support};
use maestro_packages::{DefaultPackageManager, PackageManager, PackageManagerOptions, SourceScope};
use serde::Deserialize;
use serde_json::{Value, json};
#[cfg(unix)]
use std::io;
use std::{collections::BTreeMap, rc::Rc};
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
        let Some(dir) = admit(case) else {
            continue;
        };
        setup(case, &dir);
        // A privileged run reads denied entries anyway, so such a case has nothing to check.
        #[cfg(unix)]
        let Some(_denied) = case
            .denied
            .iter()
            .map(|path| maestro_test_directory::deny(dir.path().join(path), 0o0).unwrap())
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let base = dir.path().to_string_lossy().into_owned();
        let (_, settings, _) = support::manager(&json!({}));
        let effects = Rc::new(support::Effects::default());
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
        assert_eq!(
            recorded_output(actual, &base),
            case.expected,
            "{}",
            case.case
        );
        assert!(events.borrow().is_empty(), "{}", case.case);
        assert_eq!(effects.spawns.get(), 0);
        assert_calls(case, &effects);
    }
}

/// A fixture directory for one recipe, or `None` when this host cannot store the names the
/// recipe authors, with the folded names reported.
fn admit(case: &Recipe) -> Option<Scratch> {
    #[cfg(not(unix))]
    if !case.denied.is_empty() {
        return None;
    }
    let dir = Scratch::new().unwrap();
    let paths: Vec<&str> = case
        .files
        .keys()
        .chain(&case.dirs)
        .chain(case.links.keys())
        .map(String::as_str)
        .collect();
    let folded = dir.folded_names(&paths).unwrap();
    if folded.is_empty() {
        return Some(dir);
    }
    eprintln!(
        "{} needs names this host keeps apart: {folded:?}",
        case.case
    );
    None
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
