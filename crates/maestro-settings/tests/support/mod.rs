use maestro_settings::*;

use serde_json::{Map, Value, json};

pub type StorageFactory = fn(Map<String, Value>, Map<String, Value>) -> Box<dyn SettingsStorage>;
pub fn map(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}
pub fn path(segments: &[&str]) -> Vec<String> {
    segments.iter().map(|s| (*s).to_owned()).collect()
}
pub fn memory(user: Map<String, Value>, project: Map<String, Value>) -> Box<dyn SettingsStorage> {
    Box::new(MemorySettingsStorage::new(user, project))
}
pub fn settings(
    factory: StorageFactory,
    engine: Value,
    manifest: Value,
    locks: &[&[&str]],
    user: Value,
    project: Value,
) -> Result<Settings, maestro_settings::SettingsError> {
    Settings::new(
        map(engine),
        ManifestSettings {
            values: map(manifest),
            locks: locks.iter().map(|p| path(p)).collect(),
        },
        factory(map(user), map(project)),
    )
}

pub fn caller_sequence(factory: StorageFactory) {
    use maestro_settings::{SettingsOrigin, SettingsScope, SettingsTarget};
    use serde_json::json;
    let mut s = settings(
        factory,
        json!({}),
        json!({"frozen":1}),
        &[&["frozen"]],
        json!({"key":"user","untouched":1}),
        json!({"key":"project"}),
    )
    .unwrap();
    let edited = s
        .set(
            SettingsTarget::Stored(SettingsScope::User),
            &path(&["key"]),
            json!("new-user"),
        )
        .unwrap();
    assert_eq!(edited.values["key"], json!("project"));
    assert_eq!(edited.origins[&path(&["key"])], SettingsOrigin::Project);
    s.set(
        SettingsTarget::Stored(SettingsScope::Project),
        &path(&["added"]),
        json!(3),
    )
    .unwrap();
    s.set(
        SettingsTarget::Override("cli".into()),
        &path(&["key"]),
        json!("override"),
    )
    .unwrap();
    assert_eq!(s.reload().unwrap().values["key"], json!("project"));
    assert_eq!(s.resolve().values["added"], json!(3));
    assert_eq!(s.resolve().values["untouched"], json!(1));
    assert!(
        s.set(
            SettingsTarget::Stored(SettingsScope::User),
            &path(&["frozen"]),
            json!(2)
        )
        .is_err()
    );
    assert_eq!(s.reload().unwrap().values["frozen"], json!(1));
}

pub struct ControlledStorage {
    pub maps: std::sync::Arc<std::sync::Mutex<[Map<String, Value>; 2]>>,
    pub fail: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl SettingsStorage for ControlledStorage {
    fn read(&mut self, scope: SettingsScope) -> Result<Map<String, Value>, SettingsError> {
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(SettingsError::Storage { scope });
        }
        Ok(self.maps.lock().unwrap()[match scope {
            SettingsScope::User => 0,
            SettingsScope::Project => 1,
        }]
        .clone())
    }
    fn transact(
        &mut self,
        scope: maestro_settings::SettingsScope,
        edit: &mut maestro_settings::SettingsTransaction<'_>,
    ) -> Result<(), maestro_settings::SettingsError> {
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(maestro_settings::SettingsError::Storage { scope });
        }
        let mut maps = self.maps.lock().unwrap();
        let index = match scope {
            maestro_settings::SettingsScope::User => 0,
            maestro_settings::SettingsScope::Project => 1,
        };
        if let Some(next) = edit(&maps[index])? {
            maps[index] = next;
        }
        Ok(())
    }
}
pub fn controlled(
    user: Map<String, Value>,
    project: Map<String, Value>,
) -> Box<dyn SettingsStorage> {
    Box::new(ControlledStorage {
        maps: std::sync::Arc::new(std::sync::Mutex::new([user, project])),
        fail: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    })
}

