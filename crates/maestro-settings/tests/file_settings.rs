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
        assert_eq!(next.values["nested"], json!({"leaf":"requested"}));
        for key in ["unknown.namespace", "resources", "external"] {
            assert!(!next.values.contains_key(key));
        }
        let reopened = open(&scratch).unwrap().resolve();
        assert_eq!(
            reopened.values["nested"],
            json!({"leaf":"requested","keep":2})
        );
        assert_eq!(
            reopened.values["unknown.namespace"],
            json!({"deep":{"keep":3}})
        );
        assert_eq!(reopened.values["resources"], json!(["a"]));
        assert_eq!(reopened.values["external"], json!(4));
        assert_eq!(bytes(&scratch, other), untouched);
        assert_eq!(reopened, settings.reload().unwrap());
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
            let mut settings = open(&scratch).unwrap();
            let errors = settings.drain_errors();
            assert_eq!(errors.len(), 1);
            let error = errors.into_iter().next().unwrap();
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
            let accepted = settings
                .set(SettingsTarget::Stored(scope), &path(&["added"]), json!(2))
                .unwrap();
            let mut expected = before.clone();
            expected.values.insert("added".into(), json!(2));
            expected.origins.insert(
                path(&["added"]),
                if scope == SettingsScope::User {
                    SettingsOrigin::User
                } else {
                    SettingsOrigin::Project
                },
            );
            assert_eq!(accepted, expected);
            assert_eq!(settings.resolve(), expected);
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

#[test]
fn malformed_scope_at_startup_keeps_healthy_scope_and_reports() {
    for invalid in [b"{\"secret\": SENTINEL}".as_slice(), b"", b"[\"SENTINEL\"]"] {
        for failed in [
            vec![SettingsScope::User],
            vec![SettingsScope::Project],
            vec![SettingsScope::User, SettingsScope::Project],
        ] {
            let scratch = Scratch::new();
            for scope in [SettingsScope::User, SettingsScope::Project] {
                scratch.write(scope, &json!({"healthy":true}));
                if failed.contains(&scope) {
                    std::fs::write(
                        scratch
                            .locations()
                            .configuration_directory(scope)
                            .join("settings.json"),
                        invalid,
                    )
                    .unwrap();
                }
            }
            let originals = [
                bytes(&scratch, SettingsScope::User),
                bytes(&scratch, SettingsScope::Project),
            ];
            let mut s = open(&scratch).unwrap();
            assert_eq!(
                s.resolve().values.get("healthy"),
                if failed.len() == 1 {
                    Some(&json!(true))
                } else {
                    None
                }
            );
            let errors = s.drain_errors();
            assert_eq!(errors.len(), failed.len());
            for (error, scope) in errors.iter().zip(&failed) {
                assert!(
                    matches!(error, SettingsError::File {scope:s, path:p, kind:SettingsFileError::Malformed {line:1,column:0..} | SettingsFileError::NotObject} if s == scope && *p == scratch.locations().configuration_directory(*scope).join("settings.json"))
                );
                assert!(!format!("{error} {error:?}").contains("SENTINEL"));
                assert_eq!(s.read_scope(*scope), Map::new());
            }
            for scope in [SettingsScope::User, SettingsScope::Project] {
                if !failed.contains(&scope) {
                    assert_eq!(s.read_scope(scope), support::map(json!({"healthy":true})));
                }
            }
            assert!(s.drain_errors().is_empty());
            assert_eq!(
                [
                    bytes(&scratch, SettingsScope::User),
                    bytes(&scratch, SettingsScope::Project)
                ],
                originals
            );
        }
    }
}

