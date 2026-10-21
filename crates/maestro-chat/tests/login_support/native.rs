//! Isolated native browser helpers, never a live desktop service.
use maestro_chat::LoginDialogPlatform;
use maestro_tui::Component;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    process::{Command, Stdio},
};

/// Controlled helper records argv before waiting for explicit release.
const HELPER: &str = r"#!/usr/bin/python3
import json, os, socket, sys
with socket.create_connection(('127.0.0.1', int(os.environ['MAESTRO_BROWSER_PORT']))) as stream:
    stream.sendall((json.dumps([os.path.basename(sys.argv[0]), sys.argv[1:]])+'\n').encode())
    assert stream.recv(1) == b'!'
    stream.sendall(b'done')
sys.exit(1)
";
/// URL with shell metacharacters must remain one literal operand.
const URL: &str = "https://auth.example.test/a b?x=%PATH%&y=$(never-run);\"!VALUE!";
/// Child-side default launch, returning while its helper remains blocked.
fn child(mode: &str) {
    let platform = match mode {
        "open" => LoginDialogPlatform::MacOs,
        "explorer.exe" => LoginDialogPlatform::Windows,
        _ => LoginDialogPlatform::Other,
    };
    let (dialog, renders) = super::controlled(platform, None);
    dialog.show_auth(URL, None).unwrap();
    assert_eq!(renders.get(), 1);
    assert!(
        dialog
            .render(80)
            .iter()
            .any(|row| row.contains("click to open"))
    );
    println!("browser-launch-returned");
    if mode != "missing" {
        let mut released = [0];
        std::io::stdin().read_exact(&mut released).unwrap();
        assert_eq!(released, [b'!']);
    }
}
/// Actual default opener with argv capture, release handshake and silent startup failure.
pub fn launch() {
    if maestro_test_process::selection().is_child() {
        let mode = ["open", "explorer.exe", "xdg-open", "missing"]
            .into_iter()
            .find(|mode| maestro_test_process::selection().runs_case(mode))
            .expect("selected native mode");
        child(mode);
        return;
    }
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    std::fs::write(scratch.path().join("helper-source"), HELPER).unwrap();
    for program in ["open", "explorer.exe", "xdg-open"] {
        assert!(
            Command::new("install")
                .args(["-m", "0700"])
                .arg(scratch.path().join("helper-source"))
                .arg(scratch.path().join(program))
                .status()
                .unwrap()
                .success()
        );
    }
    for program in ["open", "explorer.exe", "xdg-open", "missing"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let command = maestro_test_process::TestProcess::current(
            "native_browser_launch_is_detached_and_uses_one_literal_argument",
        )
        .unwrap()
        .nocapture()
        .case(program)
        .env(
            "MAESTRO_BROWSER_PORT",
            listener.local_addr().unwrap().port().to_string(),
        )
        .env(
            "PATH",
            scratch
                .path()
                .join(if program == "missing" { "custom" } else { "" }),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
        let mut child = command.spawn().unwrap();
        if program != "missing" {
            handshake(&listener, &mut child, program);
        }
        assert!(child.wait().unwrap().success(), "isolated browser witness");
    }
}

/// Observe return before releasing the browser process, then consume all output.
fn handshake(
    listener: &TcpListener,
    child: &mut maestro_test_process::CheckedChild,
    program: &str,
) {
    let (stream, _) = listener.accept().unwrap();
    let mut stream = BufReader::new(stream);
    let mut record = String::new();
    stream.read_line(&mut record).unwrap();
    let (identity, args): (String, Vec<String>) = serde_json::from_str(&record).unwrap();
    assert_eq!(identity, program);
    assert_eq!(args, [URL]);
    let mut output = BufReader::new(child.stdout.take().unwrap());
    loop {
        let mut line = String::new();
        assert!(output.read_line(&mut line).unwrap() > 0);
        if line.contains("browser-launch-returned") {
            break;
        }
    }
    stream.get_mut().write_all(b"!").unwrap();
    let mut done = String::new();
    stream.read_to_string(&mut done).unwrap();
    assert_eq!(done, "done");
    child.stdin.take().unwrap().write_all(b"!").unwrap();
    let mut remaining = String::new();
    output.read_to_string(&mut remaining).unwrap();
}
