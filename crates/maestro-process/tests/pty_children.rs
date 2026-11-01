//! Controlled terminal children through the checked libtest setup entry.
#![cfg(test)]
#![cfg(target_os = "linux")]
use crate::child_programs;
use maestro_process::pty::{self, PtyCommand, PtyEnvironment, PtySize};
use maestro_test_process::{TestProcess, run_alone};
use std::{
    ffi::OsString,
    io::{self, Read, Write},
    process::{Command, Output},
};

/// Default terminal size with unequal pixel dimensions.
fn size() -> PtySize {
    PtySize {
        rows: 24,
        cols: 80,
        cell_width_px: 8,
        cell_height_px: 16,
    }
}

/// Carries native launch policy through one checked setup entry.
fn wrap(mut command: PtyCommand, harness: &[String]) -> io::Result<PtyCommand> {
    let signals = command.args.iter().any(|arg| arg == "case:signals");
    command = pty::PtySetupEntry {
        executable: std::env::current_exe()?,
        args: harness.iter().map(OsString::from).collect(),
    }
    .prepare(command);
    if signals {
        let launcher = child_programs::command(child_programs::blocked_signals_launcher, &[]);
        command.program = launcher.get_program().to_owned();
        command.args = launcher.get_args().map(OsString::from).collect();
        if let PtyEnvironment::Inherit(edits) = &mut command.environment {
            let arguments = harness.join("\u{1f}");
            edits.push(("CHILD_EXEC_ARGS".into(), Some(arguments.into())));
        }
    }
    Ok(command)
}

#[test]
fn setup_entry() {
    if std::env::var_os("MAESTRO_PTY_DUP_FAILURE").is_some() {
        let original = rustix::process::getrlimit(rustix::process::Resource::Nofile);
        let mut limit = original;
        limit.current = Some(3);
        rustix::process::setrlimit(rustix::process::Resource::Nofile, limit).unwrap();
        let error = pty::setup_entry().unwrap();
        rustix::process::setrlimit(rustix::process::Resource::Nofile, original).unwrap();
        assert_eq!(error.raw_os_error(), Some(22));
    } else if pty::setup_entry().is_none() {
        return;
    }
    std::process::exit(1);
}

/// Counts parent descriptors inside an isolated test transaction.
fn descriptor_count() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}

/// Retains master bytes through the Linux terminal end-of-file condition.
fn drain(master: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut raw = Vec::new();
    match master.read_to_end(&mut raw) {
        Ok(_) => {}
        Err(error) if error.raw_os_error() == Some(5) => {}
        Err(error) => return Err(error),
    }
    Ok(raw)
}

/// Checks the exact ONLCR mapping before yielding the shared start proof.
fn normalized(raw: &[u8]) -> Vec<u8> {
    let lf = String::from_utf8(raw.to_vec())
        .unwrap()
        .replace("\r\n", "\n")
        .into_bytes();
    let reconstructed: Vec<_> = lf
        .iter()
        .flat_map(|byte| {
            if *byte == b'\n' {
                vec![b'\r', b'\n']
            } else {
                vec![*byte]
            }
        })
        .collect();
    assert_eq!(raw, reconstructed);
    lf
}

#[test]
fn pty_child_keeps_job_control_and_one_parent_master() {
    if run_alone("pty_child_keeps_job_control_and_one_parent_master").unwrap() {
        return;
    }
    let before = descriptor_count();
    let output = TestProcess::named("pty_children::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .output_with(|harness| {
            let command = PtyCommand {
                program: "/bin/sh".into(),
                argv0: Some("-sh".into()),
                args: vec![
                    "-ic".into(),
                    r#"printf 'ARG0 %s\n' "$0"; printf 'SHELL %s\n' "$$"; read -r token; printf 'GOT %s\n' "$token""#.into(),
                ],
                cwd: None,
                environment: PtyEnvironment::Inherit(vec![]),
            };
            let (pty, mut child) = pty::spawn(size(), command, |command| wrap(command, harness))?;
            assert_eq!(descriptor_count(), before + 1);
            assert!(
                rustix::io::fcntl_getfd(&pty.master_fd)?.contains(rustix::io::FdFlags::CLOEXEC)
            );
            let mut master = std::fs::File::from(pty.master_fd);
            let mut raw = Vec::new();
            let marker = format!("SHELL {}\r\n", child.id());
            while !raw.ends_with(marker.as_bytes()) {
                let mut byte = [0];
                master.read_exact(&mut byte)?;
                raw.push(byte[0]);
            }
            assert_native_session(&master, child.id())?;
            master.write_all(b"literal input\n")?;
            raw.extend(drain(&mut master)?);
            let status = child.wait()?;
            assert!(status.success());
            assert!(
                raw.windows(b"GOT literal input\r\n".len())
                    .any(|part| part == b"GOT literal input\r\n")
            );
            assert!(!String::from_utf8_lossy(&raw).contains("job control"));
            assert!(
                raw.windows(b"ARG0 -sh\r\n".len()).any(|part| part == b"ARG0 -sh\r\n"),
                "{}", String::from_utf8_lossy(&raw)
            );
            Ok::<_, io::Error>(Output {
                status,
                stdout: normalized(&raw),
                stderr: vec![],
            })
        })
        .unwrap();
    assert!(output.status.success());
    assert_eq!(descriptor_count(), before);
}