#[test]
fn latched_scope_applies_in_memory_and_writes_after_reload() {
    for scope in [SettingsScope::User, SettingsScope::Project] {
        let other = if scope == SettingsScope::User {
            SettingsScope::Project
        } else {
            SettingsScope::User
        };
        let scratch = Scratch::new();
        scratch.write(scope, &json!({}));
        scratch.write(other, &json!({"healthy":1}));
        let file = scratch
            .locations()
            .configuration_directory(scope)
            .join("settings.json");
        std::fs::write(&file, b"{").unwrap();
        let mut s = Settings::new(
            Map::new(),
            ManifestSettings {
                values: support::map(json!({"frozen":1})),
                locks: vec![path(&["frozen"])],
            },
            Box::new(adapter(&scratch)),
        )
        .unwrap();
        assert_eq!(s.drain_errors().len(), 1);
        assert_eq!(
            s.set(
                SettingsTarget::Stored(scope),
                &path(&["sessionOnly"]),
                json!(2)
            )
            .unwrap()
            .values["sessionOnly"],
            json!(2)
        );
        assert_eq!(s.read_scope(scope), support::map(json!({"sessionOnly":2})));
        assert_eq!(bytes(&scratch, scope), b"{");
        assert!(!file.with_file_name("settings.json.lock").exists());
        let before = s.resolve();
        let cached = s.read_scope(scope);
        for (segments, value) in [
            (path(&["frozen"]), json!(9)),
            (vec![], json!(false)),
            (path(&["sessionOnly", "child"]), json!(1)),
        ] {
            assert!(
                s.set(SettingsTarget::Stored(scope), &segments, value)
                    .is_err()
            );
            assert_eq!(s.resolve(), before);
            assert_eq!(s.read_scope(scope), cached);
            assert_eq!(bytes(&scratch, scope), b"{");
        }
        s.set(
            SettingsTarget::Stored(other),
            &path(&["persisted"]),
            json!(3),
        )
        .unwrap();
        assert_eq!(
            adapter(&scratch).read(other).unwrap()["persisted"],
            json!(3)
        );
        scratch.write(scope, &json!({"repaired":4}));
        let repaired = bytes(&scratch, scope);
        s.set(
            SettingsTarget::Stored(scope),
            &path(&["notWritten"]),
            json!(5),
        )
        .unwrap();
        assert_eq!(bytes(&scratch, scope), repaired);
        assert!(s.drain_errors().is_empty());
        s.reload().unwrap();
        assert_eq!(s.read_scope(scope), support::map(json!({"repaired":4})));
        s.set(SettingsTarget::Stored(scope), &path(&["written"]), json!(6))
            .unwrap();
        assert_eq!(
            adapter(&scratch).read(scope).unwrap(),
            support::map(json!({"repaired":4,"written":6}))
        );
    }
}

#[test]
fn reload_failures_collect_all_scopes_without_publication() {
    for healthy in [SettingsScope::User, SettingsScope::Project] {
        let failed = if healthy == SettingsScope::User {
            SettingsScope::Project
        } else {
            SettingsScope::User
        };
        let scratch = Scratch::new();
        scratch.write(SettingsScope::User, &json!({"user":1}));
        scratch.write(SettingsScope::Project, &json!({"project":2}));
        let mut s = open(&scratch).unwrap();
        s.apply_overrides("runtime".into(), support::map(json!({"override":3})))
            .unwrap();
        let before = s.resolve();
        let user = s.read_scope(SettingsScope::User);
        let project = s.read_scope(SettingsScope::Project);
        for scope in [SettingsScope::User, SettingsScope::Project] {
            std::fs::write(
                scratch
                    .locations()
                    .configuration_directory(scope)
                    .join("settings.json"),
                b"{",
            )
            .unwrap();
        }
        let first = s.reload().unwrap_err();
        assert!(matches!(
            &first,
            SettingsError::File {
                scope: SettingsScope::User,
                ..
            }
        ));
        let errors = s.drain_errors();
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0], first);
        assert!(matches!(
            &errors[1],
            SettingsError::File {
                scope: SettingsScope::Project,
                ..
            }
        ));
        assert!(s.drain_errors().is_empty());
        assert_eq!(s.resolve(), before);
        assert_eq!(s.read_scope(SettingsScope::User), user);
        assert_eq!(s.read_scope(SettingsScope::Project), project);
        scratch.write(healthy, &json!({"external":4}));
        assert!(matches!(s.reload(), Err(SettingsError::File {scope,..}) if scope == failed));
        assert_eq!(s.drain_errors().len(), 1);
        assert_eq!(s.resolve(), before);
        assert_eq!(s.read_scope(SettingsScope::User), user);
        assert_eq!(s.read_scope(SettingsScope::Project), project);
        s.set(
            SettingsTarget::Stored(healthy),
            &path(&["persisted"]),
            json!(5),
        )
        .unwrap();
        assert_eq!(
            adapter(&scratch).read(healthy).unwrap(),
            support::map(json!({"external":4,"persisted":5}))
        );
        assert!(!s.read_scope(healthy).contains_key("external"));
        s.set(
            SettingsTarget::Stored(failed),
            &path(&["sessionOnly"]),
            json!(6),
        )
        .unwrap();
        assert_eq!(bytes(&scratch, failed), b"{");
        assert_eq!(s.read_scope(failed)["sessionOnly"], json!(6));
        assert!(s.drain_errors().is_empty());
    }
    for scope in [SettingsScope::User, SettingsScope::Project] {
        let scratch = Scratch::new();
        scratch.write(scope, &json!({"frozen":1,"keep":2}));
        let mut s = Settings::new(
            Map::new(),
            ManifestSettings {
                values: support::map(json!({"frozen":1})),
                locks: vec![path(&["frozen"])],
            },
            Box::new(adapter(&scratch)),
        )
        .unwrap();
        s.apply_overrides("runtime".into(), support::map(json!({"override":true})))
            .unwrap();
        let before = s.resolve();
        let user = s.read_scope(SettingsScope::User);
        let project = s.read_scope(SettingsScope::Project);
        scratch.write(scope, &json!({"frozen":9}));
        assert!(matches!(
            s.reload(),
            Err(SettingsError::LockConflict { .. })
        ));
        assert_eq!(s.resolve(), before);
        assert_eq!(s.read_scope(SettingsScope::User), user);
        assert_eq!(s.read_scope(SettingsScope::Project), project);
        assert!(s.drain_errors().is_empty());
        let conflict = bytes(&scratch, scope);
        assert!(matches!(
            s.set(SettingsTarget::Stored(scope), &path(&["added"]), json!(3)),
            Err(SettingsError::LockConflict { .. })
        ));
        assert_eq!(s.resolve(), before);
        assert_eq!(bytes(&scratch, scope), conflict);
        scratch.write(scope, &json!({"frozen":1,"external":4}));
        s.set(SettingsTarget::Stored(scope), &path(&["added"]), json!(3))
            .unwrap();
        assert_eq!(adapter(&scratch).read(scope).unwrap()["added"], json!(3));
        assert!(s.drain_errors().is_empty());
    }
}

