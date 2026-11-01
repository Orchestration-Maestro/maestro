//! Environment selection at the native command boundary.
#![cfg(test)]
use super::command_env;
use crate::utils::shell::ShellEnv;
use std::ffi::OsString;

#[test]
fn windows_environment_keeps_first_sorted_case_identity() {
    for reverse in [false, true] {
        let mut entries = vec![
            ("Path", "decoy"),
            ("PATH", "winner"),
            ("é", "lower"),
            ("É", "upper"),
            ("\u{e000}", "private"),
            ("\u{10000}", "astral"),
        ];
        if reverse {
            entries.reverse();
        }
        let env: ShellEnv = entries
            .into_iter()
            .map(|(key, value)| (OsString::from(key), Some(OsString::from(value))))
            .collect();
        let selected = command_env(&env, true);
        assert_eq!(
            selected
                .iter()
                .map(|(key, value)| (key.to_str().unwrap(), value.to_str().unwrap()))
                .collect::<Vec<_>>(),
            [
                ("PATH", "winner"),
                ("É", "upper"),
                ("\u{10000}", "astral"),
                ("\u{e000}", "private")
            ]
        );
        let mut builder = tokio::process::Command::new("unused");
        builder.env_clear().envs(selected);
        assert_eq!(
            builder
                .as_std()
                .get_envs()
                .find(|(key, _)| *key == "PATH")
                .unwrap()
                .1,
            Some(std::ffi::OsStr::new("winner"))
        );
        assert_eq!(
            command_env(&env, false),
            env.iter()
                .map(|(key, value)| (key, value.as_ref().unwrap()))
                .collect::<Vec<_>>()
        );
        assert_eq!(env.len(), 6);
    }
}

#[test]
#[cfg(unix)]
fn environment_selection_preserves_non_unicode_identity() {
    use std::os::unix::ffi::OsStringExt;
    let first = OsString::from_vec(vec![0xff]);
    let second = OsString::from_vec(vec![0xfe]);
    let env: ShellEnv = [
        (first.clone(), Some("first".into())),
        (second.clone(), Some("second".into())),
        ("PATH".into(), Some("unicode".into())),
    ]
    .into();
    assert_eq!(
        command_env(&env, true),
        vec![
            (
                &OsString::from("PATH"),
                env[std::ffi::OsStr::new("PATH")].as_ref().unwrap()
            ),
            (&second, env[&second].as_ref().unwrap()),
            (&first, env[&first].as_ref().unwrap())
        ]
    );
}

#[test]
fn unset_environment_entries_remain_visible_until_native_spawn() {
    let env: ShellEnv = [
        (OsString::from("UNSET_BY_HOOK"), None),
        (OsString::from("SET_BY_HOOK"), Some(OsString::from("value"))),
        (OsString::from("PATH"), None),
        (OsString::from("Path"), Some(OsString::from("decoy"))),
    ]
    .into();
    assert_eq!(env.get(std::ffi::OsStr::new("UNSET_BY_HOOK")), Some(&None));
    assert_eq!(
        command_env(&env, false),
        vec![
            (&OsString::from("SET_BY_HOOK"), &OsString::from("value")),
            (&OsString::from("Path"), &OsString::from("decoy")),
        ]
    );
    assert_eq!(
        command_env(&env, true),
        vec![(&OsString::from("SET_BY_HOOK"), &OsString::from("value")),]
    );
    assert_eq!(env.len(), 4);
}
