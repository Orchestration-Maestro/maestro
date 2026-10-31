//! Configured sources through the supplied-operation manager.
#[cfg(unix)]
use crate::resolution_support::{case_in_child, case_in_removed_cwd};
#[cfg(unix)]
use crate::scratch;
use crate::support;
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
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: "relative".into(),
            agent_dir: "relative-agent".into(),
            settings_manager: settings.clone(),
        },
        Controlled(effects.clone()),
    );
    assert!(effects.calls.borrow().is_empty());
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
    let Some(root) = case_in_child("home", fake_home) else {
        return;
    };
    let rows: Vec<SourceVector> =
        serde_json::from_str(include_str!("source_vectors.json")).unwrap();
    let (manager, _, _) = support::manager_below(&root, &json!({}), &json!([]), None);
    let mut visited = std::collections::HashSet::new();
    for row in rows {
        assert!(visited.insert(row.input.clone()), "duplicate query");
        let expected = support::below(&root, &row.expected);
        std::fs::create_dir_all(&expected).unwrap();
        assert_eq!(
            manager
                .get_installed_path(
                    &support::below(&root, &row.input),
                    maestro_packages::InstalledSourceScope::Project
                )
                .unwrap(),
            Some(expected),
            "{}",
            row.input
        );
    }
    assert_eq!(visited.len(), 26);
}
#[test]
#[cfg(unix)]
fn package_home_matches_the_resources_rule() {
    let empty_home = |_: &str| vec![("HOME", Some(String::new()))];
    let Some(root) = case_in_child("empty home", empty_home) else {
        return;
    };
    assert_eq!(maestro_resources::paths::home_directory().unwrap(), "");
    let (manager, _, _) = support::manager_below(&root, &json!({}), &json!([]), None);
    assert_eq!(
        manager
            .get_installed_path("~", maestro_packages::InstalledSourceScope::Project)
            .unwrap(),
        None
    );
}
#[test]
#[cfg(unix)]
fn scoped_local_storage_and_matching_use_different_bases() {
    use maestro_packages::InstalledSourceScope::{Project, User};
    let Some(root) = case_in_child("home", fake_home) else {
        return;
    };
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
        let (manager, settings, _) = support::manager_below(&root, &json!({}), &json!([]), None);
        let input = support::below(&root, &row.input);
        assert!(manager.add_source_to_settings(&input, Some(scope)).unwrap());
        let stored = match scope {
            User => settings.borrow().get_global_settings(),
            Project => settings.borrow().get_project_settings(),
        };
        assert_eq!(stored.0["packages"], json!([row.expected]), "{}", row.input);
        assert!(!manager.add_source_to_settings(&input, Some(scope)).unwrap());
    }
    assert_eq!(visited.len(), 26);
    let (manager, settings, _) = support::manager_below(&root, &json!({}), &json!([]), None);
    let project = support::below(&root, "/work/project/.maestro");
    assert!(
        manager
            .add_source_to_settings(&project, Some(Project))
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
    let (manager, settings, _) = support::manager(&json!({"packages":[original]}));
    assert!(
        manager
            .add_source_to_settings("npm: pkg@latest ", None)
            .unwrap()
    );
    assert!(manager.add_source_to_settings("npm:pkg@2", None).unwrap());
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
        json!([original, "npm:pkg@2"])
    );
    assert_eq!(
        settings.borrow().get_project_settings().0["packages"],
        json!(["npm:pkg"])
    );
}
#[test]
fn git_transports_match_one_identity_while_replacing_changed_refs() {
    let (manager, settings, _) = support::manager(
        &json!({"packages":["git:git@github.com:u/r@old", {"source":"https://github.com/u/r.git","skills":[]}, "https://github.com/v/r"]}),
    );
    assert!(
        manager
            .add_source_to_settings("ssh://git@github.com/u/r@new", None)
            .unwrap()
    );
    assert!(
        manager
            .add_source_to_settings("git:github.com/u/r", None)
            .unwrap()
    );
    assert!(
        manager
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
    let (manager, settings, _) = support::manager(
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
    let (manager, settings, _) = support::manager(&json!({"packages":retained}));
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
    let (manager, _, _, tree) = support::uninstalled(
        &json!({"packages":["./local", {"source":"https://github.com/u/r","skills":null}, "./local"]}),
        &json!(["./local", "https://github.com/u/r"]),
    );
    let expected = [
        "/home/reader/agent/local",
        "/home/reader/agent/git/github.com/u/r",
        "/home/reader/agent/local",
        "/work/project/.maestro/local",
        "/work/project/.maestro/git/github.com/u/r",
    ]
    .map(|path| support::below(tree.path(), path));
    for path in &expected {
        std::fs::create_dir_all(path).unwrap();
    }
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
        rows.into_iter()
            .map(|r| r.installed_path.unwrap())
            .collect::<Vec<_>>(),
        expected
    );
    snapshot_before_lookup().unwrap();
}
#[test]
fn raw_package_errors_follow_the_consumed_entry() {
    for root in [json!(null), json!([])] {
        let (manager, _, _) = support::manager(&json!({"packages":root}));
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
    let (manager, settings, _) = support::manager(&json!({"packages":[retained]}));
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
    let (manager, _, _) = support::manager(&json!({"packages":["npm:pkg",{}]}));
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
    let (manager, settings, effects, tree) =
        support::uninstalled(&json!({"packages":["npm:first",{}]}), &json!([]));
    assert!(manager.list_configured_packages().is_err());
    assert_eq!(
        *effects.calls.borrow(),
        vec![("npm".into(), vec!["root".into(), "-g".into()])]
    );
    settings
        .borrow_mut()
        .set_packages(vec![PackageSource::Source("npm:second".into())]);
    let second = support::below(tree.path(), "/root/second");
    std::fs::create_dir_all(&second).unwrap();
    let rows = manager.list_configured_packages().unwrap();
    assert_eq!(rows[0].source, "npm:second");
    assert_eq!(rows[0].installed_path, Some(second));
    assert_eq!(effects.calls.borrow().len(), 1);
}
#[test]
#[cfg(unix)]
fn local_resolution_reads_home_and_cwd_only_when_needed() {
    use maestro_packages::InstalledSourceScope::User;
    relative_home_storage_needs_cwd();
    let Some((root, _removed)) = case_in_removed_cwd("removed working directory", |_| vec![])
    else {
        return;
    };
    let (manager, settings, _) = support::manager_below(&root, &json!({}), &json!([]), None);
    let absolute = support::below(&root, "/absolute/pkg");
    let local = support::below(&root, "/home/reader/agent/pkg");
    for path in [&absolute, &local] {
        std::fs::create_dir_all(path).unwrap();
    }
    assert_eq!(
        manager.get_installed_path(&absolute, User).unwrap(),
        Some(absolute)
    );
    assert_eq!(
        manager.get_installed_path("./pkg", User).unwrap(),
        Some(local)
    );
    assert!(!manager.remove_source_from_settings("~", None).unwrap());
    let relative = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: "relative".into(),
            agent_dir: "relative-agent".into(),
            settings_manager: settings,
        },
        Controlled(Rc::new(Effects::default())),
    );
    assert_eq!(
        relative
            .get_installed_path("./pkg", User)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::NotFound
    );
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
        let (manager, _, _, tree) = support::uninstalled(&json!({}), &json!([]));
        assert_eq!(manager.get_installed_path(source, scope).unwrap(), None);
        let expected = support::below(tree.path(), expected);
        std::fs::create_dir_all(&expected).unwrap();
        assert_eq!(
            manager.get_installed_path(source, scope).unwrap(),
            Some(expected)
        );
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
    let manager = DefaultPackageManager::new(
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
    assert!(manager.add_source_to_settings("ssh://git@github.com/u/r", Some(Project))?);
    let rows = manager.list_configured_packages()?;
    assert_eq!(
        rows.iter()
            .map(|r| (r.source.as_str(), r.scope))
            .collect::<Vec<_>>(),
        vec![
            ("../project/local", User),
            ("npm:@scope/pkg", User),
            ("ssh://git@github.com/u/r", Project)
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
    let scratch = scratch::Scratch::new().unwrap();
    let base = scratch.path().to_str().unwrap();
    let cwd = maestro_path::join(&[base, "project"]);
    let agent = maestro_path::join(&[base, "agent"]);
    let root = maestro_path::join(&[base, "global"]);
    std::fs::create_dir_all(maestro_path::join(&[&cwd, ".maestro/git/github.com/u/r"])).unwrap();
    std::fs::create_dir_all(maestro_path::join(&[&root, "@scope"])).unwrap();
    std::fs::write(maestro_path::join(&[&root, "@scope/pkg"]), "contents").unwrap();
    workflow(
        NativePackageOperations::new(
            Rc::new(|| false),
            &std::rc::Rc::new(tokio::task::LocalSet::new()),
        ),
        &cwd,
        &agent,
        &root,
    )
    .unwrap();
    let effects = Rc::new(Effects::default());
    effects
        .outputs
        .borrow_mut()
        .push_back(Ok(support::output(&root, "", Some(0))));
    workflow(Controlled(effects.clone()), &cwd, &agent, &root).unwrap();
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
        let manager = DefaultPackageManager::new(
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
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: "/work/project".into(),
            agent_dir: "/agent".into(),
            settings_manager: settings.clone(),
        },
        Controlled(Rc::new(Effects::default())),
    );
    let runtime = crate::runtime::runtime();
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
    assert!(
        manager
            .add_source_to_settings("npm:pkg@latest", None)
            .unwrap()
    );
    assert!(
        !manager
            .remove_source_from_settings("npm:absent", None)
            .unwrap()
    );
    runtime.block_on(settings.borrow().flush());
    assert_eq!(storage.writes.load(Ordering::SeqCst), first + 1);
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
    assert_eq!(storage.writes.load(Ordering::SeqCst), first + 2);
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

use maestro_settings::{
    InMemorySettingsStorage, SettingsScope, SettingsStorage, SettingsStorageError, SettingsUpdate,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
/// Observes completed persistence without holding a lock across updates.
#[derive(Default)]
pub(crate) struct Storage {
    /// The existing raw-text storage.
    pub(crate) inner: InMemorySettingsStorage,
    /// Completed writes.
    writes: AtomicUsize,
    /// Completed setter scopes, in persistence order.
    pub(crate) scopes: std::sync::Mutex<Vec<String>>,
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
                self.scopes.lock().unwrap().push(match scope {
                    SettingsScope::Global => "user".into(),
                    SettingsScope::Project => "project".into(),
                });
            }
            Ok(next)
        })
    }
}

