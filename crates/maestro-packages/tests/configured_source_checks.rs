//! Configured sources through the supplied-operation manager.
mod support;
use maestro_packages::PackageManager;
use maestro_packages::{DefaultPackageManager, PackageManagerOptions};
use maestro_settings::{Settings, SettingsManager};
use serde_json::json;
use std::{cell::RefCell, rc::Rc};
use support::{Controlled, Effects};

#[test]
fn construction_stores_dependencies_without_observing_them() {
    let settings = Rc::new(RefCell::new(
        SettingsManager::in_memory(Settings::default()),
    ));
    let held = settings.borrow_mut();
    let effects = Rc::new(Effects::default());
    effects.fail_ambient.set(true);
    let mut manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: "relative".into(),
            agent_dir: "relative-agent".into(),
            settings_manager: settings.clone(),
        },
        Controlled(effects.clone()),
    );
    assert!(effects.reads.borrow().is_empty());
    assert!(effects.calls.borrow().is_empty());
    assert!(effects.paths.borrow().is_empty());
    assert_eq!(
        manager
            .add_source_to_settings("npm:pkg", None)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::Other
    );
    drop(held);
}
#[test]
#[cfg(unix)]
fn source_classification_preserves_npm_and_local_precedence() {
    let rows: Vec<SourceVector> =
        serde_json::from_str(include_str!("source_vectors.json")).unwrap();
    let (mut manager, _, effects) = support::manager(&json!({}));
    effects.exists.set(true);
    let mut visited = std::collections::HashSet::new();
    for row in rows {
        assert!(visited.insert(row.input.clone()), "duplicate query");
        assert_eq!(
            manager
                .get_installed_path(&row.input, maestro_packages::InstalledSourceScope::Project)
                .unwrap(),
            Some(row.expected),
            "{}",
            row.input
        );
    }
    assert_eq!(visited.len(), 26);
}
#[test]
#[cfg(unix)]
fn scoped_local_storage_and_matching_use_different_bases() {
    use maestro_packages::InstalledSourceScope::{Project, User};
    let rows: Vec<NormalizationVector> =
        serde_json::from_str(include_str!("normalization_vectors.json")).unwrap();
    let mut visited = std::collections::HashSet::new();
    for row in rows {
        assert!(visited.insert((row.input.clone(), row.scope.clone())));
        let scope = match row.scope.as_str() {
            "user" => User,
            "project" => Project,
            _ => panic!("unknown scope"),
        };
        let (mut manager, settings, _) = support::manager(&json!({}));
        assert!(
            manager
                .add_source_to_settings(&row.input, Some(scope))
                .unwrap()
        );
        let stored = match scope {
            User => settings.borrow().get_global_settings(),
            Project => settings.borrow().get_project_settings(),
        };
        assert_eq!(stored.0["packages"], json!([row.expected]), "{}", row.input);
        assert!(
            !manager
                .add_source_to_settings(&row.input, Some(scope))
                .unwrap()
        );
    }
    assert_eq!(visited.len(), 26);
    let (mut manager, settings, _) = support::manager(&json!({}));
    assert!(
        manager
            .add_source_to_settings("/work/project/.maestro", Some(Project))
            .unwrap()
    );
    assert_eq!(
        settings.borrow().get_project_settings().0["packages"],
        json!(["."])
    );
}
#[test]
fn scoped_changes_preserve_filtered_objects_and_raw_spelling() {
    let original = json!({"source":"npm:keep", "skills":7, "unknown":{"x":1}});
    let (mut manager, settings, _) = support::manager(&json!({"packages":[original]}));
    assert!(
        manager
            .add_source_to_settings("npm: pkg@latest ", None)
            .unwrap()
    );
    assert!(!manager.add_source_to_settings("npm:pkg@2", None).unwrap());
    assert!(
        manager
            .add_source_to_settings(
                "npm:pkg",
                Some(maestro_packages::InstalledSourceScope::Project)
            )
            .unwrap()
    );
    assert!(
        !manager
            .remove_source_from_settings("npm:missing", None)
            .unwrap()
    );
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!([original, "npm: pkg@latest "])
    );
    assert_eq!(
        settings.borrow().get_project_settings().0["packages"],
        json!(["npm:pkg"])
    );
}
#[test]
fn git_transports_match_one_identity_without_rewriting() {
    let (mut manager, settings, _) = support::manager(
        &json!({"packages":["git:git@github.com:u/r@old", {"source":"https://github.com/u/r.git","skills":[]}, "https://github.com/v/r"]}),
    );
    assert!(
        !manager
            .add_source_to_settings("ssh://git@github.com/u/r@new", None)
            .unwrap()
    );
    assert!(
        !manager
            .add_source_to_settings("git:github.com/u/r", None)
            .unwrap()
    );
    assert!(
        !manager
            .add_source_to_settings("https://github.com/u/r@v1", None)
            .unwrap()
    );
    assert!(
        manager
            .remove_source_from_settings("https://github.com/u/r", None)
            .unwrap()
    );
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!(["https://github.com/v/r"])
    );
}
#[test]
#[cfg(unix)]
fn local_input_and_stored_paths_match_in_their_own_scopes() {
    let (mut manager, settings, _) = support::manager(
        &json!({"packages":["../../../work/project/pkg", {"source":"../../../work/project/pkg","skills":null},"./local"]}),
    );
    assert!(!manager.add_source_to_settings("./pkg", None).unwrap());
    assert!(manager.remove_source_from_settings("./pkg/", None).unwrap());
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!(["./local"])
    );
    assert!(
        manager
            .add_source_to_settings("/home/reader/agent", None)
            .unwrap()
    );
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!(["./local", "."])
    );
    let retained = json!(["../../work/project/pkg","./local",{"source":"../../work/project/pkg","skills":null}]);
    let (mut manager, settings, _) = support::manager(&json!({"packages":retained}));
    assert!(manager.add_source_to_settings("./pkg", None).unwrap());
    assert!(manager.remove_source_from_settings("./pkg/", None).unwrap());
    assert!(
        manager
            .add_source_to_settings("/home/reader/agent", None)
            .unwrap()
    );
    assert!(
        manager
            .add_source_to_settings(
                "./.maestro",
                Some(maestro_packages::InstalledSourceScope::Project)
            )
            .unwrap()
    );
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!(["../../work/project/pkg","./local",{"source":"../../work/project/pkg","skills":null},"."])
    );
    assert_eq!(
        settings.borrow().get_project_settings().0["packages"],
        json!(["."])
    );
}
#[test]
#[cfg(unix)]
fn configured_listing_retains_order_duplicates_and_object_flags() {
    use maestro_packages::InstalledSourceScope::{Project, User};
    let (mut manager, settings, effects) = support::manager(
        &json!({"packages":["./local", {"source":"https://github.com/u/r","skills":null}, "./local"]}),
    );
    settings.borrow_mut().set_project_packages(vec![
        maestro_settings::PackageSource::Source("./local".into()),
        maestro_settings::PackageSource::Source("https://github.com/u/r".into()),
    ]);
    effects.exists.set(true);
    let rows = manager.list_configured_packages().unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| (r.source.as_str(), r.scope, r.filtered))
            .collect::<Vec<_>>(),
        vec![
            ("./local", User, false),
            ("https://github.com/u/r", User, true),
            ("./local", User, false),
            ("./local", Project, false),
            ("https://github.com/u/r", Project, false)
        ]
    );
    assert_eq!(
        rows.iter()
            .map(|r| r.installed_path.as_deref())
            .collect::<Vec<_>>(),
        vec![
            Some("/home/reader/agent/local"),
            Some("/home/reader/agent/git/github.com/u/r"),
            Some("/home/reader/agent/local"),
            Some("/work/project/.maestro/local"),
            Some("/work/project/.maestro/git/github.com/u/r")
        ]
    );
    assert_eq!(effects.paths.borrow().len(), 5);
    snapshot_before_lookup().unwrap();
}
#[test]
fn raw_package_errors_follow_the_consumed_entry() {
    for root in [json!(null), json!([])] {
        let (mut manager, _, _) = support::manager(&json!({"packages":root}));
        assert!(manager.list_configured_packages().unwrap().is_empty());
        assert!(
            !manager
                .remove_source_from_settings("npm:pkg", None)
                .unwrap()
        );
        assert!(manager.add_source_to_settings("npm:pkg", None).unwrap());
    }
    for root in [json!({}), json!(false), json!(42), json!(""), json!("abc")] {
        rejects_packages(&root);
    }
    for entry in [
        json!(null),
        json!({}),
        json!({"source":null}),
        json!({"source":4}),
    ] {
        rejects_packages(&json!([entry]));
    }
    rejects_packages(&json!([{}, "npm:pkg"]));
    let retained = json!({"source":"npm:pkg","skills":4,"unknown":[1]});
    let (mut manager, settings, _) = support::manager(&json!({"packages":[retained]}));
    assert!(!manager.add_source_to_settings("npm:pkg", None).unwrap());
    assert!(manager.list_configured_packages().unwrap()[0].filtered);
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!([retained])
    );
    assert!(
        manager
            .remove_source_from_settings("npm:pkg", None)
            .unwrap()
    );
    let (mut manager, _, _) = support::manager(&json!({"packages":["npm:pkg",{}]}));
    assert!(!manager.add_source_to_settings("npm:pkg", None).unwrap());
    assert!(
        manager
            .remove_source_from_settings("npm:pkg", None)
            .is_err()
    );
}
#[test]
#[cfg(unix)]
fn later_bad_entries_preserve_earlier_root_cache_effects() {
    use maestro_settings::PackageSource;
    let (mut manager, settings, effects) = support::manager(&json!({"packages":["npm:first",{}]}));
    assert!(manager.list_configured_packages().is_err());
    assert_eq!(
        *effects.calls.borrow(),
        vec![("npm".into(), vec!["root".into(), "-g".into()])]
    );
    settings
        .borrow_mut()
        .set_packages(vec![PackageSource::Source("npm:second".into())]);
    let rows = manager.list_configured_packages().unwrap();
    assert_eq!(rows[0].source, "npm:second");
    assert_eq!(effects.calls.borrow().len(), 1);
    assert_eq!(*effects.paths.borrow(), vec!["/root/first", "/root/second"]);
}
#[test]
#[cfg(unix)]
fn local_resolution_reads_home_and_cwd_only_when_needed() {
    use maestro_packages::InstalledSourceScope::User;
    let (mut manager, settings, effects) = support::manager(&json!({}));
    effects.fail_ambient.set(true);
    effects.exists.set(true);
    assert_eq!(
        manager.get_installed_path("/absolute/pkg", User).unwrap(),
        Some("/absolute/pkg".into())
    );
    assert_eq!(
        manager.get_installed_path("./pkg", User).unwrap(),
        Some("/home/reader/agent/pkg".into())
    );
    assert!(!manager.remove_source_from_settings("~", None).unwrap());
    assert!(effects.reads.borrow().is_empty());
    assert_eq!(
        manager
            .get_installed_path("~", User)
            .unwrap_err()
            .to_string(),
        "home unavailable"
    );
    assert_eq!(
        manager
            .add_source_to_settings("~", None)
            .unwrap_err()
            .to_string(),
        "home unavailable"
    );
    assert!(
        !settings
            .borrow()
            .get_global_settings()
            .0
            .contains_key("packages")
    );
    let mut relative = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: "relative".into(),
            agent_dir: "relative-agent".into(),
            settings_manager: settings,
        },
        Controlled(effects.clone()),
    );
    assert_eq!(
        relative
            .get_installed_path("./pkg", User)
            .unwrap_err()
            .to_string(),
        "cwd unavailable"
    );
    assert_eq!(*effects.reads.borrow(), vec!["home", "home", "cwd"]);
    relative_home_storage_needs_cwd().unwrap();
}
#[test]
#[cfg(unix)]
fn installed_paths_require_existing_contents_in_the_selected_scope() {
    use maestro_packages::InstalledSourceScope::{Project, User};
    for (source, scope, expected) in [
        ("npm:@scope/pkg", User, "/root/@scope/pkg"),
        (
            "npm:@scope/pkg",
            Project,
            "/work/project/.maestro/npm/node_modules/@scope/pkg",
        ),
        (
            "https://github.com/u/r",
            User,
            "/home/reader/agent/git/github.com/u/r",
        ),
        (
            "https://github.com/u/r",
            Project,
            "/work/project/.maestro/git/github.com/u/r",
        ),
        ("./local", User, "/home/reader/agent/local"),
        ("./local", Project, "/work/project/.maestro/local"),
    ] {
        let (mut manager, _, effects) = support::manager(&json!({}));
        assert_eq!(manager.get_installed_path(source, scope).unwrap(), None);
        effects.exists.set(true);
        assert_eq!(
            manager.get_installed_path(source, scope).unwrap(),
            Some(expected.into())
        );
        assert_eq!(*effects.paths.borrow(), vec![expected, expected]);
    }
}
/// Exercises one operation sequence against any supplied adapter.
#[cfg(unix)]
fn workflow(
    operations: impl maestro_packages::PackageOperations,
    cwd: &str,
    agent: &str,
    root: &str,
) -> std::io::Result<()> {
    use maestro_packages::InstalledSourceScope::{Project, User};
    let settings = Rc::new(RefCell::new(SettingsManager::in_memory(Settings(
        json!({"npmCommand":["/bin/sh","-c",format!("printf '%s' '{root}'"),"--"]})
            .as_object()
            .cloned()
            .unwrap_or_default(),
    ))));
    let mut manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: cwd.into(),
            agent_dir: agent.into(),
            settings_manager: settings.clone(),
        },
        operations,
    );
    assert!(manager.add_source_to_settings("./local", None)?);
    assert!(manager.add_source_to_settings("npm:@scope/pkg", None)?);
    assert!(manager.add_source_to_settings("https://github.com/u/r", Some(Project))?);
    assert!(!manager.add_source_to_settings("ssh://git@github.com/u/r", Some(Project))?);
    let rows = manager.list_configured_packages()?;
    assert_eq!(
        rows.iter()
            .map(|r| (r.source.as_str(), r.scope))
            .collect::<Vec<_>>(),
        vec![
            ("../project/local", User),
            ("npm:@scope/pkg", User),
            ("https://github.com/u/r", Project)
        ]
    );
    assert_eq!(
        rows.iter()
            .map(|r| r.installed_path.as_deref())
            .collect::<Vec<_>>(),
        vec![
            None,
            Some(maestro_path::join(&[root, "@scope/pkg"]).as_str()),
            Some(maestro_path::join(&[cwd, ".maestro/git/github.com/u/r"]).as_str())
        ]
    );
    assert!(manager.remove_source_from_settings("git:git@github.com:u/r", Some(Project))?);
    assert_eq!(
        manager
            .list_configured_packages()?
            .iter()
            .map(|r| r.source.as_str())
            .collect::<Vec<_>>(),
        vec!["../project/local", "npm:@scope/pkg"]
    );
    assert_eq!(
        settings.borrow().get_project_settings().0["packages"],
        json!([])
    );
    Ok(())
}
#[cfg(unix)]
#[test]
fn configured_sources_work_without_acquisition() {
    use maestro_packages::NativePackageOperations;
    let scratch = Scratch::new().unwrap();
    let base = scratch.0.to_str().unwrap();
    let cwd = maestro_path::join(&[base, "project"]);
    let agent = maestro_path::join(&[base, "agent"]);
    let root = maestro_path::join(&[base, "global"]);
    std::fs::create_dir_all(maestro_path::join(&[&cwd, ".maestro/git/github.com/u/r"])).unwrap();
    std::fs::create_dir_all(maestro_path::join(&[&root, "@scope"])).unwrap();
    std::fs::write(maestro_path::join(&[&root, "@scope/pkg"]), "contents").unwrap();
    workflow(NativePackageOperations::new(|_| false), &cwd, &agent, &root).unwrap();
    let effects = Rc::new(Effects::default());
    effects
        .outputs
        .borrow_mut()
        .push_back(Ok(support::output(&root, "", Some(0))));
    workflow(
        WorkflowOperations(Controlled(effects.clone())),
        &cwd,
        &agent,
        &root,
    )
    .unwrap();
    assert_eq!(
        *effects.calls.borrow(),
        vec![(
            "/bin/sh".into(),
            vec![
                "-c".into(),
                format!("printf '%s' '{root}'"),
                "--".into(),
                "root".into(),
                "-g".into()
            ]
        )]
    );
}
#[cfg(windows)]
#[test]
fn windows_scopes_keep_drive_and_namespace_identity() {
    use maestro_packages::InstalledSourceScope::{Project, User};
    for (cwd, agent, source, scope, expected) in [
        (
            r"C:\project",
            r"D:\agent",
            r".\pkg",
            User,
            r"C:\project\pkg",
        ),
        (r"C:\project", r"D:\agent", r".\pkg", Project, r"..\pkg"),
        (
            r"C:\project",
            r"C:\project\.maestro",
            r".\.maestro",
            User,
            ".",
        ),
        (
            r"\\srv\share\project",
            r"\\srv\other\agent",
            r".\pkg",
            User,
            r"\\srv\share\project\pkg",
        ),
        (
            r"\\?\C:\project",
            r"C:\agent",
            r".\pkg",
            User,
            r"\\?\C:\project\pkg",
        ),
    ] {
        let settings = Rc::new(RefCell::new(
            SettingsManager::in_memory(Settings::default()),
        ));
        let mut manager = DefaultPackageManager::new(
            PackageManagerOptions {
                cwd: cwd.into(),
                agent_dir: agent.into(),
                settings_manager: settings.clone(),
            },
            Controlled(Rc::new(Effects::default())),
        );
        assert!(manager.add_source_to_settings(source, Some(scope)).unwrap());
        let actual = match scope {
            User => settings.borrow().get_global_settings(),
            Project => settings.borrow().get_project_settings(),
        };
        assert_eq!(actual.0["packages"], json!([expected]));
        assert!(!manager.add_source_to_settings(source, Some(scope)).unwrap());
    }
}
#[test]
fn configuration_writes_publish_before_flush_without_rewriting_noops() {
    let storage = Arc::new(Storage::default());
    let settings = Rc::new(RefCell::new(SettingsManager::from_storage(storage.clone())));
    let mut manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: "/work/project".into(),
            agent_dir: "/agent".into(),
            settings_manager: settings.clone(),
        },
        Controlled(Rc::new(Effects::default())),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    assert!(
        manager
            .add_source_to_settings("npm: pkg@latest ", None)
            .unwrap()
    );
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!(["npm: pkg@latest "])
    );
    runtime.block_on(settings.borrow().flush());
    let first = storage.writes.load(Ordering::SeqCst);
    assert_eq!(first, 1);
    assert!(!manager.add_source_to_settings("npm:pkg@2", None).unwrap());
    assert!(
        !manager
            .remove_source_from_settings("npm:absent", None)
            .unwrap()
    );
    runtime.block_on(settings.borrow().flush());
    assert_eq!(storage.writes.load(Ordering::SeqCst), first);
    assert!(
        manager
            .remove_source_from_settings("npm:pkg", None)
            .unwrap()
    );
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!([])
    );
    runtime.block_on(settings.borrow().flush());
    assert_eq!(storage.writes.load(Ordering::SeqCst), first + 1);
    let mut text = None;
    storage
        .with_lock(SettingsScope::Global, &mut |current| {
            text = current.map(str::to_owned);
            Ok(None)
        })
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&text.unwrap()).unwrap()["packages"],
        json!([])
    );
}

