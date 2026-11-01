//! Native raw shell execution through replaceable operations.
#![cfg(unix)]
#![cfg(test)]
use maestro_tools::{BashExecOptions, create_local_bash_operations};

#[test]
fn local_bash_preserves_command_directory_and_closed_input() {
    run(async {
        let operations = create_local_bash_operations("/tool-bin".into(), Some("/bin/bash".into()));
        let mut bytes = Vec::new();
        let mut sink = |data: &[u8]| bytes.extend_from_slice(data);
        let cwd = std::env::current_dir().unwrap();
        let result = operations
            .exec(
                "read ignored; printf '%s\\n' \"$?\"; pwd; exit 7",
                cwd.to_str().unwrap(),
                BashExecOptions {
                    on_data: &mut sink,
                    signal: None,
                    timeout: None,
                    env: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(result.exit_code, Some(7.0));
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            format!("1\n{}\n", cwd.display())
        );
    });
}

use crate::read_support;
use crate::runtime::run;
use maestro_models::Cancellation;
use maestro_tools::BashError;
use maestro_tools::utils::shell::{ShellEnv, get_shell_config};
use read_support::{Fixture, Scratch};
use std::{ffi::OsString, future::Future};

#[test]
fn shell_native_probes_validate_filesystem_boundaries() {
    let dir = Scratch::new().unwrap();
    assert_eq!(
        get_shell_config(Some(&dir.text())).unwrap().shell,
        dir.text()
    );
    let file = dir.path().join("not-executable");
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&file)
        .unwrap();
    assert_eq!(
        get_shell_config(Some(file.to_str().unwrap()))
            .unwrap()
            .shell,
        file.to_str().unwrap()
    );
    assert!(get_shell_config(Some(dir.path().join("missing").to_str().unwrap())).is_err());
    assert_eq!(get_shell_config(None).unwrap().shell, "/bin/bash");
}

