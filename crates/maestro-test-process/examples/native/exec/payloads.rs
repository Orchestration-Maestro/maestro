//! The payload bodies of the controlled command family.

use std::{
    ffi::OsString,
    io::{self, Write},
    net::TcpStream,
};

use crate::native::termination::Termination;
use maestro_test_process::child::{
    announce, connect, read_input, receive, write_stderr, write_stdout,
};

/// The large stream each of the two independent outputs carries.
const STREAM_LENGTH: usize = 300_000;

/// The zero-filled tail after the interleaved scalar and diagnostic bytes.
const RAW_LENGTH: usize = 262_144;

/// One four-byte scalar, written in two parts to both outputs.
const SCALAR: &str = "\u{10348}";

/// Report the operands, directory and home exactly as the caller supplied them.
pub(super) fn operands(operands: &[OsString]) -> io::Result<()> {
    if !read_input()?.is_empty() {
        return Err(io::Error::other("standard input must be closed"));
    }
    let operand = operands
        .first()
        .ok_or_else(|| io::Error::other("missing operand"))?;
    let home = std::env::var("HOME").map_err(io::Error::other)?;
    let directory = std::env::current_dir()?;
    write_stdout(
        format!(
            "{}|{}|{home}",
            operand.to_string_lossy(),
            directory.display()
        )
        .as_bytes(),
    )
}

/// Write one large terminal-coloured output and one large diagnostic output.
pub(super) fn streams() -> io::Result<()> {
    write_stdout(b"\x1b[31mred\x1b[0m\r\0")?;
    write_stdout(&vec![b'o'; STREAM_LENGTH])?;
    write_stderr(&vec![b'e'; STREAM_LENGTH])?;
    write_stderr(b"\r\n")
}

/// Write one scalar to both outputs around the parent's release, keeping its bytes split.
pub(super) fn utf8(port: u16, split: usize) -> io::Result<()> {
    let bytes = SCALAR.as_bytes();
    let (head, tail) = bytes
        .split_at_checked(split)
        .ok_or_else(|| io::Error::other("split beyond one scalar"))?;
    write_stdout(head)?;
    write_stderr(head)?;
    let mut socket = connect(port)?;
    announce(&mut socket, "READY")?;
    receive(&mut socket)?;
    write_stdout(tail)?;
    write_stderr(tail)
}

/// Write bytes that are never a complete scalar on either output.
pub(super) fn malformed() -> io::Result<()> {
    write_stdout(b"a\xffb")?;
    write_stderr(b"\xf0\x90")
}

/// Report the directory this process inherited.
pub(super) fn cwd() -> io::Result<()> {
    let directory = std::env::current_dir()?;
    write_stdout(directory.as_os_str().as_encoded_bytes())
}

/// Announce readiness, then finish after the parent's release byte.
pub(super) fn holder(port: u16) -> io::Result<()> {
    let mut socket = connect(port)?;
    announce(&mut socket, "READY")?;
    receive(&mut socket)?;
    write_stdout(b"bye")
}

/// Answer every TERM with `bye` and `T` until killed, released or disconnected.
pub(super) fn stubborn(port: u16) -> io::Result<()> {
    let notifications = Termination::new()?;
    let mut socket = connect(port)?;
    let answers = socket.try_clone()?;
    drop(std::thread::spawn(move || {
        answer_each_termination(notifications, answers);
    }));
    announce(&mut socket, "READY")?;
    receive(&mut socket)?;
    Ok(())
}

/// Report each queued termination until the parent kills this process.
fn answer_each_termination(mut notifications: Termination, mut socket: TcpStream) {
    while notifications.wait().is_ok() {
        if write_stdout(b"bye").is_err() || socket.write_all(b"T").is_err() {
            return;
        }
    }
}

/// Write the direct child's bytes, then leave a descendant holding both pipes.
pub(super) fn descendant() -> io::Result<()> {
    write_stdout(b"early")?;
    write_stderr(b"diag")?;
    drop(super::spawn(super::Role::Descendant)?);
    Ok(())
}

/// Answer the parent's questions after the direct child exited.
pub(super) fn descendant_holder(port: u16) -> io::Result<()> {
    let mut socket = connect(port)?;
    announce(&mut socket, "READY")?;
    loop {
        match receive(&mut socket)? {
            Some(b'p') => socket.write_all(b"E")?,
            Some(_) => return write_stdout(b"late"),
            None => return Ok(()),
        }
    }
}