pub fn precedence_retains_leaf_origins(factory: StorageFactory) {
    let s = settings(
        factory,
        json!({"engine":1,"all":0,"nested":{"e":1,"same":0}}),
        json!({"manifest":2,"all":1,"nested":{"m":2,"same":1}}),
        &[],
        json!({"user":3,"all":2,"nested":{"u":3,"same":2}}),
        json!({"project":4,"all":3,"nested":{"p":4,"same":3}}),
    )
    .unwrap()
    .resolve();
    assert_eq!(s.values["all"], json!(3));
    assert_eq!(
        s.values["nested"],
        json!({"e":1,"m":2,"u":3,"p":4,"same":3})
    );
    for (key, origin) in [
        ("e", SettingsOrigin::Engine),
        ("m", SettingsOrigin::Manifest),
        ("u", SettingsOrigin::User),
        ("p", SettingsOrigin::Project),
        ("same", SettingsOrigin::Project),
    ] {
        assert_eq!(s.origins[&path(&["nested", key])], origin);
    }
    for (key, origin) in [
        ("engine", SettingsOrigin::Engine),
        ("manifest", SettingsOrigin::Manifest),
        ("user", SettingsOrigin::User),
        ("project", SettingsOrigin::Project),
        ("all", SettingsOrigin::Project),
    ] {
        assert_eq!(s.origins[&path(&[key])], origin);
    }
}

pub fn recursive_objects_preserve_unmentioned_descendants(factory: StorageFactory) {
    let s = settings(
        factory,
        json!({"a":{"b":{"c":{"keep":1,"change":0}}}}),
        json!({"a":{"b":{"c":{"change":2}}}}),
        &[],
        json!({"a":{"b":{"user":3}}}),
        json!({"a":{"b":{"c":{"change":4}}}}),
    )
    .unwrap()
    .resolve();
    assert_eq!(
        s.values["a"],
        json!({"b":{"c":{"keep":1,"change":4},"user":3}})
    );
    assert_eq!(
        s.origins[&path(&["a", "b", "c", "keep"])],
        SettingsOrigin::Engine
    );
    assert_eq!(
        s.origins[&path(&["a", "b", "c", "change"])],
        SettingsOrigin::Project
    );
}

pub fn scalars_arrays_and_null_replace(factory: StorageFactory) {
    let s = settings(
        factory,
        json!({"object":{"stale":1},"scalar":1,"array":[1,2],"null":null,"empty":{}}),
        json!({"object":null,"scalar":{"leaf":2},"array":[3],"null":[],"empty":{"leaf":4}}),
        &[],
        json!({"scalar":[],"empty":false}),
        json!({"null":{}}),
    )
    .unwrap()
    .resolve();
    for (key, value) in map(json!({"object":null,"scalar":[],"array":[3],"null":{},"empty":false}))
    {
        assert_eq!(s.values[&key], value);
    }
    assert!(!s.origins.contains_key(&path(&["object", "stale"])));
    assert!(!s.origins.contains_key(&path(&["scalar", "leaf"])));
    assert!(!s.origins.contains_key(&path(&["empty", "leaf"])));
    for (key, origin) in [
        ("object", SettingsOrigin::Manifest),
        ("scalar", SettingsOrigin::User),
        ("array", SettingsOrigin::Manifest),
        ("null", SettingsOrigin::Project),
        ("empty", SettingsOrigin::User),
    ] {
        assert_eq!(s.origins[&path(&[key])], origin);
    }
}

pub fn resource_lists_replace_without_discovery(factory: StorageFactory) {
    let s = settings(factory, json!({"packages":["engine"],"extensions":["engine"],"skills":["engine"],"promptTemplates":["engine"]}), json!({"packages":["manifest"],"extensions":["manifest"],"skills":["manifest"],"promptTemplates":["manifest"]}), &[], json!({"skills":[]}), json!({"packages":["project"]})).unwrap().resolve();
    assert_eq!(s.values["packages"], json!(["project"]));
    assert_eq!(s.values["extensions"], json!(["manifest"]));
    assert_eq!(s.values["skills"], json!([]));
    assert_eq!(s.values["promptTemplates"], json!(["manifest"]));
}