/// Every public walk rejects this malformed selected value without a write.
fn rejects_packages(root: &serde_json::Value) {
    let (manager, settings, _) = support::manager(&json!({"packages":root}));
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
    let (manager, settings, effects, tree) =
        support::uninstalled(&json!({"packages":["npm:first"]}), &json!(["./local"]));
    let expected = ["/root/first", "/work/project/.maestro/local"]
        .map(|path| support::below(tree.path(), path));
    for path in &expected {
        std::fs::create_dir_all(path).unwrap();
    }
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
        rows.into_iter()
            .map(|row| row.installed_path.unwrap())
            .collect::<Vec<_>>(),
        expected
    );
    Ok(())
}

/// A relative home operand needs ambient context only for scope-relative storage.
#[cfg(unix)]
fn relative_home_storage_needs_cwd() {
    let home = |_: &str| vec![("HOME", Some("relative-home".into()))];
    let Some((root, _removed)) = case_in_removed_cwd("relative home", home) else {
        return;
    };
    let (manager, settings, _) = support::manager_below(&root, &json!({}), &json!([]), None);
    let error = manager.add_source_to_settings("~", None).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(
        !settings
            .borrow()
            .get_global_settings()
            .0
            .contains_key("packages")
    );
    let ambient = support::below(&root, "/ambient");
    std::fs::create_dir(&ambient).unwrap();
    std::env::set_current_dir(ambient).unwrap();
    assert!(manager.add_source_to_settings("~", None).unwrap());
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!(["../../../ambient/relative-home"])
    );
}
/// The child environment whose home is the fixture's `/home/fake`.
#[cfg(unix)]
fn fake_home(root: &str) -> Vec<(&'static str, Option<String>)> {
    vec![("HOME", Some(format!("{root}/home/fake")))]
}

