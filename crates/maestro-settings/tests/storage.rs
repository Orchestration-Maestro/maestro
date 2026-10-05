#[allow(dead_code)]
mod support;
use maestro_settings::{SettingsError, SettingsScope, SettingsStorage};
use serde_json::{Map, Value, json};

fn object(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}
fn read(storage: &mut dyn SettingsStorage, scope: SettingsScope) -> Map<String, Value> {
    let mut detached = storage.read(scope).unwrap();
    let original = detached.clone();
    detached.clear();
    assert_eq!(storage.read(scope).unwrap(), original);
    original
}

#[test]
fn storage_transactions_commit_only_successful_replacements() {
    for factory in [support::memory, support::file] {
        let mut storage = factory(object(json!({"user":1})), object(json!({"project":2})));
        for (scope, other, initial) in [
            (
                SettingsScope::User,
                SettingsScope::Project,
                json!({"user":1}),
            ),
            (
                SettingsScope::Project,
                SettingsScope::User,
                json!({"project":2}),
            ),
        ] {
            let untouched = read(&mut *storage, other);
            let mut calls = 0;
            storage
                .transact(scope, &mut |current| {
                    calls += 1;
                    assert_eq!(*current, object(initial.clone()));
                    Ok(None)
                })
                .unwrap();
            assert_eq!(calls, 1);
            assert_eq!(read(&mut *storage, scope), object(initial));
            let mut calls = 0;
            storage
                .transact(scope, &mut |_| {
                    calls += 1;
                    Ok(Some(object(json!({"replaced":3}))))
                })
                .unwrap();
            assert_eq!(calls, 1);
            assert_eq!(read(&mut *storage, scope), object(json!({"replaced":3})));
            let mut calls = 0;
            assert_eq!(
                storage.transact(scope, &mut |_| {
                    calls += 1;
                    Err(SettingsError::Storage { scope })
                }),
                Err(SettingsError::Storage { scope })
            );
            assert_eq!(calls, 1);
            assert_eq!(read(&mut *storage, scope), object(json!({"replaced":3})));
            assert_eq!(read(&mut *storage, other), untouched);
        }
    }
}
