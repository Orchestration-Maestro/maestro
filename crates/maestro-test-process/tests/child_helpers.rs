//! The helpers that parent tests and re-run child programs share.
#![cfg(test)]
#![cfg(unix)]

use maestro_test_process::child::{
    announce, connect, harness, is_null, leave, port, read_input, receive, variable,
    without_harness, write_stderr, write_stdout,
};
use maestro_test_process::{HARNESS_START, TestProcess};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Stdio;

/// A child that uses every helper: it reads its input, reports over the socket the parent
/// opened, answers on both streams and ends with status 5.
#[test]
#[ignore = "runs only inside a controlled child"]
fn helper_probe() {
    let input = read_input().unwrap();
    let mut socket = connect(port("MTP_PORT").unwrap()).unwrap();
    announce(&mut socket, &variable("MTP_NAME").unwrap()).unwrap();
    assert_eq!(receive(&mut socket).unwrap(), Some(b'!'));
    let kind = if is_null("/dev/stdin").unwrap() {
        "null"
    } else {
        "pipe"
    };
    write_stdout(format!("{kind}:{}", input.len()).as_bytes()).unwrap();
    write_stderr(b"diag").unwrap();
    leave(5)
}

#[test]
fn child_helpers_serve_a_child_and_its_parent() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let mut child = TestProcess::child(helper_probe)
        .unwrap()
        .env("MTP_PORT", &port)
        .env("MTP_NAME", "helper")
        .command()
        .unwrap();
    child
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = child.spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"abc").unwrap();

    let (mut socket, _) = listener.accept().unwrap();
    let mut line = Vec::new();
    while !line.ends_with(b"\n") {
        let mut byte = [0];
        socket.read_exact(&mut byte).unwrap();
        line.push(byte[0]);
    }
    socket.write_all(b"!").unwrap();
    let output = child.wait_with_output().unwrap();

    assert_eq!(line, b"helper\n");
    assert_eq!(output.stdout, harness(b"pipe:3"));
    assert_eq!(output.stderr, b"diag");
    assert_eq!(output.status.code(), Some(5));
}

#[test]
fn harness_output_wraps_and_unwraps_every_start() {
    assert_eq!(harness(b"body"), [HARNESS_START, b"body"].concat());
    let mixed = [HARNESS_START, b"one", HARNESS_START, b"two"].concat();

    assert_eq!(without_harness(&mixed), b"onetwo");
    assert_eq!(without_harness(b"plain"), b"plain");
}

#[test]
fn port_rejects_a_missing_or_malformed_variable() {
    assert!(port("MTP_SURELY_UNSET_PORT").is_err());
    assert!(port("PATH").is_err());
    assert!(variable("MTP_SURELY_UNSET_PORT").is_err());
}
