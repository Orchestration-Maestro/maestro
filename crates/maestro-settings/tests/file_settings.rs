#[allow(dead_code)]
mod support;
use maestro_settings::*;
use serde_json::{Map, json};
use std::sync::{Arc, atomic::AtomicBool};
use support::{Scratch, path};
fn adapter(scratch: &Scratch) -> FileSettingsStorage {
    FileSettingsStorage::new(scratch.locations(), Arc::new(AtomicBool::new(false)))
}
fn open(scratch: &Scratch) -> Result<Settings, SettingsError> {
    Settings::new(
        Map::new(),
        ManifestSettings {
            values: Map::new(),
            locks: vec![],
        },
        Box::new(adapter(scratch)),
    )
}
fn bytes(scratch: &Scratch, scope: SettingsScope) -> Vec<u8> {
    std::fs::read(
        scratch
            .locations()
            .configuration_directory(scope)
            .join("settings.json"),
    )
    .unwrap()
}
#[test]
fn file_updates_preserve_external_and_owned_unknown_values() {
    for scope in [SettingsScope::User, SettingsScope::Project] {
        let scratch = Scratch::new();
        let other = if scope == SettingsScope::User {
            SettingsScope::Project
        } else {
            SettingsScope::User
        };
        scratch.write(other, &json!({"untouched": true}));
        let untouched = bytes(&scratch, other);
        let mut settings = open(&scratch).unwrap();
        scratch.write(scope, &json!({"nested":{"leaf":"external","keep":2},"unknown.namespace":{"deep":{"keep":3}},"resources":["a"],"external":4}));
        let next = settings
            .set(
                SettingsTarget::Stored(scope),
                &path(&["nested", "leaf"]),
                json!("requested"),
            )
            .unwrap();
        assert_eq!(next.values["nested"], json!({"leaf":"requested","keep":2}));
        assert_eq!(next.values["unknown.namespace"], json!({"deep":{"keep":3}}));
        assert_eq!(next.values["resources"], json!(["a"]));
        assert_eq!(next.values["external"], json!(4));
        assert_eq!(bytes(&scratch, other), untouched);
        assert_eq!(open(&scratch).unwrap().resolve(), next);
    }
}
#[test]
fn independent_writers_reread_existing_and_missing_files() {
    for scope in [SettingsScope::User, SettingsScope::Project] {
        for existing in [true, false] {
            let scratch = Scratch::new();
            if existing {
                scratch.write(scope, &json!({"initial":0}));
            }
            let mut a = open(&scratch).unwrap();
            let mut b = open(&scratch).unwrap();
            a.set(SettingsTarget::Stored(scope), &path(&["a"]), json!(1))
                .unwrap();
            b.set(SettingsTarget::Stored(scope), &path(&["b"]), json!(2))
                .unwrap();
            a.set(SettingsTarget::Stored(scope), &path(&["c"]), json!(3))
                .unwrap();
            let reopened = open(&scratch).unwrap().resolve();
            for (key, value) in [("a", 1), ("b", 2), ("c", 3)] {
                assert_eq!(reopened.values[key], json!(value));
            }
        }
    }
}
#[test]
fn malformed_initial_files_are_never_overwritten() {
    for scope in [SettingsScope::User, SettingsScope::Project] {
        for invalid in [b"{\"secret\": SENTINEL}".as_slice(), b"", b"[]", b"null"] {
            let scratch = Scratch::new();
            scratch.write(scope, &json!({}));
            let file = scratch
                .locations()
                .configuration_directory(scope)
                .join("settings.json");
            std::fs::write(&file, invalid).unwrap();
            let error = open(&scratch).err().unwrap();
            assert!(
                matches!(&error, SettingsError::File { scope: s, path, kind: SettingsFileError::Malformed { .. } | SettingsFileError::NotObject } if *s == scope && *path == file)
            );
            let mut called = false;
            assert!(
                adapter(&scratch)
                    .transact(scope, &mut |_| {
                        called = true;
                        Ok(Some(Map::new()))
                    })
                    .is_err()
            );
            assert!(!called);
            assert_eq!(bytes(&scratch, scope), invalid);
            assert!(!format!("{error:?} {error}").contains("SENTINEL"));
        }
    }
}
#[test]
fn malformed_external_replacements_preserve_snapshot_and_bytes() {
    for scope in [SettingsScope::User, SettingsScope::Project] {
        for invalid in [b"{".as_slice(), b"[]"] {
            let scratch = Scratch::new();
            scratch.write(scope, &json!({"keep":1}));
            let mut settings = open(&scratch).unwrap();
            let before = settings.resolve();
            let file = scratch
                .locations()
                .configuration_directory(scope)
                .join("settings.json");
            std::fs::write(&file, invalid).unwrap();
            assert!(settings.reload().is_err());
            assert_eq!(settings.resolve(), before);
            assert!(
                settings
                    .set(SettingsTarget::Stored(scope), &path(&["added"]), json!(2))
                    .is_err()
            );
            assert_eq!(settings.resolve(), before);
            assert_eq!(bytes(&scratch, scope), invalid);
            scratch.write(scope, &json!({"repaired":3}));
            assert_eq!(settings.reload().unwrap().values["repaired"], json!(3));
            assert_eq!(
                settings
                    .set(SettingsTarget::Stored(scope), &path(&["added"]), json!(2))
                    .unwrap()
                    .values["added"],
                json!(2)
            );
        }
    }
}
#[test]
fn missing_reads_do_not_create_configuration() {
    let scratch = Scratch::new();
    let mut settings = open(&scratch).unwrap();
    settings.reload().unwrap();
    for scope in [SettingsScope::User, SettingsScope::Project] {
        assert_eq!(adapter(&scratch).read(scope).unwrap(), Map::new());
        assert!(!scratch.locations().configuration_directory(scope).exists());
    }
    settings
        .set(
            SettingsTarget::Stored(SettingsScope::Project),
            &path(&["key"]),
            json!(1),
        )
        .unwrap();
    assert!(
        !scratch
            .locations()
            .configuration_directory(SettingsScope::User)
            .exists()
    );
    let directory = scratch
        .locations()
        .configuration_directory(SettingsScope::Project);
    let mut names: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["settings.json", "settings.json.lock"]);
}
#[test]
fn file_errors_report_metadata_without_settings_values() {
    for scope in [SettingsScope::User, SettingsScope::Project] {
        let scratch = Scratch::new();
        scratch.write(scope, &json!({"usable":1}));
        let mut settings = open(&scratch).unwrap();
        let before = settings.resolve();
        let file = scratch
            .locations()
            .configuration_directory(scope)
            .join("settings.json");
        for invalid in [
            b"{\"secret\":\"SENTINEL\",\"bad\":\xff}".as_slice(),
            b"{\"secret\":\"SENTINEL\",",
            b"[\"SENTINEL\"]",
        ] {
            std::fs::write(&file, invalid).unwrap();
            let error = settings.reload().unwrap_err();
            assert!(
                matches!(&error, SettingsError::File { scope: s, path, kind: SettingsFileError::Malformed { line: 1, column: 1.. } | SettingsFileError::NotObject } if *s == scope && *path == file)
            );
            for text in [error.to_string(), format!("{error:?}")] {
                assert!(!text.contains("SENTINEL"));
            }
            assert_eq!(settings.resolve(), before);
        }
        std::fs::remove_file(&file).unwrap();
        std::fs::create_dir(&file).unwrap();
        assert!(matches!(
            settings.reload(),
            Err(SettingsError::File {
                kind: SettingsFileError::Io(_),
                ..
            })
        ));
        assert_eq!(settings.resolve(), before);
    }
}
