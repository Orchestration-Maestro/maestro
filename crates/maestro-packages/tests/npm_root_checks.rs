//! npm root lookup through public existing-content operations.
mod support;
use maestro_packages::{InstalledSourceScope::User, PackageManager};
use maestro_settings::SettingsListEntry;
use serde_json::json;
use support::{manager, output};

#[test]
fn npm_command_selection_keeps_defaults_and_exact_bun() {
    for (configured, executable, args, root) in command_cases() {
        let (manager, _, effects) = manager(&json!({"npmCommand":configured}));
        effects.exists.set(true);
        effects
            .outputs
            .borrow_mut()
            .push_back(Ok(output("/home/bun/bin", "", Some(0))));
        assert_eq!(
            manager.get_installed_path("npm:pkg", User).unwrap(),
            Some(root.into())
        );
        assert_eq!(
            *effects.calls.borrow(),
            vec![(
                executable.into(),
                args.into_iter().map(str::to_owned).collect()
            )]
        );
    }
    for configured in [json!([4]), json!(["npm", null])] {
        let (manager, _, effects) = manager(&json!({"npmCommand":configured}));
        assert_eq!(
            manager
                .get_installed_path("npm:pkg", User)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidData
        );
        assert!(effects.calls.borrow().is_empty());
    }
    let (manager, _, _) = manager(&json!({"npmCommand":["",4]}));
    assert_eq!(
        manager
            .get_installed_path("npm:pkg", User)
            .unwrap_err()
            .to_string(),
        "Invalid npmCommand: first array entry must be a non-empty command"
    );
}
#[test]
fn npm_root_cache_tracks_all_arguments_and_nonempty_results() {
    let sequences: Vec<RootSequence> =
        serde_json::from_str(include_str!("root_sequences.json")).unwrap();
    assert_eq!(sequences.len(), 4);
    let mut unique = std::collections::HashSet::new();
    for row in sequences {
        assert!(unique.insert(row.commands.clone()));
        let (manager, settings, effects) = manager(&json!({}));
        effects.exists.set(true);
        effects
            .outputs
            .borrow_mut()
            .extend(row.outputs.iter().map(|text| Ok(output(text, "", Some(0)))));
        let mut paths = Vec::new();
        for command in row.commands {
            settings.borrow_mut().set_npm_command(Some(
                command.into_iter().map(SettingsListEntry::String).collect(),
            ));
            paths.push(manager.get_installed_path("npm:pkg", User).unwrap());
        }
        assert_eq!(
            paths,
            row.expected.into_iter().map(Some).collect::<Vec<_>>()
        );
        assert_eq!(*effects.calls.borrow(), row.calls);
    }
}
#[test]
fn root_lookup_failures_preserve_the_previous_cache_entry() {
    let (manager, settings, effects) = manager(&json!({"npmCommand":["first"]}));
    effects.exists.set(true);
    effects.outputs.borrow_mut().extend([
        Ok(output("/root/first", "", Some(0))),
        Err(std::io::Error::other("controlled lookup failure")),
        Ok(output("/root/other", "", Some(0))),
    ]);
    assert_eq!(
        manager.get_installed_path("npm:pkg", User).unwrap(),
        Some("/root/first/pkg".into())
    );
    settings
        .borrow_mut()
        .set_npm_command(Some(vec![SettingsListEntry::String("other".into())]));
    assert_eq!(
        manager
            .get_installed_path("npm:pkg", User)
            .unwrap_err()
            .to_string(),
        "Failed to run other root -g: controlled lookup failure"
    );
    settings
        .borrow_mut()
        .set_npm_command(Some(vec![SettingsListEntry::String("first".into())]));
    assert_eq!(
        manager.get_installed_path("npm:pkg", User).unwrap(),
        Some("/root/first/pkg".into())
    );
    settings
        .borrow_mut()
        .set_npm_command(Some(vec![SettingsListEntry::String("other".into())]));
    assert_eq!(
        manager.get_installed_path("npm:pkg", User).unwrap(),
        Some("/root/other/pkg".into())
    );
    assert_eq!(
        effects
            .calls
            .borrow()
            .iter()
            .map(|(command, _)| command.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "other", "other"]
    );
}
#[test]
fn captured_commands_preserve_stream_choice_and_failure_wrapper() {
    for (stdout, stderr, expected) in [
        (" root \n", "error", "root"),
        ("", " fallback \n", "fallback"),
        (" \n", "ignored", ""),
        ("\u{feff}root\u{feff}", "", "root"),
        ("\u{85}root\u{85}", "", "\u{85}root\u{85}"),
    ] {
        let (manager, _, effects) = manager(&json!({}));
        effects.exists.set(true);
        effects
            .outputs
            .borrow_mut()
            .push_back(Ok(output(stdout, stderr, Some(0))));
        let _ = manager.get_installed_path("npm:pkg", User).unwrap();
        assert_eq!(
            *effects.paths.borrow(),
            vec![maestro_path::join(&[expected, "pkg"])]
        );
    }
    for (response, expected) in [
        (Ok(output("out\n", "err\n", Some(1))), "err\n"),
        (Ok(output("out\n", "", Some(1))), "out\n"),
        (Ok(output("out", "err", None)), "err"),
        (Err(std::io::Error::other("spawn failed")), "spawn failed"),
    ] {
        let (manager, _, effects) = manager(&json!({"npmCommand":["runner","--flag","two words"]}));
        effects.outputs.borrow_mut().push_back(response);
        assert_eq!(
            manager
                .get_installed_path("npm:pkg", User)
                .unwrap_err()
                .to_string(),
            format!("Failed to run runner --flag two words root -g: {expected}")
        );
        assert!(effects.paths.borrow().is_empty());
    }
}
#[cfg(unix)]
#[test]
fn native_capture_matches_controlled_stream_results() {
    use maestro_packages::{NativePackageOperations, PackageOperations};
    let operations = NativePackageOperations::new(|_| false, std::rc::Rc::new(|| false));
    let child = operations
        .run_command_sync(
            "sh",
            &["-c".into(), "printf ' root \\n'; printf ignored >&2".into()],
        )
        .unwrap();
    assert_eq!(
        (child.status, child.stdout, child.stderr),
        (Some(0), " root \n".into(), "ignored".into())
    );
    let child = operations
        .run_command_sync(
            "sh",
            &[
                "-c".into(),
                "printf '\\377a'; printf err >&2; exit 3".into(),
            ],
        )
        .unwrap();
    assert_eq!(
        (child.status, child.stdout, child.stderr),
        (Some(3), "�a".into(), "err".into())
    );
    let child = operations
        .run_command_sync("sh", &["-c".into(), "printf ' fallback \n' >&2".into()])
        .unwrap();
    assert_eq!(
        (child.status, child.stdout, child.stderr),
        (Some(0), String::new(), " fallback \n".into())
    );
    let child = operations
        .run_command_sync(
            "sh",
            &[
                "-c".into(),
                "printf 'out\n'; printf 'err\n' >&2; exit 3".into(),
            ],
        )
        .unwrap();
    assert_eq!(
        (child.status, child.stdout, child.stderr),
        (Some(3), "out\n".into(), "err\n".into())
    );
    let child = NativePackageOperations::new(|_| true, std::rc::Rc::new(|| false))
        .run_command_sync("printf", &["fallback".into()])
        .unwrap();
    assert_eq!(child.stdout, "fallback");
}