/// One exclusively-created disposable native directory.
#[cfg(unix)]
struct Scratch(pub std::path::PathBuf);
#[cfg(unix)]
impl Scratch {
    /// Creates a fresh directory, never deleting someone else's path.
    fn new() -> std::io::Result<Self> {
        let name = format!(
            "maestro-packages-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(std::io::Error::other)?
                .as_nanos()
        );
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
#[cfg(unix)]
impl Drop for Scratch {
    fn drop(&mut self) {
        assert!(std::fs::remove_dir_all(&self.0).is_ok());
    }
}

use maestro_settings::{
    InMemorySettingsStorage, SettingsScope, SettingsStorage, SettingsStorageError, SettingsUpdate,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
/// Observes completed persistence without holding a lock across updates.
#[derive(Default)]
struct Storage {
    /// The existing raw-text storage.
    inner: InMemorySettingsStorage,
    /// Completed writes.
    writes: AtomicUsize,
}
impl SettingsStorage for Storage {
    fn with_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        self.inner.with_lock(scope, &mut |current| {
            let next = update(current)?;
            if next.is_some() {
                self.writes.fetch_add(1, Ordering::SeqCst);
            }
            Ok(next)
        })
    }
}

#[cfg(unix)]
use maestro_packages::PackageOperations;
/// A controlled adapter using the same isolated existence set.
#[cfg(unix)]
struct WorkflowOperations(Controlled);
#[cfg(unix)]
impl PackageOperations for WorkflowOperations {
    fn exists(&self, path: &str) -> bool {
        !path.ends_with("/local")
    }
    fn home_dir(&self) -> std::io::Result<String> {
        self.0.home_dir()
    }
    fn current_dir(&self) -> std::io::Result<String> {
        self.0.current_dir()
    }
    fn drive_directory(&self, drive: char) -> Option<String> {
        self.0.drive_directory(drive)
    }
    fn run_command_sync(
        &self,
        command: &str,
        args: &[String],
    ) -> std::io::Result<maestro_packages::CommandOutput> {
        self.0.run_command_sync(command, args)
    }
}

/// Every public walk rejects this malformed selected value without a write.
fn rejects_packages(root: &serde_json::Value) {
    let (mut manager, settings, _) = support::manager(&json!({"packages":root}));
    assert!(manager.add_source_to_settings("npm:pkg", None).is_err());
    assert!(
        manager
            .remove_source_from_settings("npm:pkg", None)
            .is_err()
    );
    assert!(manager.list_configured_packages().is_err());
    assert_eq!(&settings.borrow().get_global_settings().0["packages"], root);
}

/// Every field is consumed by the classification operation.
#[cfg(unix)]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceVector {
    /// Source input.
    input: String,
    /// Existing-content path.
    expected: String,
}
/// Every field is consumed by scoped normalization.
#[cfg(unix)]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NormalizationVector {
    /// Source input.
    input: String,
    /// Selected scope.
    scope: String,
    /// Stored source.
    expected: String,
}