#[test]
fn malformed_other_scope_does_not_block_setter() {
    for scope in [SettingsScope::User, SettingsScope::Project] {
        let other = if scope == SettingsScope::User {
            SettingsScope::Project
        } else {
            SettingsScope::User
        };
        let scratch = Scratch::new();
        scratch.write(scope, &json!({"keep":1}));
        scratch.write(other, &json!({"cachedOther":2}));
        let mut s = open(&scratch).unwrap();
        std::fs::write(
            scratch
                .locations()
                .configuration_directory(other)
                .join("settings.json"),
            b"{",
        )
        .unwrap();
        let next = s
            .set(SettingsTarget::Stored(scope), &path(&["added"]), json!(3))
            .unwrap();
        assert_eq!(
            adapter(&scratch).read(scope).unwrap(),
            support::map(json!({"keep":1,"added":3}))
        );
        assert_eq!(bytes(&scratch, other), b"{");
        assert_eq!(next.values["cachedOther"], json!(2));
        assert_eq!(s.read_scope(other), support::map(json!({"cachedOther":2})));
        assert!(s.drain_errors().is_empty());
    }
}

#[test]
fn save_publishes_requested_change_until_reload() {
    for scope in [SettingsScope::User, SettingsScope::Project] {
        let other = if scope == SettingsScope::User {
            SettingsScope::Project
        } else {
            SettingsScope::User
        };
        for (segments, requested, cached_target, disk_target) in [
            (
                path(&["nested", "leaf"]),
                json!(3),
                json!({"nested":{"leaf":3,"cachedSibling":1},"unrelated":1}),
                json!({"nested":{"leaf":3,"freshSibling":4},"unrelated":9}),
            ),
            (
                path(&["nested"]),
                json!({"only":3}),
                json!({"nested":{"only":3},"unrelated":1}),
                json!({"nested":{"only":3},"unrelated":9}),
            ),
            (
                vec![],
                json!({"only":3}),
                json!({"only":3}),
                json!({"only":3}),
            ),
        ] {
            let scratch = Scratch::new();
            scratch.write(
                scope,
                &json!({"nested":{"leaf":0,"cachedSibling":1},"unrelated":1}),
            );
            scratch.write(other, &json!({"other":2}));
            let mut s = open(&scratch).unwrap();
            s.apply_overrides("runtime".into(), support::map(json!({"override":true})))
                .unwrap();
            scratch.write(
                scope,
                &json!({"nested":{"leaf":99,"freshSibling":4},"unrelated":9}),
            );
            scratch.write(other, &json!({"other":8,"newOther":true}));
            let untouched = bytes(&scratch, other);
            let next = s
                .set(SettingsTarget::Stored(scope), &segments, requested)
                .unwrap();
            assert_eq!(s.read_scope(scope), support::map(cached_target.clone()));
            assert_eq!(s.read_scope(other), support::map(json!({"other":2})));
            for (key, value) in support::map(cached_target) {
                assert_eq!(next.values[&key], value);
            }
            assert_eq!(next.values["other"], json!(2));
            assert!(!next.values.contains_key("newOther"));
            assert!(!next.values.contains_key("override"));
            assert!(!next.origins.contains_key(&path(&["newOther"])));
            assert!(
                !next
                    .origins
                    .contains_key(&path(&["nested", "freshSibling"]))
            );
            assert_eq!(
                adapter(&scratch).read(scope).unwrap(),
                support::map(disk_target.clone())
            );
            assert_eq!(bytes(&scratch, other), untouched);
            let refreshed = s.reload().unwrap();
            assert_eq!(s.read_scope(scope), support::map(disk_target));
            assert_eq!(
                s.read_scope(other),
                support::map(json!({"other":8,"newOther":true}))
            );
            assert_eq!(refreshed.values["other"], json!(8));
            assert_eq!(refreshed.values["newOther"], json!(true));
            assert_eq!(
                refreshed.origins[&path(&["newOther"])],
                if other == SettingsScope::User {
                    SettingsOrigin::User
                } else {
                    SettingsOrigin::Project
                }
            );
            assert_eq!(open(&scratch).unwrap().resolve(), refreshed);
        }
    }
}