/// Selected commands and their independently observed root/argv outcomes.
fn command_cases() -> Vec<(
    serde_json::Value,
    &'static str,
    Vec<&'static str>,
    &'static str,
)> {
    vec![
        (json!(null), "npm", vec!["root", "-g"], "/home/bun/bin/pkg"),
        (json!([]), "npm", vec!["root", "-g"], "/home/bun/bin/pkg"),
        (
            json!(["npm"]),
            "npm",
            vec!["root", "-g"],
            "/home/bun/bin/pkg",
        ),
        (json!([" "]), " ", vec!["root", "-g"], "/home/bun/bin/pkg"),
        (
            json!(["mise", "exec", "node@20", "--", "npm"]),
            "mise",
            vec!["exec", "node@20", "--", "npm", "root", "-g"],
            "/home/bun/bin/pkg",
        ),
        (
            json!(["bun"]),
            "bun",
            vec!["pm", "bin", "-g"],
            "/home/bun/install/global/node_modules/pkg",
        ),
        (
            json!(["/usr/bin/bun"]),
            "/usr/bin/bun",
            vec!["root", "-g"],
            "/home/bun/bin/pkg",
        ),
        (
            json!(["Bun"]),
            "Bun",
            vec!["root", "-g"],
            "/home/bun/bin/pkg",
        ),
    ]
}

/// A consumed sequence of commands, completions and expected public effects.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RootSequence {
    /// Full configured vectors in invocation order.
    commands: Vec<Vec<String>>,
    /// Completed stdout values.
    outputs: Vec<String>,
    /// Public existing-content paths.
    expected: Vec<String>,
    /// Completed command and argv pairs.
    calls: Vec<(String, Vec<String>)>,
}
