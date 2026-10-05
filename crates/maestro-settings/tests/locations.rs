#[allow(dead_code)]
mod support;
use maestro_settings::*;
use serde_json::json;
use std::path::{Path, PathBuf};
use support::*;
#[test]
fn explicit_locations_are_isolated_and_default_to_supplied_cwd() {
    let scratch = Scratch::new();
    let process = scratch.root.join("process");
    let home = scratch.root.join("home");
    let root = scratch.root.join("selected");
    let default =
        SettingsLocations::new(process.clone(), None, root.clone(), home.clone()).unwrap();
    assert_eq!(default.working_directory(), process);
    assert_eq!(default.configuration_directory(SettingsScope::User), root);
    let relative = SettingsLocations::new(
        process.clone(),
        Some("work".into()),
        "config".into(),
        home.clone(),
    )
    .unwrap();
    assert_eq!(relative.working_directory(), process.join("work"));
    assert_eq!(
        relative.configuration_directory(SettingsScope::User),
        process.join("work/config")
    );
    assert_eq!(
        relative.configuration_directory(SettingsScope::Project),
        process.join("work/.maestro")
    );
    let explicit = SettingsLocations::new(
        process.clone(),
        Some(scratch.root.join("other")),
        root.clone(),
        home.clone(),
    )
    .unwrap();
    assert_eq!(explicit.working_directory(), scratch.root.join("other"));
    for (cwd, supplied_home, input) in [
        (PathBuf::from("relative"), home.clone(), "process_cwd"),
        (process.clone(), PathBuf::from("relative"), "home_directory"),
    ] {
        assert!(
            matches!(SettingsLocations::new(cwd, None, root.clone(), supplied_home), Err(SettingsError::Location { input: i, origin: SettingsOrigin::Engine }) if i == input)
        );
    }
    let expanded = SettingsLocations::new(
        process,
        Some("~/work".into()),
        "~/config".into(),
        home.clone(),
    )
    .unwrap();
    assert_eq!(expanded.working_directory(), home.join("work"));
    assert_eq!(
        expanded.configuration_directory(SettingsScope::User),
        home.join("config")
    );
    assert!(!root.exists());
    caller_sequence(file);
}
#[test]
fn session_location_precedence_uses_one_resolution_path() {
    let scratch = Scratch::new();
    let locations = scratch.locations();
    for factory in [memory, file] {
        for (user, project, explicit, environment, expected) in [
            (
                json!({}),
                json!({}),
                None,
                None,
                locations
                    .configuration_directory(SettingsScope::User)
                    .join("sessions"),
            ),
            (
                json!({"sessionDir":"user"}),
                json!({}),
                None,
                Some(""),
                locations.working_directory().join("user"),
            ),
            (
                json!({"sessionDir":"user"}),
                json!({"sessionDir":"project"}),
                None,
                None,
                locations.working_directory().join("project"),
            ),
            (
                json!({"sessionDir":"user"}),
                json!({"sessionDir":"project"}),
                None,
                Some("environment"),
                locations.working_directory().join("environment"),
            ),
            (
                json!({}),
                json!({"sessionDir":false}),
                Some("cli"),
                Some("environment"),
                locations.working_directory().join("cli"),
            ),
            (
                json!({}),
                json!({}),
                Some(""),
                Some("environment"),
                locations.working_directory().to_owned(),
            ),
            (
                json!({}),
                json!({"sessionDir":""}),
                None,
                None,
                locations.working_directory().to_owned(),
            ),
            (
                json!({}),
                json!({}),
                Some("/absolute"),
                None,
                PathBuf::from("/absolute"),
            ),
            (
                json!({}),
                json!({}),
                Some("~"),
                None,
                scratch.root.join("home"),
            ),
            (
                json!({}),
                json!({"sessionDir":"~/sessions"}),
                None,
                None,
                scratch.root.join("home/sessions"),
            ),
        ] {
            let s = settings(factory, json!({}), json!({}), &[], user, project).unwrap();
            let before = s.resolve();
            assert_eq!(
                s.session_directory(&locations, explicit, environment)
                    .unwrap(),
                expected
            );
            assert_eq!(s.resolve(), before);
        }
        for (user, project, origin) in [
            (json!({"sessionDir":1}), json!({}), SettingsOrigin::User),
            (
                json!({}),
                json!({"sessionDir":null}),
                SettingsOrigin::Project,
            ),
            (
                json!({"sessionDir":{"a":1}}),
                json!({"sessionDir":{"z":2}}),
                SettingsOrigin::Project,
            ),
        ] {
            let s = settings(factory, json!({}), json!({}), &[], user, project).unwrap();
            assert_eq!(
                s.session_directory(&locations, None, None),
                Err(SettingsError::Location {
                    input: "sessionDir",
                    origin
                })
            );
        }
    }
    assert!(!locations.working_directory().exists());
}
#[test]
fn session_location_overrides_cannot_bypass_value_locks() {
    let scratch = Scratch::new();
    for factory in [memory, file] {
        let mut s = settings(
            factory,
            json!({}),
            json!({"sessionDir":"relative"}),
            &[&["sessionDir"]],
            json!({"sessionDir":"relative"}),
            json!({}),
        )
        .unwrap();
        let before = s.resolve();
        for (explicit, environment, origin) in [
            (Some("other"), None, "cli"),
            (None, Some("other"), "environment"),
            (
                Some(
                    scratch
                        .locations()
                        .working_directory()
                        .join("relative")
                        .to_str()
                        .unwrap(),
                ),
                None,
                "cli",
            ),
        ] {
            assert_eq!(
                s.session_directory(&scratch.locations(), explicit, environment),
                Err(SettingsError::LockConflict {
                    path: path(&["sessionDir"]),
                    locked_by: SettingsOrigin::Manifest,
                    attempted_by: SettingsOrigin::Override(origin.into())
                })
            );
            assert_eq!(s.resolve(), before);
        }
        assert_eq!(
            s.session_directory(&scratch.locations(), Some("relative"), Some("shadowed"))
                .unwrap(),
            scratch.locations().working_directory().join("relative")
        );
        assert!(
            s.session_directory(&scratch.locations(), None, Some("relative"))
                .is_ok()
        );
        assert_eq!(s.reload().unwrap(), before);
    }
}
#[test]
fn resource_and_invocation_paths_use_their_declared_bases() {
    let scratch = Scratch::new();
    let locations = scratch.locations();
    for base in [
        locations.configuration_directory(SettingsScope::User),
        locations.configuration_directory(SettingsScope::Project),
        scratch.root.join("manifest"),
        scratch.root.join("package"),
    ] {
        assert_eq!(
            locations.resource_path(Path::new("resource"), &base),
            base.join("resource")
        );
        assert_eq!(
            locations.resource_path(Path::new("/absolute"), &base),
            PathBuf::from("/absolute")
        );
        assert_eq!(
            locations.resource_path(Path::new("~/expanded"), &base),
            scratch.root.join("home/expanded")
        );
        assert_eq!(
            locations.resource_path(Path::new("~"), &base),
            scratch.root.join("home")
        );
    }
    assert_eq!(
        locations.resource_path(Path::new("resource"), Path::new("relative-package")),
        locations
            .working_directory()
            .join("relative-package/resource")
    );
    assert_eq!(
        locations.invocation_path(Path::new("resource")),
        locations.working_directory().join("resource")
    );
    assert_eq!(
        locations.invocation_path(Path::new("~other")),
        scratch.root.join("home/other")
    );
    assert_eq!(
        locations.invocation_path(Path::new("~/expanded")),
        scratch.root.join("home/expanded")
    );
    assert!(!locations.working_directory().exists());
}