/// Invokes production spawn through the checked setup entry, retaining raw terminal proof.
fn checked_target(
    command: PtyCommand,
    size: PtySize,
    inspect: impl FnOnce(&pty::Pty) -> io::Result<()>,
) -> Output {
    TestProcess::named("pty_children::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .output_with(|harness| {
            let (pty, mut child) = pty::spawn(size, command, |command| wrap(command, harness))?;
            inspect(&pty)?;
            let raw = drain(&mut std::fs::File::from(pty.master_fd))?;
            let status = child.wait()?;
            Ok::<_, io::Error>(Output {
                status,
                stdout: normalized(&raw),
                stderr: vec![],
            })
        })
        .unwrap()
}

#[test]
fn target_facts() {
    let Some(mode) = std::env::args().find_map(|arg| arg.strip_prefix("case:").map(str::to_owned))
    else {
        return;
    };
    let mode = mode.as_str();
    assert_eq!(std::env::current_dir().unwrap(), std::path::Path::new("/"));
    assert!(std::env::args_os().any(|arg| arg == "decoy one ' quote"));
    assert!(
        std::env::vars_os()
            .all(|(key, _)| !key.as_encoded_bytes().starts_with(b"MAESTRO_PTY_ARG_"))
    );
    assert_environment(mode);
    let pid = rustix::process::getpid();
    assert_eq!(rustix::process::getsid(None).unwrap(), pid);
    assert_eq!(rustix::termios::tcgetpgrp(io::stdin()).unwrap(), pid);
    assert!(rustix::termios::isatty(io::stdin()));
    assert!(rustix::termios::isatty(io::stdout()));
    assert!(rustix::termios::isatty(io::stderr()));
    println!("FACTS READY");
}

#[test]
fn pty_setup_preserves_native_failures_and_launch_policy() {
    if run_alone("pty_setup_preserves_native_failures_and_launch_policy").unwrap() {
        return;
    }
    assert_launch_policies();
    TestProcess::named("pty_children::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .output_with(assert_failure_text)
        .unwrap();
}

#[test]
fn pty_resize_saturates_pixel_dimensions() {
    if run_alone("pty_resize_saturates_pixel_dimensions").unwrap() {
        return;
    }
    let output = checked_target(
        maestro_process::shell::pane_custom_command_pty_builder("exit 0"),
        size(),
        |pty| {
            let initial = rustix::termios::tcgetwinsize(&pty.master_fd)?;
            assert_eq!(
                (
                    initial.ws_row,
                    initial.ws_col,
                    initial.ws_xpixel,
                    initial.ws_ypixel
                ),
                (24, 80, 0, 0)
            );
            for (rows, cols, width, height, x, y) in [
                (0, 0, 0, 0, 0, 0),
                (1, 1, 1, 1, 1, 1),
                (65535, 65535, u32::MAX, u32::MAX, 65535, 65535),
                (3, 7, 11, 13, 77, 39),
                (1, 0, u32::MAX, 1, 0, 1),
            ] {
                pty.resize(PtySize {
                    rows,
                    cols,
                    cell_width_px: width,
                    cell_height_px: height,
                })?;
                let observed = rustix::termios::tcgetwinsize(&pty.master_fd)?;
                assert_eq!(
                    (
                        observed.ws_row,
                        observed.ws_col,
                        observed.ws_xpixel,
                        observed.ws_ypixel
                    ),
                    (rows, cols, x, y)
                );
            }
            Ok(())
        },
    );
    assert!(output.status.success());
}

/// Drives inherited edits, empty replacement, native operands and signal reset.
fn assert_launch_policies() {
    for mode in ["inherit", "edited", "clear", "replace", "signals"] {
        let mut edits = vec![];
        let environment = match mode {
            "inherit" => {
                if let Some(path) = std::env::var_os("PATH") {
                    edits.push(("MAESTRO_PTY_EXPECTED_PATH".into(), Some(path)));
                } else {
                    edits.push(("MAESTRO_PTY_EXPECTED_PATH".into(), None));
                }
                PtyEnvironment::Inherit(edits)
            }
            "edited" => {
                edits.extend([
                    ("PATH".into(), None),
                    ("MAESTRO_PTY_VALUE".into(), Some("supplied".into())),
                ]);
                PtyEnvironment::Inherit(edits)
            }
            "clear" => PtyEnvironment::Replace(vec![]),
            "replace" => {
                PtyEnvironment::Replace(vec![("MAESTRO_PTY_VALUE".into(), "supplied".into())])
            }
            _ => PtyEnvironment::Inherit(edits),
        };
        let before = descriptor_count();
        let output = TestProcess::named("pty_children::target_facts")
            .unwrap()
            .nocapture()
            .single_threaded()
            .skip("decoy one ' quote")
            .skip(format!("case:{mode}"))
            .output_with(|harness| {
                let command = PtyCommand {
                    program: std::env::current_exe()?.into_os_string(),
                    argv0: None,
                    args: harness.iter().map(OsString::from).collect(),
                    cwd: Some("/".into()),
                    environment,
                };
                Ok::<_, io::Error>(checked_target(command, size(), |_| Ok(())))
            })
            .unwrap();
        assert!(output.status.success(), "{mode}: {output:?}");
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains("FACTS READY")
        );
        assert_eq!(descriptor_count(), before);
    }
}