#[test]
fn short_hosts_and_public_domains_remain_separate_settings_entries() {
    let (manager, settings, _) = support::manager(&json!({}));
    let local = "https://gitlab/group/repo";
    let public = "https://gitlab.com/group/repo";
    assert!(manager.add_source_to_settings(local, None).unwrap());
    assert!(manager.add_source_to_settings(public, None).unwrap());
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!([local, public])
    );
    assert!(manager.remove_source_from_settings(public, None).unwrap());
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!([local])
    );
    assert!(!manager.remove_source_from_settings(public, None).unwrap());
    assert!(manager.remove_source_from_settings(local, None).unwrap());
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!([])
    );
}

#[test]
fn numeric_scp_namespace_matches_git_transport_in_settings() {
    let scp = "git:git@git.example:123/repo.git";
    let https = "https://git.example/123/repo";
    let (manager, settings, _) = support::manager(&json!({}));
    assert!(manager.add_source_to_settings(scp, None).unwrap());
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!([scp])
    );
    assert!(manager.add_source_to_settings(https, None).unwrap());
    assert!(manager.remove_source_from_settings(https, None).unwrap());
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!([])
    );
}

#[test]
fn queried_hosted_shortcut_matches_https_for_add_and_remove() {
    let (manager, settings, _) = support::manager(&json!({}));
    let shortcut = "git:github:octocat/Hello-World?tab=readme";
    let https = "https://github.com/octocat/Hello-World";
    assert!(manager.add_source_to_settings(shortcut, None).unwrap());
    assert!(manager.add_source_to_settings(https, None).unwrap());
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!([https])
    );
    assert!(manager.remove_source_from_settings(https, None).unwrap());
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!([])
    );
}

/// Matching entries replace their spelling while retaining object fields and exact no-ops.
#[test]
fn source_spelling_replacement_preserves_filters_and_exact_noops() {
    for entry in [
        json!("git:example.test/team/repo@old"),
        json!({"source":"git:example.test/team/repo", "skills":[], "unknown":7}),
    ] {
        let (manager, settings, _) = support::manager(&json!({"packages":[entry]}));
        let source = "https://example.test/team/repo";
        assert!(manager.add_source_to_settings(source, None).unwrap());
        let expected = if entry.is_object() {
            json!({"source":source, "skills":[], "unknown":7})
        } else {
            json!(source)
        };
        assert_eq!(
            settings.borrow().get_global_settings().0["packages"],
            json!([expected])
        );
        assert!(!manager.add_source_to_settings(source, None).unwrap());
    }
}
