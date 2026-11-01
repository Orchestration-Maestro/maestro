//! First settings writes retain independent edits under native exclusion.

use crate::support::{self, Scratch, block_on, file_storage};
use maestro_settings::{
    FileSettingsStorage, InMemorySettingsStorage, SettingsListEntry, SettingsManager,
    SettingsScope, SettingsStorage, SettingsStorageError, SettingsUpdate,
};
use serde_json::{Value, json};
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn exclusion_is_held(file: &Path) -> bool {
    let mut name = file.as_os_str().to_owned();
    name.push(".lock");
    let lock = match File::open(name) {
        Ok(lock) => lock,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return false,
        Err(error) => panic!("cannot probe sidecar: {error}"),
    };
    match lock.try_lock() {
        Err(std::fs::TryLockError::WouldBlock) => true,
        Ok(()) => false,
        Err(error) => panic!("unexpected sidecar error: {error}"),
    }
}

fn root_exclusion(scope: SettingsScope) {
    let tree = Scratch::new().unwrap();
    let mut calls = 0;
    file_storage(tree.path())
        .with_write_lock(scope, &mut |current| {
            calls += 1;
            assert_eq!(current, None);
            assert!(
                exclusion_is_held(&support::settings_file(tree.path(), scope)),
                "first-write callback ran before owning exclusion"
            );
            Ok(Some("{}".into()))
        })
        .unwrap();
    assert_eq!(
        calls, 1,
        "the root witness must execute exactly one callback"
    );
}
#[test]
fn global_first_callback_owns_exclusion() {
    root_exclusion(SettingsScope::Global);
}
#[test]
fn project_first_callback_owns_exclusion() {
    root_exclusion(SettingsScope::Project);
}
// A callback that writes over a missing file must own its real sidecar.
struct Observed {
    native: FileSettingsStorage,
    file: PathBuf,
    scope: SettingsScope,
}
impl SettingsStorage for Observed {
    fn with_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        self.native
            .with_lock(scope, &mut |current| self.update(scope, current, update))
    }
    fn with_write_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        self.native
            .with_write_lock(scope, &mut |current| self.update(scope, current, update))
    }
}
impl Observed {
    fn update(
        &self,
        scope: SettingsScope,
        current: Option<&str>,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> SettingsUpdate {
        let next = update(current)?;
        if scope == self.scope && next.is_some() && current.is_none() {
            assert!(
                exclusion_is_held(&self.file),
                "a first write ran without exclusion"
            );
        }
        Ok(next)
    }
}

fn observed(tree: &Scratch, scope: SettingsScope) -> Arc<Observed> {
    Arc::new(Observed {
        native: file_storage(tree.path()),
        file: support::settings_file(tree.path(), scope),
        scope,
    })
}
fn read(file: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(file).unwrap()).unwrap()
}
fn manager_edits(scope: SettingsScope) {
    let tree = Scratch::new().unwrap();
    let storage = observed(&tree, scope);
    let mut first = SettingsManager::from_storage(storage.clone());
    let mut second = SettingsManager::from_storage(storage);
    // Both managers loaded absence. The second writes once the first has
    // completed: a contended writer gives up after its bounded attempts,
    // however long the holder's durable write takes.
    match scope {
        SettingsScope::Global => first.set_theme("first".into()),
        SettingsScope::Project => {
            first.set_project_extension_paths(vec![SettingsListEntry::String("first".into())]);
        }
    }
    block_on(first.flush());
    match scope {
        SettingsScope::Global => second.set_default_model("second".into()),
        SettingsScope::Project => {
            second.set_project_skill_paths(vec![SettingsListEntry::String("second".into())]);
        }
    }
    block_on(second.flush());
    for manager in [&mut first, &mut second] {
        let errors = manager.drain_errors();
        assert!(errors.is_empty(), "{errors:?}");
    }
    let saved = read(&support::settings_file(tree.path(), scope));
    let expected = match scope {
        SettingsScope::Global => json!({"theme":"first","defaultModel":"second"}),
        SettingsScope::Project => json!({"extensions":["first"],"skills":["second"]}),
    };
    assert_eq!(saved, expected, "an accepted manager edit was lost");
}
#[test]
fn stale_first_global_manager_edits_merge() {
    manager_edits(SettingsScope::Global);
}
#[test]
fn stale_first_project_manager_edits_merge() {
    manager_edits(SettingsScope::Project);
}

