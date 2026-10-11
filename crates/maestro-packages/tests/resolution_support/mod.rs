//! Strict recorded recipes and complete public-result comparisons.
#![cfg(test)]
use super::{scratch::Scratch, support};
use maestro_packages::{
    DefaultPackageManager, PackageManager, PackageManagerOptions, ResolvedPaths, SourceScope,
};
use maestro_resources::{
    NativeResourceOperations,
    skills::{ResourceEntry, ResourceFileType, ResourceOperations},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    rc::Rc,
};
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
/// Native filesystem with selected read/stat failures.
#[derive(Default)]
pub struct Filesystem {
    /// Replacement extension entry sequence.
    pub order: Vec<String>,
    /// Supplied canonicalization witness.
    pub canonical: Option<Canonical>,
    /// Failure suffixes keyed by operation.
    pub fail: BTreeMap<String, Vec<String>>,
}
/// Supplied canonicalization operation.
pub type Canonical = Rc<dyn Fn(&Path) -> io::Result<PathBuf>>;
impl Filesystem {
    /// Whether the recorded operation fails here.
    fn check(&self, operation: &str, path: &Path) -> io::Result<()> {
        if self
            .fail
            .get(operation)
            .is_some_and(|ends| ends.iter().any(|s| path.to_string_lossy().ends_with(s)))
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(())
    }
}
impl ResourceOperations for Filesystem {
    fn exists(&self, path: &Path) -> bool {
        NativeResourceOperations.exists(path)
    }
    fn read_dir(&self, path: &Path) -> io::Result<Vec<ResourceEntry>> {
        self.check("readdirSync", path)?;
        if path.ends_with("extensions") && !self.order.is_empty() {
            return Ok(self
                .order
                .iter()
                .map(|name| ResourceEntry {
                    name: name.into(),
                    file_type: ResourceFileType::File,
                })
                .collect());
        }
        NativeResourceOperations.read_dir(path)
    }
    fn read_file(&self, path: &Path) -> io::Result<String> {
        self.check("readFileSync", path)?;
        NativeResourceOperations.read_file(path)
    }
    fn metadata(&self, path: &Path) -> io::Result<ResourceFileType> {
        self.check("statSync", path)?;
        NativeResourceOperations.metadata(path)
    }
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        if let Some(canonical) = &self.canonical {
            return canonical(path);
        }
        NativeResourceOperations.canonicalize(path)
    }
    fn current_directory(&self) -> io::Result<String> {
        NativeResourceOperations.current_directory()
    }
    fn drive_directories(&self) -> Vec<(char, String)> {
        NativeResourceOperations.drive_directories()
    }
}
/// Compare all returned paths, flags and provenance fields.
pub fn output(result: ResolvedPaths, base: &str) -> Value {
    let encode = |items: Vec<maestro_packages::ResolvedResource>| {
        items.into_iter().map(|r|json!({"path":r.path.replace(base,"$BASE"),"enabled":r.enabled,"metadata":{"source":r.metadata.source.replace(base,"$BASE"),"scope":r.metadata.scope,"origin":r.metadata.origin,"baseDir":r.metadata.base_dir.map(|s|s.replace(base,"$BASE"))}})).collect::<Vec<_>>()
    };
    json!({"extensions":encode(result.extensions),"skills":encode(result.skills),"prompts":encode(result.prompts),"themes":encode(result.themes)})
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

/// Constructs a native-file manager with controlled package effects.
pub fn manager(
    base: &str,
    resources: Filesystem,
) -> (
    DefaultPackageManager<support::Controlled>,
    Rc<support::Effects>,
) {
    let (_, settings, _) = support::manager(&json!({}));
    let effects = Rc::new(support::Effects {
        native_exists: true,
        resources: Some(Rc::new(resources)),
        ..Default::default()
    });
    (
        DefaultPackageManager::new(
            PackageManagerOptions {
                cwd: base.into(),
                agent_dir: base.into(),
                settings_manager: settings,
            },
            support::Controlled(Rc::clone(&effects)),
        ),
        effects,
    )
}

/// Drives boundary-test futures through the native Rust runtime.
pub fn run(operation: impl std::future::Future<Output = io::Result<()>>) -> io::Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(operation)
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
