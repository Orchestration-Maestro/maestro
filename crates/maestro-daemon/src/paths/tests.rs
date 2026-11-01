//! Workspace home selection from supplied platform environments.
use super::*;

/// Windows skips unusable values and joins a drive only to a rooted home path.
#[test]
fn windows_home_selection_follows_environment_precedence() {
    let cases = [
        ([None, None, None, None], None),
        ([None, None, None, Some("")], None),
        ([None, None, None, Some("~")], None),
        ([None, None, None, Some("relative/.")], Some("relative/.")),
        ([Some("~"), None, None, None], None),
        ([Some(""), None, None, Some("home")], Some("home")),
        (
            [Some("profile"), Some("C:"), Some("\\users"), Some("home")],
            Some("profile"),
        ),
        (
            [Some("~"), Some("C:"), Some("\\users"), Some("home")],
            Some("C:\\users"),
        ),
        ([None, Some("C:"), Some("/users"), None], Some("C:/users")),
        (
            [None, Some("C:"), Some("users"), Some("home")],
            Some("home"),
        ),
        ([None, Some("C:"), Some("users"), None], None),
        (
            [None, Some(""), Some("\\users"), Some("home")],
            Some("home"),
        ),
        (
            [None, Some("~"), Some("\\users"), Some("home")],
            Some("home"),
        ),
        ([None, Some("C:"), Some("~"), Some("home")], Some("home")),
        ([None, Some("C:"), None, Some("home")], Some("home")),
        ([None, None, Some("\\users"), Some("home")], Some("home")),
    ];
    for (values, expected) in cases {
        let variables = ["USERPROFILE", "HOMEDRIVE", "HOMEPATH", "HOME"];
        let home = select_home(Platform::Windows, |key| {
            variables
                .iter()
                .position(|name| *name == key)
                .and_then(|index| values[index])
                .map(Into::into)
        });
        assert_eq!(home, expected.map(PathBuf::from), "{values:?}");
    }
}

/// Native Windows expansion and the public lookup share the same selected home.
#[cfg(windows)]
#[test]
fn windows_workspace_paths_use_the_selected_home() {
    let cases = [
        ("USERPROFILE", "C:\\profile"),
        ("HOME", "C:\\home"),
        ("USERPROFILE", "~"),
    ];
    let Some(cases) = maestro_test_process::run_per_environment(
        "paths::tests::windows_workspace_paths_use_the_selected_home",
        cases,
        |(key, value)| std::collections::BTreeMap::from([(*key, *value)]),
    )
    .unwrap() else {
        return;
    };
    for (_, value) in cases {
        let home = home_directory();
        let expected = (value != "~").then(|| PathBuf::from(value));
        assert_eq!(home, expected);
        for path in ["~", "~/x", "~\\x"] {
            let actual = maestro_path::expand_tilde_native(path, || home_directory().ok_or(()))
                .unwrap_or_else(|()| path.into());
            let expected = expected.as_ref().map_or_else(
                || PathBuf::from(path),
                |home| {
                    if path == "~" {
                        home.clone()
                    } else {
                        home.join("x")
                    }
                },
            );
            assert_eq!(actual, expected);
        }
    }
}

/// Mutable observations are read sequentially until a usable home is found.
#[test]
fn home_selection_accepts_mutable_observations() {
    let mut observed = Vec::new();
    let home = select_home(Platform::Windows, |key| {
        observed.push(key.to_owned());
        (key == "HOME").then(|| "home".into())
    });

    assert_eq!(home, Some(PathBuf::from("home")));
    assert_eq!(observed, ["USERPROFILE", "HOMEDRIVE", "HOMEPATH", "HOME"]);
}