struct ProcessObserver {
    storage: FileSettingsStorage,
    file: std::path::PathBuf,
}
impl SettingsStorage for ProcessObserver {
    fn with_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        self.storage.with_lock(scope, update)
    }
    fn with_write_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        let mut calls = 0;
        let result = self.storage.with_write_lock(scope, &mut |current| {
            calls += 1;
            assert!(
                exclusion_is_held(&self.file),
                "write callback must hold native exclusion"
            );
            assert!(calls == 1, "callback replayed");
            update(current)
        });
        assert_eq!(calls, 1, "{result:?}");
        result
    }
}
fn child_scope() -> SettingsScope {
    match std::env::var("MAESTRO_FIRST_WRITE_SCOPE").unwrap().as_str() {
        "global" => SettingsScope::Global,
        "project" => SettingsScope::Project,
        _ => panic!("unknown scope"),
    }
}
fn write_child(kind: &str) {
    let root = std::path::PathBuf::from(std::env::var_os("MAESTRO_FIRST_WRITE_ROOT").unwrap());
    let scope = child_scope();
    let key = std::env::var("MAESTRO_FIRST_WRITE_KEY").unwrap();
    let storage = Arc::new(ProcessObserver {
        storage: file_storage(&root),
        file: support::settings_file(&root, scope),
    });
    let manager = (kind != "raw").then(|| SettingsManager::from_storage(storage.clone()));
    // Managers have loaded absence before readiness; callbacks never wait for each other.
    println!("READY");
    std::io::stdout().flush().unwrap();
    std::io::stdin().read_exact(&mut [0]).unwrap();
    if kind == "raw" {
        storage
            .with_write_lock(scope, &mut |current| {
                let mut value: Value = serde_json::from_str(current.unwrap_or("{}")).unwrap();
                value[key.clone()] = Value::Bool(true);
                Ok(Some(value.to_string()))
            })
            .unwrap();
    } else {
        let mut manager = manager.unwrap();
        match (scope, key.as_str()) {
            (SettingsScope::Global, "first") => manager.set_theme("first".into()),
            (SettingsScope::Global, _) => manager.set_default_model("second".into()),
            (SettingsScope::Project, "first") => {
                manager.set_project_extension_paths(support::text_entries(&["first"]));
            }
            (SettingsScope::Project, _) => {
                manager.set_project_skill_paths(support::text_entries(&["second"]));
            }
        }
        support::block_on(manager.flush());
        let errors = manager.drain_errors();
        assert!(errors.is_empty(), "{errors:?}");
    }
    println!("DONE");
}
fn two_processes(test: &str, scope: SettingsScope, kind: &str) {
    if support::child_case().is_some() {
        write_child(kind);
        return;
    }
    if maestro_test_process::run_alone(test).unwrap() {
        return;
    }
    let root = support::Scratch::new().unwrap();
    let mut children = Vec::new();
    for key in ["first", "second"] {
        let mut child = support::child_command(test, "write")
            .env("MAESTRO_FIRST_WRITE_ROOT", root.path())
            .env("MAESTRO_FIRST_WRITE_KEY", key)
            .env(
                "MAESTRO_FIRST_WRITE_SCOPE",
                if scope == SettingsScope::Global {
                    "global"
                } else {
                    "project"
                },
            )
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        support::read_marker(&mut output, "READY");
        children.push((child, output));
    }
    assert!(
        !support::settings_file(root.path(), scope).exists(),
        "both writers start from absence"
    );
    // Each writer starts once the previous one has completed: a contended
    // writer gives up after its bounded attempts, however long the holder's
    // durable write takes.
    for (mut child, mut output) in children {
        child.stdin.as_mut().unwrap().write_all(b"x").unwrap();
        support::read_marker(&mut output, "DONE");
        assert!(child.wait().unwrap().success());
    }
    let saved: Value = serde_json::from_str(
        &fs::read_to_string(support::settings_file(root.path(), scope)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        saved,
        match (kind, scope) {
            ("raw", _) => json!({"first":true,"second":true}),
            (_, SettingsScope::Global) => json!({"theme":"first","defaultModel":"second"}),
            _ => json!({"extensions":["first"],"skills":["second"]}),
        }
    );
}
#[test]
fn first_global_raw_process_writes_merge() {
    two_processes(
        "settings_first_writes::first_global_raw_process_writes_merge",
        SettingsScope::Global,
        "raw",
    );
}
#[test]
fn first_project_raw_process_writes_merge() {
    two_processes(
        "settings_first_writes::first_project_raw_process_writes_merge",
        SettingsScope::Project,
        "raw",
    );
}
#[test]
fn first_global_manager_process_writes_merge() {
    two_processes(
        "settings_first_writes::first_global_manager_process_writes_merge",
        SettingsScope::Global,
        "manager",
    );
}
#[test]
fn first_project_manager_process_writes_merge() {
    two_processes(
        "settings_first_writes::first_project_manager_process_writes_merge",
        SettingsScope::Project,
        "manager",
    );
}
struct Supplied(InMemorySettingsStorage);
impl SettingsStorage for Supplied {
    fn with_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        self.0.with_lock(scope, update)
    }
}
#[test]
fn supplied_adapter_default_keeps_its_callback_contract() {
    let adapter = Supplied(InMemorySettingsStorage::new());
    for scope in [SettingsScope::Global, SettingsScope::Project] {
        let mut calls = 0;
        adapter
            .with_write_lock(scope, &mut |current| {
                calls += 1;
                assert_eq!(current, None);
                Ok(Some(String::new()))
            })
            .unwrap();
        assert_eq!(calls, 1);
        calls = 0;
        let error = adapter
            .with_write_lock(scope, &mut |current| {
                calls += 1;
                assert_eq!(current, Some(""));
                Err("callback failed".into())
            })
            .unwrap_err();
        assert_eq!(calls, 1);
        assert_eq!(error.to_string(), "callback failed");
        calls = 0;
        adapter
            .with_write_lock(scope, &mut |current| {
                calls += 1;
                assert_eq!(current, Some(""));
                Ok(None)
            })
            .unwrap();
        assert_eq!(calls, 1);
    }
}
#[test]
fn write_intent_no_write_and_error_release_exclusion() {
    for scope in [SettingsScope::Global, SettingsScope::Project] {
        let root = support::Scratch::new().unwrap();
        let storage = file_storage(root.path());
        let file = support::settings_file(root.path(), scope);
        let mut calls = 0;
        storage
            .with_write_lock(scope, &mut |current| {
                calls += 1;
                assert_eq!(current, None);
                assert!(exclusion_is_held(&file));
                Ok(None)
            })
            .unwrap();
        assert_eq!(calls, 1);
        assert!(!file.exists());
        assert!(!exclusion_is_held(&file));
        let error = storage
            .with_write_lock(scope, &mut |current| {
                assert_eq!(current, None);
                assert!(exclusion_is_held(&file));
                Err("callback failed".into())
            })
            .unwrap_err();
        assert_eq!(error.to_string(), "callback failed");
        assert!(!file.exists());
        assert!(!exclusion_is_held(&file));
        storage
            .with_write_lock(scope, &mut |current| {
                assert_eq!(current, None);
                Ok(Some("arbitrary\ntext".into()))
            })
            .unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "arbitrary\ntext");
        storage
            .with_write_lock(scope, &mut |current| {
                assert_eq!(current, Some("arbitrary\ntext"));
                Ok(Some(String::new()))
            })
            .unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"");
    }
    native_removal_preserves_siblings();
}