/// A completed lookup changes live project settings after both list snapshots.
#[cfg(unix)]
fn snapshot_before_lookup() -> std::io::Result<()> {
    use maestro_packages::InstalledSourceScope::{Project, User};
    let (mut manager, settings, effects) = support::manager(&json!({"packages":["npm:first"]}));
    settings
        .borrow_mut()
        .set_project_packages(vec![maestro_settings::PackageSource::Source(
            "./local".into(),
        )]);
    *effects.on_command.borrow_mut() =
        Some(Box::new(move || {
            settings.borrow_mut().set_project_packages(vec![
                maestro_settings::PackageSource::Source("./changed".into()),
            ]);
        }));
    let rows = manager.list_configured_packages()?;
    assert_eq!(
        rows.iter()
            .map(|row| (row.source.as_str(), row.scope))
            .collect::<Vec<_>>(),
        vec![("npm:first", User), ("./local", Project)]
    );
    assert_eq!(
        *effects.paths.borrow(),
        vec!["/root/first", "/work/project/.maestro/local"]
    );
    Ok(())
}

/// A relative home operand needs ambient context only for scope-relative storage.
#[cfg(unix)]
fn relative_home_storage_needs_cwd() -> std::io::Result<()> {
    let (mut manager, settings, effects) = support::manager(&json!({}));
    *effects.home_override.borrow_mut() = Some("relative-home".into());
    effects.fail_ambient.set(true);
    let result = manager.add_source_to_settings("~", None);
    assert!(
        result.is_err(),
        "relative home storage must read ambient cwd"
    );
    if let Err(error) = result {
        assert_eq!(error.to_string(), "cwd unavailable");
    }
    assert_eq!(*effects.reads.borrow(), vec!["home", "cwd"]);
    assert!(
        !settings
            .borrow()
            .get_global_settings()
            .0
            .contains_key("packages")
    );
    effects.fail_ambient.set(false);
    assert!(manager.add_source_to_settings("~", None)?);
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!(["../../../ambient/relative-home"])
    );
    Ok(())
}