pub fn literal_segments_do_not_split_keys(factory: StorageFactory) {
    let mut s = settings(
        factory,
        json!({"a.b":1,"a":{"b":2},"a/b":3,"~":4,"":5,"array":[0]}),
        json!({}),
        &[],
        json!({}),
        json!({}),
    )
    .unwrap();
    for (segments, value) in [
        (vec!["a.b"], 10),
        (vec!["a", "b"], 20),
        (vec!["a/b"], 30),
        (vec!["~"], 40),
        (vec![""], 50),
        (vec!["new", "nested"], 60),
    ] {
        s.set(
            SettingsTarget::Override("cli".into()),
            &path(&segments),
            json!(value),
        )
        .unwrap();
    }
    for (key, value) in
        map(json!({"a.b":10,"a":{"b":20},"a/b":30,"~":40,"":50,"array":[0],"new":{"nested":60}}))
    {
        assert_eq!(s.resolve().values[&key], value);
    }
    for segments in [&["a.b", "child"][..], &["array", "0"][..], &[][..]] {
        let before = s.resolve();
        assert_eq!(
            s.set(
                SettingsTarget::Override("cli".into()),
                &path(segments),
                json!(99)
            ),
            Err(SettingsError::InvalidPath {
                path: path(segments),
                origin: SettingsOrigin::Override("cli".into())
            })
        );
        assert_eq!(s.resolve(), before);
    }
    let replaced = s
        .set(
            SettingsTarget::Override("runtime".into()),
            &[],
            json!({"only":1}),
        )
        .unwrap();
    assert_eq!(replaced.values, map(json!({"only":1})));
}

pub fn manifest_locks_freeze_values_and_subtrees(factory: StorageFactory) {
    let mut s = settings(
        factory,
        json!({"engine":7}),
        json!({"scalar":"frozen","null":null,"object":{"child":1}}),
        &[&["engine"], &["scalar"], &["null"], &["object"]],
        json!({}),
        json!({}),
    )
    .unwrap();
    for (segments, value, lock) in [
        (vec!["engine"], json!(8), "engine"),
        (vec!["scalar"], json!("changed"), "scalar"),
        (vec!["null"], json!(0), "null"),
        (vec!["object", "child"], json!(2), "object"),
        (vec!["object", "added"], json!(1), "object"),
    ] {
        assert_eq!(
            s.set(
                SettingsTarget::Override("cli".into()),
                &path(&segments),
                value
            ),
            Err(SettingsError::LockConflict {
                path: path(&[lock]),
                locked_by: SettingsOrigin::Manifest,
                attempted_by: SettingsOrigin::Override("cli".into())
            })
        );
    }
    assert_eq!(
        s.set(
            SettingsTarget::Override("cli".into()),
            &path(&["unlocked"]),
            json!(true)
        )
        .unwrap()
        .values["unlocked"],
        json!(true)
    );
}

pub fn equal_restatement_changes_origin_not_lock_authority(factory: StorageFactory) {
    let mut s = settings(
        factory,
        json!({}),
        json!({"scalar":1,"array":[1,2],"object":{"child":3}}),
        &[
            &["scalar"],
            &["array"],
            &["object"],
            &["object", "child"],
            &["scalar"],
        ],
        json!({"scalar":1}),
        json!({"array":[1,2],"object":{"child":3}}),
    )
    .unwrap();
    assert_eq!(
        s.resolve().origins[&path(&["scalar"])],
        SettingsOrigin::User
    );
    assert_eq!(
        s.resolve().origins[&path(&["object", "child"])],
        SettingsOrigin::Project
    );
    for (key, value) in [
        ("scalar", json!(1)),
        ("array", json!([1, 2])),
        ("object", json!({"child":3})),
    ] {
        s.set(
            SettingsTarget::Override("runtime".into()),
            &path(&[key]),
            value,
        )
        .unwrap();
    }
    assert_eq!(
        s.resolve().origins[&path(&["object", "child"])],
        SettingsOrigin::Override("runtime".into())
    );
    assert_eq!(s.resolve().locks.len(), 5);
    assert_eq!(
        s.set(
            SettingsTarget::Override("cli".into()),
            &path(&["scalar"]),
            json!(2)
        ),
        Err(SettingsError::LockConflict {
            path: path(&["scalar"]),
            locked_by: SettingsOrigin::Manifest,
            attempted_by: SettingsOrigin::Override("cli".into())
        })
    );
}