#[test]
fn local_bash_forwards_factory_shell_path_each_run() {
    run(async {
        let dir = Scratch::new().unwrap();
        let path = dir.path().join("selected-shell");
        let operations =
            create_local_bash_operations("/tools".into(), Some(path.to_str().unwrap().into()));
        std::os::unix::fs::symlink("/bin/bash", &path).unwrap();
        let mut sink = |_: &[u8]| {};
        let first = operations
            .exec(
                "exit 7",
                &dir.text(),
                BashExecOptions {
                    on_data: &mut sink,
                    signal: None,
                    timeout: None,
                    env: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(first.exit_code, Some(7.0));
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink("/usr/bin/false", &path).unwrap();
        let second = operations
            .exec(
                "exit 7",
                &dir.text(),
                BashExecOptions {
                    on_data: &mut sink,
                    signal: None,
                    timeout: None,
                    env: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(second.exit_code, Some(1.0));
        std::fs::remove_file(&path).unwrap();
        assert!(
            operations
                .exec(
                    "",
                    &dir.text(),
                    BashExecOptions {
                        on_data: &mut sink,
                        signal: None,
                        timeout: None,
                        env: None
                    }
                )
                .await
                .is_err()
        );
    });
}

/// Checks one constructor bin directory through three environment routes.
async fn environment_routes(bin: &str, cwd: &str, shell: &str) {
    let operations = create_local_bash_operations(bin.into(), Some(shell.into()));
    for mode in 0..3 {
        let mut env = ShellEnv::new();
        if mode == 1 {
            env.insert(
                OsString::from("TEST_LOCAL_BASH_OPS"),
                Some(OsString::from("replacement")),
            );
        }
        let mut bytes = Vec::new();
        let mut sink = |data: &[u8]| bytes.extend_from_slice(data);
        let command = [
            "/usr/bin/env",
            "printf '%s' \"$TEST_LOCAL_BASH_OPS\"",
            "printf '%s' \"$PATH\"",
        ][mode];
        let result = operations
            .exec(
                command,
                cwd,
                BashExecOptions {
                    on_data: &mut sink,
                    signal: None,
                    timeout: None,
                    env: (mode != 2).then_some(&env),
                },
            )
            .await
            .unwrap();
        assert_eq!(result.exit_code, Some(0.0));
        let text = String::from_utf8(bytes).unwrap();
        match mode {
            0 => {
                assert!(!text.lines().any(|line| line.starts_with("PATH=")));
                assert!(!text.contains(bin));
            }
            1 => assert_eq!(text, "replacement"),
            _ => assert!(text.starts_with(&format!("{bin}:"))),
        }
    }
}

#[test]
fn local_bash_replaces_environment_or_builds_fresh_default() {
    run(async {
        let dir = Scratch::new().unwrap();
        for bin in ["/first-tool-bin", "/second-tool-bin"] {
            environment_routes(bin, &dir.text(), "/bin/bash").await;
        }
        for shell in ["/bin/bash", "/bin/sh"] {
            environment_routes("/first-tool-bin", &dir.text(), shell).await;
        }
    });
}

#[test]
fn local_bash_preserves_exit_and_native_failure_precedence() {
    run(async {
        let dir = Scratch::new().unwrap();
        let operations = create_local_bash_operations("/tools".into(), Some("/bin/bash".into()));
        let mut sink = |_: &[u8]| {};
        for (command, wanted) in [
            ("", Some(0.0)),
            ("exit 7", Some(7.0)),
            ("kill -KILL $$", None),
        ] {
            let result = operations
                .exec(
                    command,
                    &dir.text(),
                    BashExecOptions {
                        on_data: &mut sink,
                        signal: None,
                        timeout: None,
                        env: None,
                    },
                )
                .await
                .unwrap();
            assert_eq!(result.exit_code, wanted);
        }
        let file = dir.path().join("ordinary-file");
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file)
            .unwrap();
        let error = operations
            .exec(
                "",
                file.to_str().unwrap(),
                BashExecOptions {
                    on_data: &mut sink,
                    signal: None,
                    timeout: None,
                    env: None,
                },
            )
            .await
            .unwrap_err();
        assert!(
            std::error::Error::source(&error)
                .unwrap()
                .downcast_ref::<std::io::Error>()
                .is_some()
        );
    });
}

#[test]
fn local_bash_spawns_before_honoring_preabort() {
    run(async {
        let dir = Scratch::new().unwrap();
        let signal = Cancellation::new();
        signal.abort();
        let mut sink = |_: &[u8]| {};
        let invalid = create_local_bash_operations("/tools".into(), Some(dir.text()));
        let error = invalid
            .exec(
                "",
                &dir.text(),
                BashExecOptions {
                    on_data: &mut sink,
                    signal: Some(&signal),
                    timeout: Some(0.025),
                    env: None,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, BashError::Other(_)), "spawn error must win");
        let valid = create_local_bash_operations("/tools".into(), Some("/bin/bash".into()));
        let error = valid
            .exec(
                "exec /bin/sleep 60",
                &dir.text(),
                BashExecOptions {
                    on_data: &mut sink,
                    signal: Some(&signal),
                    timeout: None,
                    env: None,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, BashError::Aborted));
    });
}

#[test]
fn local_bash_timeout_is_optional_seconds() {
    run(async {
        let dir = Scratch::new().unwrap();
        let operations = create_local_bash_operations("/tools".into(), Some("/bin/bash".into()));
        let mut sink = |_: &[u8]| {};
        for timeout in [
            None,
            Some(0.0),
            Some(-1.0),
            Some(f64::NAN),
            Some(f64::INFINITY),
            Some(1e300),
            Some(1e12),
        ] {
            let result = operations
                .exec(
                    "exit 7",
                    &dir.text(),
                    BashExecOptions {
                        on_data: &mut sink,
                        signal: None,
                        timeout,
                        env: None,
                    },
                )
                .await
                .unwrap();
            assert_eq!(result.exit_code, Some(7.0));
        }
    });
}

#[test]
fn local_bash_errors_keep_exact_authored_messages() {
    run(async {
        let dir = Scratch::new().unwrap();
        let missing = dir.path().join("missing").to_str().unwrap().to_owned();
        let mut sink = |_: &[u8]| {};
        let options = create_local_bash_operations("/tools".into(), Some(missing.clone()));
        let error = options
            .exec(
                "",
                "",
                BashExecOptions {
                    on_data: &mut sink,
                    signal: None,
                    timeout: None,
                    env: None,
                },
            )
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("Custom shell path not found: {missing}")
        );
        let options = create_local_bash_operations("/tools".into(), Some("/bin/bash".into()));
        for cwd in ["", missing.as_str()] {
            let error = options
                .exec(
                    "",
                    cwd,
                    BashExecOptions {
                        on_data: &mut sink,
                        signal: None,
                        timeout: None,
                        env: None,
                    },
                )
                .await
                .unwrap_err();
            assert_eq!(
                error.to_string(),
                format!("Working directory does not exist: {cwd}\nCannot execute bash commands.")
            );
        }
        assert_eq!(BashError::Aborted.to_string(), "aborted");
        assert_eq!(BashError::Timeout(0.025).to_string(), "timeout:0.025");
        assert_eq!(BashError::Timeout(1e-7).to_string(), "timeout:0.0000001");
    });
}

#[test]
fn local_bash_has_its_own_session_and_process_group() {
    run(async {
        let operations = create_local_bash_operations("/tools".into(), Some("/bin/bash".into()));
        let mut bytes = Vec::new();
        let mut sink = |data: &[u8]| bytes.extend_from_slice(data);
        let result = operations
            .exec(
                // Not every `ps` names the session column, so the interpreter reads both ids.
                "printf '%s\\n' \"$$\"; python3 -c 'import os; print(os.getsid(0)); print(os.getpgrp())'; if ( : </dev/tty ) 2>/dev/null; then exit 9; fi",
                "/",
                BashExecOptions {
                    on_data: &mut sink,
                    signal: None,
                    timeout: None,
                    env: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(result.exit_code, Some(0.0));
        let text = String::from_utf8(bytes).unwrap();
        let ids: Vec<u32> = text
            .split_whitespace()
            .map(|value| value.parse().unwrap())
            .collect();
        assert_eq!(ids.len(), 3);
        assert_eq!(ids[0], ids[1], "child must lead a new session");
        assert_eq!(ids[0], ids[2], "child must lead its process group");
        assert_ne!(ids[0], std::process::id());
    });
}

#[test]
fn local_bash_streams_raw_bytes_before_completion() {
    run(async {
        let dir = Scratch::new().unwrap();
        let gate = dir.path().join("release");
        let operations = create_local_bash_operations("/tools".into(), Some("/bin/bash".into()));
        let mut bytes = Vec::new();
        let mut released = false;
        let mut sink = |data: &[u8]| {
            bytes.extend_from_slice(data);
            if !released {
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&gate)
                    .unwrap();
                released = true;
            }
        };
        let command = format!(
            "printf 'raw\\000\\033[31m\\r\\n\\377'; while [ ! -f '{}' ]; do :; done; printf 'stderr' >&2",
            gate.display()
        );
        let result = operations
            .exec(
                &command,
                &dir.text(),
                BashExecOptions {
                    on_data: &mut sink,
                    signal: None,
                    timeout: None,
                    env: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(result.exit_code, Some(0.0));
        assert_eq!(bytes, b"raw\0\x1b[31m\r\n\xffstderr");
    });
}

/// A replacement operation that observes caller inputs and does not narrow status.
struct Supplied;
impl maestro_tools::BashOperations for Supplied {
    fn exec<'a>(
        &'a self,
        command: &'a str,
        cwd: &'a str,
        options: BashExecOptions<'a>,
    ) -> std::pin::Pin<
        Box<dyn Future<Output = Result<maestro_tools::BashExecResult, BashError>> + Send + 'a>,
    > {
        Box::pin(async move {
            assert_eq!(cwd, "/selected");
            assert!(options.signal.unwrap().is_aborted());
            assert_eq!(
                options.env.unwrap().get(&OsString::from("MARK")),
                Some(&Some(OsString::from("supplied")))
            );
            (options.on_data)(b"custom-bytes");
            Ok(maestro_tools::BashExecResult {
                exit_code: if command == "fractional" {
                    Some(0.5)
                } else {
                    None
                },
            })
        })
    }
}

#[test]
fn supplied_bash_operations_keep_open_result_values() {
    run(async {
        let operations: std::sync::Arc<
            <maestro_models::Threaded as maestro_tools::BashExecution>::Operations,
        > = std::sync::Arc::new(Supplied);
        let signal = Cancellation::new();
        signal.abort();
        let env = [(OsString::from("MARK"), Some(OsString::from("supplied")))]
            .into_iter()
            .collect();
        for (command, wanted) in [("fractional", Some(0.5)), ("absent", None)] {
            let mut bytes = Vec::new();
            let mut sink = |data: &[u8]| bytes.extend_from_slice(data);
            let result = operations
                .exec(
                    command,
                    "/selected",
                    BashExecOptions {
                        on_data: &mut sink,
                        signal: Some(&signal),
                        timeout: Some(2.0),
                        env: Some(&env),
                    },
                )
                .await
                .unwrap();
            assert_eq!(result.exit_code, wanted);
            assert_eq!(bytes, b"custom-bytes");
        }
    });
}

#[test]
fn native_shell_environment_reads_each_process_inheritance() {
    use maestro_tools::utils::shell::get_shell_env;
    for (mode, value) in [
        ("absent", None),
        ("empty", Some("")),
        ("changed", Some("/inherited")),
    ] {
        if read_support::isolated(
            "native_shell_environment_reads_each_process_inheritance",
            mode,
            &[("PATH", value)],
        ) {
            let first = get_shell_env("/first");
            let second = get_shell_env("/second");
            let suffix = value
                .filter(|value| !value.is_empty())
                .map_or(String::new(), |value| format!(":{value}"));
            assert_eq!(
                first.get(&OsString::from("PATH")),
                Some(&Some(OsString::from(format!("/first{suffix}"))))
            );
            assert_eq!(
                second.get(&OsString::from("PATH")),
                Some(&Some(OsString::from(format!("/second{suffix}"))))
            );
            assert_eq!(std::env::var_os("PATH"), value.map(OsString::from));
        }
    }
}

#[test]
fn local_bash_keeps_interleaved_split_unicode_and_large_output_raw() {
    run(async {
        let dir = Scratch::new().unwrap();
        let gates = [dir.path().join("first"), dir.path().join("second")];
        let mut bytes = Vec::new();
        let mut released = 0;
        let mut sink = |data: &[u8]| {
            bytes.extend_from_slice(data);
            if released < 2 && bytes.len() >= [2, 5][released] {
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&gates[released])
                    .unwrap();
                released += 1;
            }
        };
        let command = format!(
            "printf '\\360\\237'; while [ ! -f '{}' ]; do :; done; printf err >&2; while [ ! -f '{}' ]; do :; done; printf '\\230\\200'; /usr/bin/head -c 262144 /dev/zero",
            gates[0].display(),
            gates[1].display()
        );
        let operations = create_local_bash_operations("/tools".into(), Some("/bin/bash".into()));
        let result = operations
            .exec(
                &command,
                &dir.text(),
                BashExecOptions {
                    on_data: &mut sink,
                    signal: None,
                    timeout: None,
                    env: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(result.exit_code, Some(0.0));
        assert_eq!(&bytes[..7], b"\xf0\x9ferr\x98\x80");
        assert_eq!(bytes.len(), 262_151);
        assert!(bytes[7..].iter().all(|byte| *byte == 0));
        assert_eq!(released, 2);
    });
}
