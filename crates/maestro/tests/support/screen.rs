//! Native output observed through an actual pseudo-terminal.
#![cfg(target_os = "linux")]
use super::Root;
use maestro_process::pty::{PtyCommand, PtyEnvironment, PtySetupEntry, PtySize};
use std::io::{Read, Write};

/// Reap the real executable on assertion failure as well as successful completion.
struct Process(std::process::Child);
impl Drop for Process {
    fn drop(&mut self) {
        if self.0.try_wait().unwrap().is_none() {
            self.0.kill().unwrap();
            self.0.wait().unwrap();
        }
    }
}

/// Start the supplied root route on the terminal, optionally selecting benchmark mode.
fn terminal(root: &Root, args: &[&str], benchmark: Option<&str>) -> (std::fs::File, Process) {
    let command = root.command(&[]);
    let mut environment: Vec<_> = command
        .get_envs()
        .filter_map(|(key, value)| value.map(|value| (key.to_owned(), value.to_owned())))
        .collect();
    environment.push(("TERM".into(), "xterm-256color".into()));
    environment.extend(benchmark.map(|value| ("MAESTRO_STARTUP_BENCHMARK".into(), value.into())));
    let binary = std::path::PathBuf::from(env!("CARGO_BIN_EXE_maestro"));
    let (pty, child) = PtySetupEntry {
        executable: binary.clone(),
        args: Vec::new(),
    }
    .spawn(
        PtySize {
            rows: 60,
            cols: 160,
            cell_width_px: 8,
            cell_height_px: 16,
        },
        PtyCommand {
            program: binary.into_os_string(),
            argv0: None,
            args: std::iter::once("--offline")
                .chain(args.iter().copied())
                .map(Into::into)
                .collect(),
            cwd: Some(root.path("project")),
            environment: PtyEnvironment::Replace(environment),
        },
    )
    .unwrap();
    (pty.master_fd.into(), Process(child))
}

/// Drain through the terminal's native EOF condition, then require a successful exit.
fn finish(mut master: std::fs::File, mut process: Process, mut bytes: Vec<u8>) -> String {
    match master.read_to_end(&mut bytes) {
        Ok(_) => {}
        Err(error) if error.raw_os_error() == Some(5) => {}
        Err(error) => panic!("{error}"),
    }
    let output = String::from_utf8(bytes).unwrap();
    assert!(process.0.wait().unwrap().success(), "{output}");
    output
}

/// Run real interactive initialization and benchmark teardown, retaining all terminal bytes.
pub fn benchmark(root: &Root, args: &[&str]) -> String {
    let (master, process) = terminal(root, args, Some("1"));
    finish(master, process, Vec::new())
}

/// Wait for the rendered resource, then use the editor's immediate quit route.
pub fn resource(root: &Root, args: &[&str], marker: &str) -> String {
    let (mut master, process) = terminal(root, args, None);
    let mut bytes = Vec::new();
    while !bytes.ends_with(marker.as_bytes()) {
        let mut byte = [0];
        master.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
    }
    master.write_all(b"/quit\r").unwrap();
    master.flush().unwrap();
    finish(master, process, bytes)
}

/// Waits for real startup, submits one shell command and quits after its completed row.
pub fn shell(root: &Root, args: &[&str], command: &str) -> String {
    let (mut master, process) = terminal(root, args, None);
    let mut bytes = Vec::new();
    for (marker, input) in [
        ("LOCAL-SHELL-READY", format!("{command}\r")),
        ("(exit 7)", "/quit\r".into()),
    ] {
        while !bytes.ends_with(marker.as_bytes()) {
            let mut byte = [0];
            master.read_exact(&mut byte).unwrap();
            bytes.push(byte[0]);
            assert!(
                !bytes.ends_with(b"Bash command failed:"),
                "the returned operations were not consumed: {}",
                String::from_utf8_lossy(&bytes)
            );
        }
        master.write_all(input.as_bytes()).unwrap();
        master.flush().unwrap();
    }
    finish(master, process, bytes)
}
