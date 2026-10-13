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
            .map(|(key, value)| (OsString::from(key), OsString::from(value)))
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
        assert_eq!(command_env(&env, false), env.iter().collect::<Vec<_>>());
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
        (first.clone(), "first".into()),
        (second.clone(), "second".into()),
        ("PATH".into(), "unicode".into()),
    ]
    .into();
    assert_eq!(
        command_env(&env, true),
        vec![
            env.get_key_value(std::ffi::OsStr::new("PATH")).unwrap(),
            (&second, &env[&second]),
            (&first, &env[&first])
        ]
    );
}