#[test]
fn stored_saves_validate_cached_and_fresh_candidates() {
    for scope in [SettingsScope::User, SettingsScope::Project] {
        let other = if scope == SettingsScope::User {
            SettingsScope::Project
        } else {
            SettingsScope::User
        };
        for (cached, fresh, segments, value, invalid_path) in [
            (
                json!({"a":{"locked":1,"open":2}}),
                json!({"a":{"locked":1,"external":3}}),
                path(&["a"]),
                json!({"open":4}),
                false,
            ),
            (
                json!({"a":{"locked":1}}),
                json!({"a":{"locked":1,"external":3}}),
                vec![],
                json!({"other":4}),
                false,
            ),
            (
                json!({"a":{"locked":1}}),
                json!({"a":{"locked":1,"external":3}}),
                path(&["a", "locked"]),
                json!(9),
                false,
            ),
            (
                json!({"a":{"locked":1}}),
                json!({"a":{"locked":9}}),
                path(&["added"]),
                json!(4),
                false,
            ),
            (
                json!({"a":{"locked":1},"nonobject":0}),
                json!({"a":{"locked":1},"nonobject":{}}),
                path(&["nonobject", "child"]),
                json!(4),
                true,
            ),
            (
                json!({"a":{"locked":1},"object":{}}),
                json!({"a":{"locked":1},"object":0}),
                path(&["object", "child"]),
                json!(4),
                true,
            ),
        ] {
            let scratch = Scratch::new();
            scratch.write(scope, &cached);
            scratch.write(other, &json!({"a":{"locked":1}}));
            let mut s = Settings::new(
                Map::new(),
                ManifestSettings {
                    values: support::map(json!({"a":{"locked":1}})),
                    locks: vec![path(&["a", "locked"])],
                },
                Box::new(adapter(&scratch)),
            )
            .unwrap();
            s.apply_overrides("runtime".into(), support::map(json!({"override":true})))
                .unwrap();
            let before = s.resolve();
            let user = s.read_scope(SettingsScope::User);
            let project = s.read_scope(SettingsScope::Project);
            scratch.write(scope, &fresh);
            let original = bytes(&scratch, scope);
            let other_bytes = bytes(&scratch, other);
            let error = s
                .set(SettingsTarget::Stored(scope), &segments, value)
                .unwrap_err();
            let origin = if scope == SettingsScope::User {
                SettingsOrigin::User
            } else {
                SettingsOrigin::Project
            };
            if invalid_path {
                assert_eq!(
                    error,
                    SettingsError::InvalidPath {
                        path: segments,
                        origin
                    }
                );
            } else {
                assert_eq!(
                    error,
                    SettingsError::LockConflict {
                        path: path(&["a", "locked"]),
                        locked_by: SettingsOrigin::Manifest,
                        attempted_by: origin
                    }
                );
            }
            assert_eq!(s.resolve(), before);
            assert_eq!(s.read_scope(SettingsScope::User), user);
            assert_eq!(s.read_scope(SettingsScope::Project), project);
            assert_eq!(bytes(&scratch, scope), original);
            assert_eq!(bytes(&scratch, other), other_bytes);
            assert!(s.drain_errors().is_empty());
        }
    }
}