pub fn locks_reject_ancestor_replacement_and_omission(factory: StorageFactory) {
    let mut s = settings(
        factory,
        json!({"a":{"child":"frozen","other":0}}),
        json!({}),
        &[&["a", "child"]],
        json!({"a":{"child":"frozen"}}),
        json!({}),
    )
    .unwrap();
    for value in [json!(0), json!(null), json!([]), json!({"other":1})] {
        for target in [
            SettingsTarget::Override("cli".into()),
            SettingsTarget::Stored(SettingsScope::User),
        ] {
            assert!(matches!(
                s.set(target, &path(&["a"]), value.clone()),
                Err(SettingsError::LockConflict { .. })
            ));
        }
    }
    for value in [json!({}), json!({"a":{"other":1}})] {
        assert!(matches!(
            s.set(SettingsTarget::Stored(SettingsScope::User), &[], value),
            Err(SettingsError::LockConflict { .. })
        ));
    }
    assert!(
        s.set(
            SettingsTarget::Override("cli".into()),
            &path(&["a"]),
            json!({"child":"frozen"})
        )
        .is_ok()
    );
}

pub fn array_locks_are_atomic(factory: StorageFactory) {
    let mut s = settings(
        factory,
        json!({}),
        json!({"array":[1,2]}),
        &[&["array"]],
        json!({}),
        json!({}),
    )
    .unwrap();
    assert!(
        s.set(
            SettingsTarget::Override("cli".into()),
            &path(&["array"]),
            json!([1, 2])
        )
        .is_ok()
    );
    for value in [json!([1, 3]), json!([2, 1]), json!([1]), json!([1, 2, 3])] {
        assert!(matches!(
            s.set(
                SettingsTarget::Override("cli".into()),
                &path(&["array"]),
                value
            ),
            Err(SettingsError::LockConflict { .. })
        ));
    }
    assert!(matches!(
        s.set(
            SettingsTarget::Override("cli".into()),
            &path(&["array", "0"]),
            json!(1)
        ),
        Err(SettingsError::InvalidPath { .. })
    ));
    assert!(matches!(
        settings(
            factory,
            json!({}),
            json!({"array":[1,2]}),
            &[&["array", "0"]],
            json!({}),
            json!({})
        ),
        Err(SettingsError::InvalidLock { .. })
    ));
}

pub fn invalid_and_missing_manifest_locks_fail(factory: StorageFactory) {
    for segments in [
        &[][..],
        &["scalar", "x"][..],
        &["null", "x"][..],
        &["array", "0"][..],
    ] {
        assert!(
            matches!(settings(factory,json!({"scalar":1,"null":null,"array":[]}),json!({}),&[segments],json!({}),json!({})), Err(SettingsError::InvalidLock{path:p}) if p == path(segments))
        );
    }
    for segments in [
        &["absent"][..],
        &["nested", "absent"][..],
        &["userOnly"][..],
        &["projectOnly"][..],
    ] {
        assert!(
            matches!(settings(factory,json!({"nested":{}}),json!({}),&[segments],json!({"userOnly":1}),json!({"projectOnly":2})),Err(SettingsError::MissingLockTarget{path:p}) if p == path(segments))
        );
    }
    assert!(
        settings(
            factory,
            json!({"null":null}),
            json!({}),
            &[&["null"]],
            json!({}),
            json!({})
        )
        .is_ok()
    );
}

pub fn masked_conflicts_fail_at_their_layer(factory: StorageFactory) {
    assert!(matches!(
        settings(
            factory,
            json!({}),
            json!({"key":"frozen"}),
            &[&["key"]],
            json!({"key":"wrong"}),
            json!({"key":"frozen"})
        ),
        Err(SettingsError::LockConflict {
            attempted_by: SettingsOrigin::User,
            ..
        })
    ));
    let mut s = settings(
        factory,
        json!({}),
        json!({"key":"frozen"}),
        &[&["key"]],
        json!({"key":"frozen"}),
        json!({"key":"frozen"}),
    )
    .unwrap();
    let before = s.resolve();
    assert!(matches!(
        s.set(
            SettingsTarget::Stored(SettingsScope::User),
            &path(&["key"]),
            json!("wrong")
        ),
        Err(SettingsError::LockConflict {
            attempted_by: SettingsOrigin::User,
            ..
        })
    ));
    assert_eq!(s.resolve(), before);
}

