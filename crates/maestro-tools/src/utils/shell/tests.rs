//! Recorded shell selection, environment and cleanup policies.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use super::native::{select_shell, shell_env};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeMap, ffi::OsString};

/// Strict outer corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Independently recorded cases.
    cases: Vec<Case>,
}

/// One typed operation's input and expectation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Case {
    /// Non-secret diagnostic label.
    pub(crate) id: String,
    /// Selecting operation.
    kind: String,
    /// Decoded by the owning consumer.
    pub(crate) input: Value,
    /// Decoded by the owning consumer.
    pub(crate) expected: Value,
}

/// Loads a nonempty operation group from the shared recorded corpus.
pub(crate) fn cases(kind: &str) -> Vec<Case> {
    let corpus: Corpus = serde_json::from_str(include_str!(
        "../../../tests/fixtures/local-shell-cases.json"
    ))
    .unwrap();
    let rows: Vec<_> = corpus
        .cases
        .into_iter()
        .filter(|row| row.kind == kind)
        .collect();
    assert!(!rows.is_empty());
    rows
}

/// Controlled discovery observations.
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
struct SelectionInput {
    /// Explicit path, including the empty spelling.
    custom: Option<String>,
    /// Recorded platform, omission means Unix.
    platform: String,
    /// Paths observed as present.
    exists: Vec<String>,
    /// `ProgramFiles` observations.
    env: BTreeMap<String, String>,
    /// Synchronous probe result.
    probe: Option<Probe>,
    /// Probe exception, swallowed by discovery.
    probe_throw: bool,
}

/// Probe status and raw decoded stdout.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Probe {
    /// Native status, possibly absent.
    status: Option<i32>,
    /// Recorded stdout.
    stdout: Option<String>,
}

/// Exact discovery observations and result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionExpected {
    /// Chosen executable and operands.
    value: Option<Config>,
    /// Authored failure text.
    error: Option<String>,
    /// Ordered filesystem and subprocess effects.
    trace: Vec<Value>,
}

/// Chosen shell record.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    /// Executable spelling.
    shell: String,
    /// Literal leading operands.
    args: Vec<String>,
}

/// Runs the recorded discovery rows belonging to a single branch family.
fn selection_rows(select: impl Fn(&str) -> bool) {
    for row in cases("shell_config")
        .into_iter()
        .filter(|row| select(&row.id))
    {
        let input: SelectionInput = serde_json::from_value(row.input).unwrap();
        let expected: SelectionExpected = serde_json::from_value(row.expected).unwrap();
        let trace = RefCell::new(Vec::new());
        let actual = select_shell(
            input.custom.as_deref(),
            input.platform == "win32",
            |key| input.env.get(key).cloned(),
            |path| {
                trace.borrow_mut().push(json!(["exists", path]));
                input.exists.iter().any(|entry| entry == path)
            },
            |program, operand, deadline| {
                trace
                    .borrow_mut()
                    .push(json!(["probe", program, [operand], {"timeout": deadline.as_millis()}]));
                if input.probe_throw {
                    return None;
                }
                input
                    .probe
                    .as_ref()
                    .filter(|probe| probe.status == Some(0))
                    .and_then(|probe| probe.stdout.clone())
            },
        );
        match (actual, expected.value, expected.error) {
            (Ok(actual), Some(value), None) => {
                assert_eq!(actual.shell, value.shell, "{}", row.id);
                assert_eq!(actual.args, value.args);
            }
            (Err(error), None, Some(text)) => assert_eq!(error.to_string(), text, "{}", row.id),
            _ => panic!("unexpected selection outcome"),
        }
        assert_eq!(trace.into_inner(), expected.trace, "{}", row.id);
    }
}

#[test]
fn shell_custom_path_is_checked_before_discovery() {
    selection_rows(|id| id.starts_with("custom_"));
    let selected = select_shell(
        Some("/chosen"),
        false,
        |_| panic!("environment must not be read"),
        |path| path == "/chosen",
        |_, _, _| panic!("probe must not run"),
    )
    .unwrap();
    assert_eq!(selected.shell, "/chosen");
}

#[test]
fn shell_unix_selection_uses_bin_then_probe_then_sh() {
    selection_rows(|id| id.starts_with("unix_"));
}

#[test]
fn shell_windows_selection_retains_first_probe_match() {
    selection_rows(|id| id.starts_with("windows_"));
}

/// Native inherited environment operands.
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct EnvInput {
    /// Insertion-ordered entries.
    env: indexmap::IndexMap<String, String>,
    /// Binary directory, omission uses the recorded default.
    bin: Option<String>,
    /// Delimiter policy.
    platform: String,
}

/// Expected environment entries.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvExpected {
    /// Complete entry sequence.
    value: indexmap::IndexMap<String, String>,
}

/// Converts fixture text to native environment identity.
fn native_env(entries: indexmap::IndexMap<String, String>) -> super::ShellEnv {
    entries
        .into_iter()
        .map(|(key, value)| (OsString::from(key), Some(OsString::from(value))))
        .collect()
}