/// Require both release bytes while writing to each output in turn.
pub(super) fn exchange(port: u16) -> io::Result<()> {
    let mut socket = connect(port)?;
    if receive(&mut socket)? != Some(b'x') {
        return Err(io::Error::other("missing first release"));
    }
    write_stdout(b"one")?;
    if receive(&mut socket)? != Some(b'x') {
        return Err(io::Error::other("missing second release"));
    }
    write_stderr(b"two")
}

/// Write raw output before release and diagnostic bytes only after release.
pub(super) fn raw(port: u16) -> io::Result<()> {
    let mut socket = connect(port)?;
    write_stdout(b"raw\0\x1b[31m\r\n\xff")?;
    receive(&mut socket)?.ok_or_else(|| io::Error::other("missing raw release"))?;
    write_stderr(b"stderr")
}

/// Keep a scalar split around a diagnostic write, releasing each part independently.
pub(super) fn interleaved(port: u16) -> io::Result<()> {
    let mut socket = connect(port)?;
    write_stdout(b"\xf0\x9f")?;
    receive(&mut socket)?.ok_or_else(|| io::Error::other("missing first release"))?;
    write_stderr(b"err")?;
    receive(&mut socket)?.ok_or_else(|| io::Error::other("missing second release"))?;
    write_stdout(b"\x98\x80")?;
    write_stdout(&vec![0; RAW_LENGTH])
}

/// Report the native PID, acknowledge the first release, then mark survival after the second.
pub(super) fn termination(port: u16) -> io::Result<()> {
    let mut socket = connect(port)?;
    write_stdout(format!("{}\n", std::process::id()).as_bytes())?;
    receive(&mut socket)?.ok_or_else(|| io::Error::other("missing first release"))?;
    write_stdout(b"alive\n")?;
    receive(&mut socket)?.ok_or_else(|| io::Error::other("missing second release"))?;
    std::fs::write("survived", b"survived")
}

/// Report the session and process group this process leads or belongs to.
pub(super) fn identity() -> io::Result<()> {
    let session = rustix::process::getsid(None).map_err(io::Error::from)?;
    let group = rustix::process::getpgrp();
    write_stdout(format!("{session}\n{group}\n").as_bytes())
}

/// Answer each question under the default termination action.
pub(super) fn yielding(port: u16) -> io::Result<()> {
    let mut socket = connect(port)?;
    announce(&mut socket, "READY")?;
    while receive(&mut socket)? == Some(b'p') {
        socket.write_all(b"E")?;
    }
    Ok(())
}

/// Hold one control channel beside a linked descendant until the tree is killed.
pub(super) fn tree_root(port: u16) -> io::Result<()> {
    let mut socket = connect(port)?;
    socket.write_all(b"READY")?;
    let status = super::spawn(super::Role::TreeLeaf)?.wait()?;
    receive(&mut socket)?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("linked descendant did not end"))
    }
}

/// Hold one control channel until the owning process tree is killed.
pub(super) fn tree_leaf(port: u16) -> io::Result<()> {
    let mut socket = connect(port)?;
    socket.write_all(b"READY")?;
    receive(&mut socket).map(|_| ())
}

/// Finish after one termination signal, reporting it with `code` when supplied.
/// Announce readiness on `port`, when supplied, only after registering TERM.
pub(super) fn blocked(code: Option<i32>, port: Option<u16>) -> io::Result<()> {
    let mut notifications = Termination::new()?;
    let mut socket = port.map(connect).transpose()?;
    if let Some(socket) = &mut socket {
        announce(socket, "READY")?;
    }
    notifications.wait()?;
    match code {
        Some(code) => std::process::exit(code),
        None => Ok(()),
    }
}

/// Write grace bytes into another process's output descriptor.
#[cfg(target_os = "linux")]
pub(super) fn grace(pid: u32) -> io::Result<()> {
    use std::io::BufRead;

    let mut target = std::fs::OpenOptions::new()
        .write(true)
        .open(format!("/proc/{pid}/fd/1"))?;
    write_stdout(b"held")?;
    let mut lines = std::io::BufReader::new(io::stdin());
    let mut line = String::new();
    lines.read_line(&mut line)?;
    target.write_all(b"grace")?;
    target.flush()?;
    line.clear();
    lines.read_line(&mut line)?;
    Ok(())
}

/// Report the missing platform instead of writing to a descriptor this host cannot reach.
#[cfg(not(target_os = "linux"))]
pub(super) fn grace(_pid: u32) -> io::Result<()> {
    Err(io::Error::other(
        "grace bytes need a Linux output descriptor",
    ))
}