pub fn all_caller_origins_use_the_same_guard(factory: StorageFactory) {
    let targets = [
        SettingsTarget::Override("cli".into()),
        SettingsTarget::Override("environment".into()),
        SettingsTarget::Override("runtime".into()),
        SettingsTarget::Override("extension:sample".into()),
        SettingsTarget::Stored(SettingsScope::User),
        SettingsTarget::Stored(SettingsScope::Project),
    ];
    for target in targets {
        let origin = match &target {
            SettingsTarget::Override(s) => SettingsOrigin::Override(s.clone()),
            SettingsTarget::Stored(SettingsScope::User) => SettingsOrigin::User,
            SettingsTarget::Stored(SettingsScope::Project) => SettingsOrigin::Project,
        };
        let mut s = settings(
            factory,
            json!({}),
            json!({"frozen":1}),
            &[&["frozen"]],
            json!({}),
            json!({}),
        )
        .unwrap();
        assert_eq!(
            s.set(target.clone(), &path(&["frozen"]), json!(1))
                .unwrap()
                .origins[&path(&["frozen"])],
            origin
        );
        assert_eq!(
            s.set(target.clone(), &path(&["open"]), json!(2))
                .unwrap()
                .origins[&path(&["open"])],
            origin
        );
        assert_eq!(
            s.set(target, &path(&["frozen"]), json!(3)),
            Err(SettingsError::LockConflict {
                path: path(&["frozen"]),
                locked_by: SettingsOrigin::Manifest,
                attempted_by: origin
            })
        );
    }
}

pub fn rejected_updates_preserve_values_origins_and_storage(factory: StorageFactory) {
    let mut s = settings(
        factory,
        json!({}),
        json!({"secret":"FROZEN_SENTINEL"}),
        &[&["secret"]],
        json!({"keep":1}),
        json!({"other":2}),
    )
    .unwrap();
    let before = s.resolve();
    for target in [
        SettingsTarget::Override("cli".into()),
        SettingsTarget::Stored(SettingsScope::User),
        SettingsTarget::Stored(SettingsScope::Project),
    ] {
        let error = s
            .set(target, &path(&["secret"]), json!("ATTEMPT_SENTINEL"))
            .unwrap_err();
        for text in [error.to_string(), format!("{error:?}")] {
            assert!(text.contains("Manifest"));
            assert!(!text.contains("FROZEN_SENTINEL"));
            assert!(!text.contains("ATTEMPT_SENTINEL"));
            assert!(!text.contains("{\"secret\""));
        }
        assert_eq!(s.resolve(), before);
        assert_eq!(s.reload().unwrap(), before);
    }
}

pub fn ordinary_settings_cannot_change_lock_authority(factory: StorageFactory) {
    let mut s = settings(
        factory,
        json!({}),
        json!({"key":1,"other":2}),
        &[&["key"]],
        json!({"locks":[]}),
        json!({"locks":[["other"]]}),
    )
    .unwrap();
    assert_eq!(s.resolve().locks, vec![path(&["key"])]);
    s.set(
        SettingsTarget::Override("cli".into()),
        &path(&["locks"]),
        json!([]),
    )
    .unwrap();
    assert_eq!(s.resolve().locks, vec![path(&["key"])]);
    assert!(
        s.set(
            SettingsTarget::Override("cli".into()),
            &path(&["other"]),
            json!(3)
        )
        .is_ok()
    );
    assert!(matches!(
        s.set(
            SettingsTarget::Override("cli".into()),
            &path(&["key"]),
            json!(2)
        ),
        Err(SettingsError::LockConflict { .. })
    ));
}