#[test]
fn shell_environment_preserves_first_path_key_and_exact_entries() {
    for row in cases("shell_env") {
        let input: EnvInput = serde_json::from_value(row.input).unwrap();
        let expected: EnvExpected = serde_json::from_value(row.expected).unwrap();
        let actual = shell_env(
            native_env(input.env),
            input.bin.as_deref().unwrap_or("/app/bin"),
            input.platform == "win32",
        );
        assert_eq!(
            actual.into_iter().collect::<Vec<_>>(),
            native_env(expected.value).into_iter().collect::<Vec<_>>(),
            "{}",
            row.id
        );
    }
}

/// Two independently observed environment states.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvSequence {
    /// First inherited state.
    first: indexmap::IndexMap<String, String>,
    /// Later inherited state.
    second: indexmap::IndexMap<String, String>,
}

#[test]
fn shell_environment_is_observed_each_time() {
    for row in cases("shell_env_sequence") {
        let input: EnvSequence = serde_json::from_value(row.input).unwrap();
        let expected: EnvSequence = serde_json::from_value(row.expected).unwrap();
        for (env, wanted) in [
            (input.first, expected.first),
            (input.second, expected.second),
        ] {
            assert_eq!(
                shell_env(native_env(env), "/app/bin", false),
                native_env(wanted)
            );
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let raw = OsString::from_vec(vec![0xff]);
        let mut env = super::ShellEnv::new();
        env.insert(raw.clone(), Some(raw.clone()));
        env.insert(OsString::from("PATH"), Some(OsString::from_vec(vec![0xfe])));
        let actual = shell_env(env, "/app/bin", false);
        assert_eq!(actual.get(&raw), Some(&Some(raw)));
        assert_eq!(
            actual
                .get(&OsString::from("PATH"))
                .unwrap()
                .as_ref()
                .unwrap()
                .as_encoded_bytes(),
            b"/app/bin:\xfe"
        );
    }
}

/// Recorded ordered PID operations.
#[cfg(unix)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TrackingInput {
    /// Track/untrack/clear operands.
    sequence: Vec<Vec<Value>>,
    /// Native signal attempts that fail.
    #[serde(default)]
    fail_kills: Vec<i64>,
}

/// Ordered native signal observations.
#[cfg(unix)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TrackingExpected {
    /// Signal requests, including repeated attempts.
    trace: Vec<Value>,
}

#[test]
#[cfg(unix)]
fn tracked_pids_are_unique_ordered_and_drained() {
    for row in cases("tracking") {
        let input: TrackingInput = serde_json::from_value(row.input).unwrap();
        let expected: TrackingExpected = serde_json::from_value(row.expected).unwrap();
        let trace = RefCell::new(Vec::new());
        let kill = |pid: i64| {
            trace.borrow_mut().push(json!(["kill", pid, "SIGKILL"]));
            if input.fail_kills.contains(&pid) {
                Err(std::io::Error::other("controlled signal failure"))
            } else {
                Ok(())
            }
        };
        for operation in input.sequence {
            let pid = operation
                .get(1)
                .and_then(Value::as_u64)
                .map(|pid| u32::try_from(pid).unwrap());
            match operation[0].as_str().unwrap() {
                "track" => super::track_detached_child_pid(pid.unwrap()),
                "untrack" => super::untrack_detached_child_pid(pid.unwrap()),
                "kill" => super::native::tree_kill(
                    pid.unwrap(),
                    |pid| kill(-i64::from(pid)),
                    |pid| kill(i64::from(pid)),
                ),
                "clear" => super::native::cleanup(|pid| {
                    super::native::tree_kill(
                        pid,
                        |pid| kill(-i64::from(pid)),
                        |pid| kill(i64::from(pid)),
                    );
                }),
                _ => panic!("unknown operation"),
            }
        }
        assert_eq!(trace.into_inner(), expected.trace, "{}", row.id);
    }
}

#[test]
#[cfg(unix)]
fn tree_kill_falls_back_only_after_group_failure() {
    for fail in [false, true] {
        let calls = RefCell::new(Vec::new());
        super::native::tree_kill(
            123,
            |pid| {
                calls.borrow_mut().push(-i64::from(pid));
                if fail {
                    Err(std::io::Error::other("missing group"))
                } else {
                    Ok(())
                }
            },
            |pid| {
                calls.borrow_mut().push(i64::from(pid));
                Err(std::io::Error::other("missing child"))
            },
        );
        assert_eq!(
            calls.into_inner(),
            if fail { vec![-123, 123] } else { vec![-123] }
        );
    }
    assert!(maestro_process::child_process::kill_process_group(0).is_err());
    assert!(maestro_process::child_process::kill_process(u32::MAX).is_err());
}

#[test]
#[cfg(unix)]
fn shell_probe_uses_five_second_sync_capture() {
    assert_eq!(
        super::native::probe(
            "/usr/bin/printf",
            "native-probe",
            std::time::Duration::from_millis(5000)
        ),
        Some("native-probe".into())
    );
    assert_eq!(
        super::native::probe(
            "/bin/false",
            "ignored",
            std::time::Duration::from_millis(5000)
        ),
        None
    );
    let selected = select_shell(
        None,
        false,
        |_| panic!("Unix selection does not read roots"),
        |_| false,
        |program, operand, duration| {
            assert_eq!((program, operand), ("which", "bash"));
            assert_eq!(duration, std::time::Duration::from_millis(5000));
            Some("/controlled/bash".into())
        },
    )
    .unwrap();
    assert_eq!(selected.shell, "/controlled/bash");
}