/// Checks original failure text and descriptor cleanup before a checked success control.
fn assert_failure_text(harness: &[String]) -> io::Result<Output> {
    for missing_directory in [false, true] {
        let before = descriptor_count();
        let mut native = Command::new(if missing_directory {
            "/bin/true"
        } else {
            "/no/such/maestro-pty-target"
        });
        let mut command = PtyCommand {
            program: native.get_program().to_owned(),
            argv0: None,
            args: vec![],
            cwd: None,
            environment: PtyEnvironment::Inherit(vec![]),
        };
        if missing_directory {
            native.current_dir("/no/such/maestro-directory");
            command.cwd = Some("/no/such/maestro-directory".into());
        }
        let expected = native.spawn().unwrap_err().to_string();
        let error = pty::spawn(size(), command, |command| wrap(command, harness)).unwrap_err();
        assert_eq!(error.to_string(), expected);
        assert_eq!(descriptor_count(), before);
    }
    let before = descriptor_count();
    let mut command = maestro_process::shell::pane_custom_command_pty_builder("exit 0");
    command.environment =
        PtyEnvironment::Inherit(vec![("MAESTRO_PTY_DUP_FAILURE".into(), Some("1".into()))]);
    let error = pty::spawn(size(), command, |command| wrap(command, harness)).unwrap_err();
    assert_eq!(
        error.to_string(),
        io::Error::from_raw_os_error(22).to_string()
    );
    assert_eq!(descriptor_count(), before);
    let command = maestro_process::shell::pane_custom_command_pty_builder(
        "printf 'checked failure completion\\n'",
    );
    let (pty, mut child) = pty::spawn(size(), command, |command| wrap(command, harness))?;
    let raw = drain(&mut std::fs::File::from(pty.master_fd))?;
    let status = child.wait()?;
    Ok::<_, io::Error>(Output {
        status,
        stdout: normalized(&raw),
        stderr: vec![],
    })
}

/// Checks the environment actually observed after target exec.
fn assert_environment(mode: &str) {
    match mode {
        "inherit" => assert_eq!(
            std::env::var_os("PATH"),
            std::env::var_os("MAESTRO_PTY_EXPECTED_PATH")
        ),
        "clear" => {
            assert!(std::env::var_os("PATH").is_none());
            assert!(std::env::var_os("MAESTRO_PTY_VALUE").is_none());
        }
        "replace" | "edited" => {
            assert!(std::env::var_os("PATH").is_none());
            assert_eq!(std::env::var("MAESTRO_PTY_VALUE").unwrap(), "supplied");
        }
        "signals" => {
            let mask = [1, 2, 3, 14, 15, 17]
                .into_iter()
                .fold(0u64, |mask, number| mask | (1 << (number - 1)));
            // The harness leader can temporarily block signals while creating a thread.
            let text = std::fs::read_to_string("/proc/thread-self/status").unwrap();
            let value = text
                .lines()
                .find_map(|line| line.strip_prefix("SigBlk:"))
                .unwrap()
                .trim();
            assert_eq!(
                u64::from_str_radix(value, 16).unwrap() & mask,
                0,
                "SigBlk signal state survived setup"
            );
        }
        _ => panic!("unknown launch case"),
    }
}

/// Observes native foreground, session and process-group identity after shell startup.
fn assert_native_session(master: &std::fs::File, pid: u32) -> io::Result<()> {
    assert_eq!(
        rustix::termios::tcgetpgrp(master)?
            .as_raw_nonzero()
            .get()
            .unsigned_abs(),
        pid
    );
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let fields: Vec<_> = stat[stat.rfind(')').unwrap() + 2..]
        .split_whitespace()
        .collect();
    for index in [2, 3, 5] {
        assert_eq!(fields[index].parse::<u32>().unwrap(), pid);
    }

    Ok(())
}