pub fn memory_initial_values_and_updates_survive_reload(factory: StorageFactory) {
    {
        let mut s = settings(
            factory,
            json!({}),
            json!({"frozen":1}),
            &[&["frozen"]],
            json!({"initialUser":1,"user":{"keep":1}}),
            json!({"initialProject":2}),
        )
        .unwrap();
        s.set(
            SettingsTarget::Stored(SettingsScope::User),
            &path(&["user", "added"]),
            json!(3),
        )
        .unwrap();
        s.set(
            SettingsTarget::Stored(SettingsScope::Project),
            &path(&["project"]),
            json!(4),
        )
        .unwrap();
        let before = s.resolve();
        for _ in 0..3 {
            assert_eq!(s.reload().unwrap(), before);
        }
        assert_eq!(before.values["initialUser"], json!(1));
        assert_eq!(before.values["initialProject"], json!(2));
        assert_eq!(before.values["user"], json!({"keep":1,"added":3}));
        assert_eq!(
            before.origins[&path(&["user", "added"])],
            SettingsOrigin::User
        );
        assert_eq!(before.origins[&path(&["project"])], SettingsOrigin::Project);
        assert_eq!(before.locks, vec![path(&["frozen"])]);
    }
}

pub fn overrides_are_ephemeral_and_reload_restores_stored_values(factory: StorageFactory) {
    {
        let mut s = settings(
            factory,
            json!({}),
            json!({"frozen":1}),
            &[&["frozen"]],
            json!({"key":"stored"}),
            json!({}),
        )
        .unwrap();
        s.set(
            SettingsTarget::Stored(SettingsScope::User),
            &path(&["accepted"]),
            json!(2),
        )
        .unwrap();
        let stored = s.resolve();
        assert_eq!(
            s.set(
                SettingsTarget::Override("runtime".into()),
                &path(&["key"]),
                json!("ephemeral")
            )
            .unwrap()
            .origins[&path(&["key"])],
            SettingsOrigin::Override("runtime".into())
        );
        assert_eq!(s.reload().unwrap(), stored);
        assert!(
            s.set(
                SettingsTarget::Override("runtime".into()),
                &path(&["frozen"]),
                json!(2)
            )
            .is_err()
        );
        s.set(
            SettingsTarget::Override("runtime".into()),
            &path(&["key"]),
            json!("ephemeral"),
        )
        .unwrap();
        let next = s
            .set(
                SettingsTarget::Stored(SettingsScope::Project),
                &path(&["project"]),
                json!(3),
            )
            .unwrap();
        assert_eq!(next.values["key"], json!("stored"));
        assert_eq!(next.values["accepted"], json!(2));
        assert_eq!(next.origins[&path(&["key"])], SettingsOrigin::User);
        assert_eq!(s.reload().unwrap(), next);
    }
}

pub fn snapshots_are_detached(factory: StorageFactory) {
    let mut s = settings(
        factory,
        json!({}),
        json!({"frozen":{"child":1}}),
        &[&["frozen"]],
        json!({}),
        json!({}),
    )
    .unwrap();
    let original = s.resolve();
    let mut detached = original.clone();
    detached.values["frozen"] = json!({"child":2});
    detached.origins.clear();
    detached.locks.clear();
    assert_eq!(s.resolve(), original);
    assert!(
        s.set(
            SettingsTarget::Override("cli".into()),
            &path(&["frozen", "child"]),
            json!(2)
        )
        .is_err()
    );
}

pub fn defaults_cover_omitted_explicit_and_disabled(factory: StorageFactory) {
    let s = settings(factory, json!({}), json!({}), &[], json!({}), json!({}))
        .unwrap()
        .resolve();
    let expected = map(
        json!({"packages":[],"extensions":[],"skills":[],"promptTemplates":[],"enableSkillCommands":true,"quietStartup":false,"hideThinking":false,"collapseChangelog":false}),
    );
    assert_eq!(s.values, expected);
    for key in expected.keys() {
        assert_eq!(s.origins[&path(&[key])], SettingsOrigin::Engine);
    }
    for boolean in [true, false] {
        let mut layer = map(
            json!({"packages":["p"],"extensions":["e"],"skills":["s"],"promptTemplates":["t"]}),
        );
        for key in [
            "enableSkillCommands",
            "quietStartup",
            "hideThinking",
            "collapseChangelog",
        ] {
            layer.insert(key.into(), json!(boolean));
        }
        let explicit = settings(factory, json!(layer), json!({}), &[], json!({}), json!({}))
            .unwrap()
            .resolve();
        assert_eq!(explicit.values, layer);
        let cleared = settings(
            factory,
            json!(layer),
            json!({}),
            &[],
            json!({"packages":[],"extensions":[],"skills":[],"promptTemplates":[]}),
            json!({}),
        )
        .unwrap()
        .resolve();
        for key in ["packages", "extensions", "skills", "promptTemplates"] {
            assert_eq!(cleared.values[key], json!([]));
            assert_eq!(cleared.origins[&path(&[key])], SettingsOrigin::User);
        }
    }
}