#[test]
fn resource_paths_follow_normalization_rules() {
    let scratch = Scratch::new();
    let locations = scratch.locations();
    let cwd = locations.working_directory();
    let home = scratch.root.join("home");
    for base in [
        locations.configuration_directory(SettingsScope::User),
        locations.configuration_directory(SettingsScope::Project),
        scratch.root.join("manifest"),
        scratch.root.join("package"),
        cwd.join("relative-package"),
    ] {
        for (input, relative) in [
            (" ext.ts ", "ext.ts"),
            ("", ""),
            (" \t\n", ""),
            (".", ""),
            ("dir/../x", "x"),
            ("link/../x", "x"),
            ("dir///child/./x", "dir/child/x"),
            ("\u{FEFF}ext.ts\u{FEFF}", "ext.ts"),
            ("\u{85}ext.ts", "\u{85}ext.ts"),
        ] {
            assert_eq!(
                locations.invocation_path(Path::new(input)),
                cwd.join(relative)
            );
            assert_eq!(
                locations.resource_path(Path::new(input), &base),
                base.join(relative)
            );
        }
        for (input, expected) in [
            ("~", home.clone()),
            ("~/x", home.join("x")),
            ("~other", home.join("other")),
            ("~/dir/../x", home.join("x")),
            ("~dir/../x", home.join("x")),
            ("/absolute/dir/.././x", PathBuf::from("/absolute/x")),
            ("/../../x", PathBuf::from("/x")),
            ("~//x", home.join("x")),
        ] {
            assert_eq!(locations.invocation_path(Path::new(input)), expected);
            assert_eq!(locations.resource_path(Path::new(input), &base), expected);
        }
    }
    assert_eq!(
        locations.resource_path(Path::new(" dir/../x "), Path::new("relative-package")),
        cwd.join("relative-package/x")
    );
    assert!(!cwd.exists());
    std::fs::create_dir_all(cwd.join("link")).unwrap();
    assert_eq!(
        locations.invocation_path(Path::new("link/../x")),
        cwd.join("x")
    );
    #[cfg(unix)]
    {
        std::fs::remove_dir(cwd.join("link")).unwrap();
        std::os::unix::fs::symlink(scratch.root.join("missing"), cwd.join("link")).unwrap();
        assert_eq!(
            locations.invocation_path(Path::new("link/../x")),
            cwd.join("x")
        );
        use std::os::unix::ffi::OsStringExt;
        let native = PathBuf::from(std::ffi::OsString::from_vec(b"dir/../\xff".to_vec()));
        let filename = PathBuf::from(std::ffi::OsString::from_vec(vec![0xff]));
        assert_eq!(locations.invocation_path(&native), cwd.join(filename));
    }
    for (raw, equivalent) in [
        (" ext.ts ", "ext.ts"),
        ("~/x", home.join("x").to_str().unwrap()),
        ("dir/../x", "x"),
    ] {
        let s = settings(
            memory,
            json!({}),
            json!({"sessionDir":raw}),
            &[&["sessionDir"]],
            json!({}),
            json!({}),
        )
        .unwrap();
        assert_eq!(s.resolve().values["sessionDir"], json!(raw));
        assert_eq!(
            s.session_directory(&locations, Some(raw), None).unwrap(),
            locations.invocation_path(Path::new(raw))
        );
        for (explicit, environment) in [(Some(equivalent), None), (None, Some(equivalent))] {
            assert!(matches!(
                s.session_directory(&locations, explicit, environment),
                Err(SettingsError::LockConflict { .. })
            ));
        }
        assert_eq!(s.resolve().values["sessionDir"], json!(raw));
    }
}
