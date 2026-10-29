//! Child probes and the interface matrix of the checked test process.
#![cfg(test)]

mod checked_runs;
mod environment_groups;
mod user_cache;
mod windows_console;

use std::ffi::OsStr;
use std::fmt::Write as _;
use std::io::{BufRead, Read, Write};

/// Raw bytes of an operating-system string, lossy where the platform has no byte form.
pub(crate) fn raw_bytes(value: &OsStr) -> Vec<u8> {
    #[cfg(unix)]
    {
        std::os::unix::ffi::OsStrExt::as_bytes(value).to_vec()
    }
    #[cfg(not(unix))]
    {
        value.to_string_lossy().into_owned().into_bytes()
    }
}

/// Lower-case hexadecimal spelling of `bytes`.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut spelled, byte| {
        write!(spelled, "{byte:02x}").unwrap();
        spelled
    })
}

/// Describe each variable named in `MTP_VARS` as `name=hex` or `name=-` when absent.
fn describe_environment(out: &mut impl Write) {
    let names = std::env::var("MTP_VARS").unwrap_or_default();
    for name in names.split(',').filter(|name| !name.is_empty()) {
        match std::env::var_os(name) {
            Some(value) => writeln!(out, "{name}={}", hex(&raw_bytes(&value))),
            None => writeln!(out, "{name}=-"),
        }
        .unwrap();
    }
}

/// Wait for the parent's acknowledgement line on standard input.
fn acknowledged() {
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).unwrap();
    assert_eq!(line, "continue\n");
}

/// Write bytes the harness must not alter, then end the way `MTP_END` says.
fn emit_bytes(out: &mut impl Write, err: &mut impl Write) {
    out.write_all(b"out\0\xff").unwrap();
    err.write_all(b"err\0\xfe").unwrap();
    out.flush().unwrap();
    match std::env::var("MTP_END").as_deref() {
        Ok("panic") => panic!("probe panic"),
        Ok("exit7") => std::process::exit(7),
        Ok("signal") => std::process::abort(),
        _ => {}
    }
}

/// Report on standard error whether a write to a closed standard output fails.
///
/// Stop after at most 256 bytes, below the minimum pipe capacity, so a retained reader
/// reports a clear failure instead of blocking on a full pipe.
fn write_after_close(out: &mut impl Write) {
    for _ in 0..128 {
        if out.write_all(b"x\n").and_then(|()| out.flush()).is_err() {
            writeln!(std::io::stderr(), "broken-pipe").unwrap();
            return;
        }
    }
    panic!("stdout still has a reader after its checked read end was dropped");
}

/// Describe whether each standard descriptor is a regular file, then echo standard input.
fn describe_descriptors(out: &mut impl Write) {
    for (name, path) in [
        ("stdin", "/dev/stdin"),
        ("stdout", "/dev/stdout"),
        ("stderr", "/dev/stderr"),
    ] {
        let file = std::fs::metadata(path).is_ok_and(|meta| meta.is_file());
        writeln!(out, "{name}-file={file}").unwrap();
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    writeln!(out, "input={input:?}").unwrap();
    writeln!(std::io::stderr(), "diagnostic").unwrap();
}

/// The controlled body shared by every probe; `MTP_BEHAVIOR` selects what it does.
pub(crate) fn behave(label: &str) {
    let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
    match std::env::var("MTP_BEHAVIOR").unwrap_or_default().as_str() {
        "marker" => {
            let marker = std::env::var_os("MTP_MARKER").unwrap();
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(marker)
                .unwrap();
            writeln!(file, "{label}").unwrap();
        }
        "name" => writeln!(
            out,
            "identity={}",
            std::thread::current().name().unwrap_or("-")
        )
        .unwrap(),
        "argv" => std::env::args()
            .skip(1)
            .for_each(|arg| writeln!(out, "arg={arg:?}").unwrap()),
        "print" => println!("printed {label}"),
        "bytes" => emit_bytes(&mut out, &mut err),
        "exit" => {
            writeln!(out, "leaving").unwrap();
            out.flush().unwrap();
            std::process::exit(0);
        }
        "live" => {
            writeln!(out, "READY").unwrap();
            out.flush().unwrap();
            acknowledged();
            writeln!(out, "after").unwrap();
        }
        "pipe" => {
            writeln!(out, "READY").unwrap();
            out.flush().unwrap();
            acknowledged();
            write_after_close(&mut out);
        }
        "env" => describe_environment(&mut out),
        "cwd" => {
            describe_environment(&mut out);
            writeln!(out, "cwd={}", std::env::current_dir().unwrap().display()).unwrap();
        }
        "echo" => {
            let mut input = Vec::new();
            std::io::stdin().read_to_end(&mut input).unwrap();
            out.write_all(b"stdin:").unwrap();
            out.write_all(&input).unwrap();
            err.write_all(b"err\0\xff").unwrap();
        }
        "file" => describe_descriptors(&mut out),
        _ => writeln!(out, "idle {label}").unwrap(),
    }
}

/// Standalone probe selected by its bare identity.
#[test]
#[ignore = "runs only inside a controlled child"]
fn bare_child() {
    behave("bare");
}