mod scratch;
pub(crate) use scratch::Scratch;
struct OwnedFile {
    storage: FileSettingsStorage,
    _scratch: Scratch,
}
impl SettingsStorage for OwnedFile {
    fn read(&mut self, scope: SettingsScope) -> Result<Map<String, Value>, SettingsError> {
        self.storage.read(scope)
    }
    fn transact(
        &mut self,
        scope: SettingsScope,
        edit: &mut SettingsTransaction<'_>,
    ) -> Result<(), SettingsError> {
        self.storage.transact(scope, edit)
    }
}
pub fn file(user: Map<String, Value>, project: Map<String, Value>) -> Box<dyn SettingsStorage> {
    let scratch = Scratch::new();
    scratch.write(SettingsScope::User, &Value::Object(user));
    scratch.write(SettingsScope::Project, &Value::Object(project));
    Box::new(OwnedFile {
        storage: FileSettingsStorage::new(
            scratch.locations(),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        ),
        _scratch: scratch,
    })
}

pub fn scoped_reads_keep_shadowed_values(factory: StorageFactory) {
    let user = json!({"packages":["u"],"nested":{"leaf":1}});
    let project = json!({"packages":["p"],"projectOnly":2});
    let mut s = settings(
        factory,
        json!({"engineOnly":true}),
        json!({"manifestOnly":true}),
        &[],
        user.clone(),
        project.clone(),
    )
    .unwrap();
    assert_eq!(s.resolve().values["packages"], json!(["p"]));
    s.set(
        SettingsTarget::Override("runtime".into()),
        &path(&["ephemeral"]),
        json!(true),
    )
    .unwrap();
    assert_eq!(s.read_scope(SettingsScope::User), map(user.clone()));
    assert_eq!(s.read_scope(SettingsScope::Project), map(project.clone()));
    let mut detached = s.read_scope(SettingsScope::User);
    detached["nested"]["leaf"] = json!(99);
    detached.insert("extra".into(), json!(true));
    assert_eq!(s.read_scope(SettingsScope::User), map(user.clone()));
    assert_eq!(s.resolve().values["nested"]["leaf"], json!(1));
    s.set(
        SettingsTarget::Stored(SettingsScope::User),
        &path(&["unrelated"]),
        json!(3),
    )
    .unwrap();
    s.reload().unwrap();
    assert_eq!(
        s.read_scope(SettingsScope::User),
        map(json!({"packages":["u"],"nested":{"leaf":1},"unrelated":3}))
    );
    assert_eq!(s.read_scope(SettingsScope::Project), map(project));
}

