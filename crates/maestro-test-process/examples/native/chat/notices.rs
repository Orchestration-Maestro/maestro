//! Terminal multiplexer, package manager and Git programs of the startup notices.

use std::ffi::OsString;
use std::io::{self, BufRead};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use maestro_test_process::child::{leave, write_stdout};
use serde::Deserialize;

use super::append;
use super::share::ExitSignal;

/// The options a multiplexer query may name.
const OPTIONS: [&str; 2] = ["extended-keys", "extended-keys-format"];

/// One recorded answer to a multiplexer query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Reply {
    /// Standard output bytes.
    text: String,
    /// Status when no signal is selected.
    #[serde(default)]
    code: u8,
    /// Native termination instead of a status, after writing.
    #[serde(default)]
    signal: Option<ExitSignal>,
}

/// The option a `show -gv <option>` query names.
fn option(operands: &[OsString]) -> io::Result<&str> {
    let [show, flags, option] = operands else {
        return Err(io::Error::other("expected show, -gv and one option"));
    };
    match (show.to_str(), flags.to_str(), option.to_str()) {
        (Some("show"), Some("-gv"), Some(option)) if OPTIONS.contains(&option) => Ok(option),
        _ => Err(io::Error::other("expected a multiplexer option query")),
    }
}

/// Answer the queried option with its recorded reply.
pub(super) fn tmux(keys: &Reply, format: &Reply, operands: &[OsString]) -> io::Result<()> {
    let reply = if option(operands)? == OPTIONS[0] {
        keys
    } else {
        format
    };
    write_stdout(reply.text.as_bytes())?;
    if let Some(signal) = &reply.signal {
        signal.raise()?;
    }
    leave(i32::from(reply.code))
}

/// What the test observes of a gated program before it is permitted to answer.
fn observation(operands: &[OsString]) -> io::Result<Vec<u8>> {
    let argv: Vec<_> = operands
        .iter()
        .map(|operand| operand.to_string_lossy())
        .collect();
    let inherited = std::env::var("MAESTRO_NOTICE_INHERITED").ok();
    serde_json::to_vec(&serde_json::json!({
        "argv": argv,
        "cwd": std::env::current_dir()?,
        "inherited": inherited,
    }))
    .map_err(io::Error::other)
}

/// Report the launch on `<name>.ready`, then wait for the line `go` on `<name>.permit`.
fn gate(directory: &str, name: &str, operands: &[OsString]) -> io::Result<()> {
    let directory = Path::new(directory);
    std::fs::write(
        directory.join(format!("{name}.ready")),
        observation(operands)?,
    )?;
    let permit = std::fs::File::open(directory.join(format!("{name}.permit")))?;
    let mut line = String::new();
    io::BufReader::new(permit).read_line(&mut line)?;
    if line == "go\n" {
        Ok(())
    } else {
        Err(io::Error::other("the permit was not `go`"))
    }
}

/// Answer a gated multiplexer query while ignoring termination.
pub(super) fn gated_tmux(directory: &str, operands: &[OsString]) -> io::Result<()> {
    let option = option(operands)?;
    // A handler that only sets a flag leaves the process running when it is asked to end.
    signal_hook::flag::register(
        signal_hook::consts::SIGTERM,
        Arc::new(AtomicBool::new(false)),
    )?;
    gate(directory, option, operands)?;
    write_stdout(if option == OPTIONS[0] {
        b"off\n"
    } else {
        b"xterm\n"
    })?;
    leave(0)
}

/// Answer a gated package version query for the one expected release lookup.
pub(super) fn gated_package(directory: &str, operands: &[OsString]) -> io::Result<()> {
    if operands != ["view", "before", "version", "--json"] {
        return Err(io::Error::other("expected the package version query"));
    }
    gate(directory, "package", operands)?;
    write_stdout(b"\"2\"")?;
    leave(0)
}

/// Pause a search program's version probe until permitted.
pub(super) fn gated_probe(directory: &str, operands: &[OsString]) -> io::Result<()> {
    if operands != ["--version"] {
        return Err(io::Error::other("expected the version probe"));
    }
    gate(directory, "init", operands)?;
    leave(0)
}

/// Append the working directory and every operand, `|`-separated, then answer version `2`.
pub(super) fn package_version(log: &str, operands: &[OsString]) -> io::Result<()> {
    let mut line = std::env::current_dir()?
        .into_os_string()
        .into_encoded_bytes();
    for operand in operands {
        line.push(b'|');
        line.extend_from_slice(operand.as_encoded_bytes());
    }
    line.push(b'\n');
    append(log, &line)?;
    write_stdout(b"\"2\"")?;
    leave(0)
}

/// Log one query as a JSON line, then answer the three queries an update check makes.
pub(super) fn package_remote(log: &str, operands: &[OsString]) -> io::Result<()> {
    let argv: Vec<_> = operands
        .iter()
        .map(|operand| operand.to_string_lossy())
        .collect();
    let mut line = serde_json::to_vec(&serde_json::json!({
        "cwd": std::env::current_dir()?,
        "argv": argv,
        "prompt": std::env::var("GIT_TERMINAL_PROMPT").ok(),
    }))
    .map_err(io::Error::other)?;
    line.push(b'\n');
    append(log, &line)?;
    let answer = match argv.iter().map(|operand| &**operand).collect::<Vec<_>>()[..] {
        ["rev-parse", "HEAD"] => format!("{}\n", "a".repeat(40)),
        ["rev-parse", "--abbrev-ref", "@{upstream}"] => "origin/main\n".into(),
        ["ls-remote", "origin", "refs/heads/main"] => {
            format!("{}\trefs/heads/main\n", "b".repeat(40))
        }
        _ => return Err(io::Error::other("unexpected repository query")),
    };
    write_stdout(answer.as_bytes())?;
    leave(0)
}
