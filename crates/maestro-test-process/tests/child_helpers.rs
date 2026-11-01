//! The helpers that parent tests and re-run child programs share.
#![cfg(test)]
#![cfg(unix)]

use maestro_test_process::child::{
    Stream, UnknownStream, announce, close_streams, collect_output, connect, harness, is_null,
    leave, port, read_input, receive, variable, without_harness, write_stderr, write_stdout,
};
use maestro_test_process::{HARNESS_START, TestProcess};
use std::env::VarError;
use std::error::Error;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::num::ParseIntError;
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
    let collected = collect_output(child, &listener).unwrap();

    let (mut socket, _) = listener.accept().unwrap();
    let mut line = Vec::new();
    while !line.ends_with(b"\n") {
        let mut byte = [0];
        socket.read_exact(&mut byte).unwrap();
        line.push(byte[0]);
    }
    socket.write_all(b"!").unwrap();
    let output = collected.join().unwrap().unwrap();

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
fn variable_error_names_the_variable_and_keeps_the_cause() {
    let error = variable("MTP_SURELY_UNSET_PORT").unwrap_err();

    assert!(error.to_string().contains("MTP_SURELY_UNSET_PORT"));
    assert!(error.source().unwrap().is::<VarError>());
}

#[test]
fn port_error_names_the_variable_and_keeps_the_cause() {
    let missing = port("MTP_SURELY_UNSET_PORT").unwrap_err();
    let malformed = port("PATH").unwrap_err();

    assert!(missing.source().unwrap().is::<VarError>());
    assert!(malformed.to_string().contains("PATH"));
    assert!(malformed.source().unwrap().is::<ParseIntError>());
}

#[test]
fn socket_helpers_work_over_any_reader_and_writer() {
    let mut line = Vec::new();

    announce(&mut line, "ready").unwrap();

    assert_eq!(line, b"ready\n");
    assert_eq!(receive(&mut &b"x"[..]).unwrap(), Some(b'x'));
    assert_eq!(receive(&mut &b""[..]).unwrap(), None);
}

#[test]
fn stream_lists_round_trip_through_their_text() {
    let streams = [Stream::Input, Stream::Output, Stream::Error];

    assert_eq!(
        Stream::parse_list(&Stream::list_text(&streams)),
        Ok(streams.to_vec())
    );
    assert_eq!(Stream::parse_list(""), Ok(vec![]));
}

#[test]
fn stream_list_rejects_an_unknown_stream() {
    let error = Stream::parse_list("stdout,bogus").unwrap_err();

    assert_eq!(error, UnknownStream("bogus".to_owned()));
    assert!(error.to_string().contains("bogus"));
}

/// A child that replaces two descriptors and reports which of its three are the null device.
#[test]
#[ignore = "runs only inside a controlled child"]
fn stream_closer() {
    close_streams(&[Stream::Input, Stream::Error]).unwrap();
    let kind = |path| {
        if is_null(path).unwrap() {
            "null"
        } else {
            "pipe"
        }
    };
    write_stdout(
        format!(
            "in:{} out:{} err:{}",
            kind("/dev/stdin"),
            kind("/dev/stdout"),
            kind("/dev/stderr")
        )
        .as_bytes(),
    )
    .unwrap();
    leave(0)
}

#[test]
fn close_streams_replaces_only_the_named_descriptors() {
    let mut command = TestProcess::child(stream_closer)
        .unwrap()
        .command()
        .unwrap();
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = command.spawn().unwrap().wait_with_output().unwrap();

    assert_eq!(output.stdout, harness(b"in:null out:pipe err:null"));
}

/// A child that leaves with output it never flushed.
#[test]
#[ignore = "runs only inside a controlled child"]
fn unflushed_leaver() {
    read_input().unwrap();
    std::io::stdout().write_all(b"unflushed").unwrap();
    leave(0)
}

#[test]
fn leave_fails_when_standard_output_cannot_be_flushed() {
    let mut command = TestProcess::child(unflushed_leaver)
        .unwrap()
        .command()
        .unwrap();
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    stdout.read_exact(&mut [0; HARNESS_START.len()]).unwrap();
    drop(stdout);
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();

    assert_ne!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stderr).contains("flush"));
}

/// A child that ends without ever connecting.
#[test]
#[ignore = "runs only inside a controlled child"]
fn silent_leaver() {
    leave(3)
}

#[test]
fn collect_output_wakes_an_accept_the_child_never_answers() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let mut command = TestProcess::child(silent_leaver)
        .unwrap()
        .command()
        .unwrap();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let collected = collect_output(command.spawn().unwrap(), &listener).unwrap();

    let (mut socket, _) = listener.accept().unwrap();

    assert_eq!(socket.read(&mut [0]).unwrap(), 0);
    assert_eq!(collected.join().unwrap().unwrap().status.code(), Some(3));
}