pub fn override_overlay_keeps_unmentioned_values(factory: StorageFactory) {
    let user = json!({"a":{"x":1,"y":3,"deep":{"keep":4,"change":0}},"scalar":{"old":1},"nullable":1,"array":[1,2],"unrelated":7});
    let mut s = settings(factory, json!({}), json!({}), &[], user.clone(), json!({})).unwrap();
    let stored = s.resolve();
    let next = s
        .apply_overrides(
            "runtime".into(),
            map(json!({"a":{"x":2,"deep":{"change":5}},"scalar":2,"nullable":null,"array":[3]})),
        )
        .unwrap();
    assert_eq!(
        next.values["a"],
        json!({"x":2,"y":3,"deep":{"keep":4,"change":5}})
    );
    assert_eq!(next.values["scalar"], json!(2));
    assert_eq!(next.values["nullable"], json!(null));
    assert_eq!(next.values["array"], json!([3]));
    for segments in [
        &["a", "x"][..],
        &["a", "deep", "change"],
        &["scalar"],
        &["nullable"],
        &["array"],
    ] {
        assert_eq!(
            next.origins[&path(segments)],
            SettingsOrigin::Override("runtime".into())
        );
    }
    for segments in [&["a", "y"][..], &["a", "deep", "keep"], &["unrelated"]] {
        assert_eq!(next.origins[&path(segments)], SettingsOrigin::User);
    }
    let later = s
        .apply_overrides("extension:sample".into(), map(json!({"flag":true})))
        .unwrap();
    assert_eq!(later.values["a"], next.values["a"]);
    assert_eq!(later.values["unrelated"], json!(7));
    assert_eq!(
        later.origins[&path(&["a", "x"])],
        SettingsOrigin::Override("runtime".into())
    );
    assert_eq!(
        later.origins[&path(&["flag"])],
        SettingsOrigin::Override("extension:sample".into())
    );
    assert_eq!(s.read_scope(SettingsScope::User), map(user.clone()));
    assert_eq!(s.read_scope(SettingsScope::Project), map(json!({})));
    assert_eq!(s.reload().unwrap(), stored);
    s.apply_overrides("runtime".into(), map(json!({"flag":true})))
        .unwrap();
    let saved = s
        .set(
            SettingsTarget::Stored(SettingsScope::User),
            &path(&["a"]),
            json!({"only":8}),
        )
        .unwrap();
    assert_eq!(saved.values["a"], json!({"only":8}));
    assert!(!saved.values.contains_key("flag"));
    assert_eq!(s.reload().unwrap(), saved);
    assert_eq!(
        s.set(
            SettingsTarget::Override("runtime".into()),
            &[],
            json!({"only":1})
        )
        .unwrap()
        .values,
        map(json!({"only":1}))
    );
}

pub fn override_overlay_respects_locks(factory: StorageFactory) {
    for source in ["cli", "environment", "runtime", "extension:sample"] {
        let mut s = settings(factory, json!({}), json!({"scalar":"FROZEN_SENTINEL","array":[1,2],"tree":{"child":3},"a":{"locked":4,"open":0}}), &[&["scalar"], &["array"], &["tree"], &["a","locked"]], json!({"user":1}), json!({"project":2})).unwrap();
        let stored = s.resolve();
        let equal = s
            .apply_overrides(
                source.into(),
                map(json!({"scalar":"FROZEN_SENTINEL","array":[1,2],"tree":{"child":3}})),
            )
            .unwrap();
        assert_eq!(equal.locks, stored.locks);
        assert_eq!(
            equal.origins[&path(&["scalar"])],
            SettingsOrigin::Override(source.into())
        );
        assert_eq!(
            s.apply_overrides(source.into(), map(json!({"a":{"open":5}})))
                .unwrap()
                .values["a"],
            json!({"locked":4,"open":5})
        );
        let before = s.resolve();
        let user = s.read_scope(SettingsScope::User);
        let project = s.read_scope(SettingsScope::Project);
        for (values, lock) in [
            (json!({"scalar":"ATTEMPT_SENTINEL"}), path(&["scalar"])),
            (json!({"array":[1,3]}), path(&["array"])),
            (json!({"tree":{"added":1}}), path(&["tree"])),
            (json!({"tree":null}), path(&["tree"])),
            (json!({"a":0}), path(&["a", "locked"])),
        ] {
            let error = s.apply_overrides(source.into(), map(values)).unwrap_err();
            assert_eq!(
                error,
                SettingsError::LockConflict {
                    path: lock,
                    locked_by: SettingsOrigin::Manifest,
                    attempted_by: SettingsOrigin::Override(source.into())
                }
            );
            assert!(!format!("{error} {error:?}").contains("SENTINEL"));
            assert_eq!(s.resolve(), before);
            assert_eq!(s.read_scope(SettingsScope::User), user);
            assert_eq!(s.read_scope(SettingsScope::Project), project);
        }
        assert_eq!(s.reload().unwrap(), stored);
    }
}