fn native_removal_preserves_siblings() {
    let root = support::Scratch::new().unwrap();
    let storage = Arc::new(file_storage(root.path()));
    storage
        .with_write_lock(SettingsScope::Global, &mut |_| {
            Ok(Some(r#"{"shellPath":"/bin/sh","opaque":true}"#.into()))
        })
        .unwrap();
    let mut manager = SettingsManager::from_storage(storage);
    manager.set_shell_path(None);
    support::block_on(manager.flush());
    assert!(manager.drain_errors().is_empty());
    let saved: Value = serde_json::from_str(
        &fs::read_to_string(support::settings_file(root.path(), SettingsScope::Global)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        saved,
        json!({"opaque":true}),
        "native persistence removes the edited key only"
    );
}

#[test]
fn legacy_missing_callbacks_stay_noncreating() {
    for scope in [SettingsScope::Global, SettingsScope::Project] {
        for fails in [false, true] {
            legacy_missing_callback(scope, fails);
        }
    }
}

fn legacy_missing_callback(scope: SettingsScope, fails: bool) {
    let root = support::Scratch::new().unwrap();
    let storage = file_storage(root.path());
    let target = support::settings_file(root.path(), scope);
    let mut calls = 0;
    let result = storage.with_lock(scope, &mut |current| {
        calls += 1;
        assert_eq!(current, None);
        assert!(!target.parent().unwrap().exists());
        if fails {
            Err("callback failed".into())
        } else {
            Ok(None)
        }
    });
    assert_eq!(calls, 1);
    match result {
        Ok(()) => assert!(!fails),
        Err(error) => {
            assert!(fails);
            assert_eq!(error.to_string(), "callback failed");
        }
    }
    assert!(!target.parent().unwrap().exists());
}

#[test]
fn write_intent_io_failures_skip_or_release_callback() {
    for scope in [SettingsScope::Global, SettingsScope::Project] {
        let root = support::Scratch::new().unwrap();
        let storage = file_storage(root.path());
        let target = support::settings_file(root.path(), scope);
        let mut sidecar = target.as_os_str().to_owned();
        sidecar.push(".lock");
        fs::create_dir_all(&sidecar).unwrap();
        let mut calls = 0;
        assert!(
            storage
                .with_write_lock(scope, &mut |_| {
                    calls += 1;
                    Ok(None)
                })
                .is_err()
        );
        assert_eq!(calls, 0);
        fs::remove_dir(&sidecar).unwrap();
        fs::create_dir(&target).unwrap();
        assert!(
            storage
                .with_write_lock(scope, &mut |_| {
                    calls += 1;
                    Ok(None)
                })
                .is_err()
        );
        assert_eq!(calls, 0);
        assert!(!exclusion_is_held(&target));
        fs::remove_dir(&target).unwrap();
        storage
            .with_write_lock(scope, &mut |_| Ok(Some("before".into())))
            .unwrap();
        assert!(
            storage
                .with_write_lock(scope, &mut |current| {
                    calls += 1;
                    assert_eq!(current, Some("before"));
                    fs::remove_file(&target).unwrap();
                    fs::create_dir(&target).unwrap();
                    Ok(Some("cannot write a directory".into()))
                })
                .is_err()
        );
        assert_eq!(calls, 1);
        assert!(!exclusion_is_held(&target));
        fs::remove_dir(&target).unwrap();
        storage
            .with_write_lock(scope, &mut |_| Ok(Some("recovered".into())))
            .unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "recovered");
    }
}
