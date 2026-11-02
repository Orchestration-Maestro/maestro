//! Controlled repository commands, installed under the names a recipe resolves.

use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

use maestro_test_process::child::{leave, write_stderr, write_stdout};
use serde::Deserialize;

/// The command stand-in's complete behavior, parsed before any stream or file effect.
#[derive(Deserialize)]
#[serde(tag = "scenario", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Record {
    /// Answer a recipe's command, running the real program where the recipe needs one.
    StandIn,
    /// Record the invocation, refuse it on request, and otherwise run the real program.
    Forward,
}

impl Record {
    /// Read a tooling-command record, rejecting unknown scenarios and fields.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("tooling-command\n")
            .ok_or_else(|| io::Error::other("missing tooling-command family"))?;
        serde_json::from_str(json).map_err(io::Error::other)
    }
}

/// Run the selected stand-in with its untouched native operands.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    match record {
        Record::StandIn => stand_in(operands),
        Record::Forward => forwarded(operands),
    }
}

/// Answer a recipe command from the recorded environment, running what the recipe really needs.
fn stand_in(operands: &[OsString]) -> io::Result<()> {
    if std::env::var_os("MAESTRO_COMMAND_OUTPUT").is_some() {
        write_stdout(b"controlled command stdout\n")?;
        write_stderr(b"controlled command stderr\n")?;
    }
    let development = std::env::var_os("MAESTRO_DEVELOPMENT");
    let started = development_operands(operands);
    if std::env::var_os("MAESTRO_REAL_CARGO").is_some()
        && let (Some(binary), Some(arguments)) = (&development, started)
    {
        return run_development(binary, arguments);
    }
    if invoked() == OsStr::new("watchexec")
        && let Some(watcher) = std::env::var_os("MAESTRO_REAL_WATCHEXEC")
    {
        return run_program(
            Command::new(watcher)
                .arg("--wrap-process=none")
                .args(operands),
        );
    }
    if let Some(cargo) = std::env::var_os("MAESTRO_REAL_CARGO") {
        return run_cargo(Path::new(&cargo), operands);
    }
    log(operands)?;
    if std::env::var("MAESTRO_FAIL_ARG")
        .is_ok_and(|value| operands.iter().any(|operand| operand == OsStr::new(&value)))
    {
        leave(17);
    }
    if let (Some(binary), Some(arguments)) = (&development, started) {
        return run_development(binary, arguments);
    }
    if std::env::var("MAESTRO_FAIL").ok().as_deref()
        == operands.first().and_then(|operand| operand.to_str())
    {
        leave(17);
    }
    Ok(())
}

/// Record the invocation, refuse it on request, then run the real program.
fn forwarded(operands: &[OsString]) -> io::Result<()> {
    let log = std::env::var_os("MAESTRO_NATIVE_LOG")
        .ok_or_else(|| io::Error::other("MAESTRO_NATIVE_LOG is unset"))?;
    let line: Vec<_> = operands
        .iter()
        .map(|operand| operand.to_string_lossy())
        .collect();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)?;
    writeln!(file, "{}", line.join(" "))?;
    let version = operands
        .first()
        .is_some_and(|first| first == OsStr::new("--version"));
    if std::env::var("MAESTRO_NATIVE_FAIL").as_deref() == Ok("1") && !version {
        write_stderr(b"controlled native tool failure\n")?;
        leave(17);
    }
    let tool = std::env::var_os("MAESTRO_NATIVE_TOOL")
        .ok_or_else(|| io::Error::other("MAESTRO_NATIVE_TOOL is unset"))?;
    run_program(Command::new(tool).args(operands))
}

/// The operands of a `run -- <program>` command, which the recipe forwards instead of building.
fn development_operands(operands: &[OsString]) -> Option<&[OsString]> {
    let separator = operands
        .iter()
        .position(|operand| operand == OsStr::new("--"))?;
    operands
        .first()
        .is_some_and(|operand| operand == OsStr::new("run"))
        .then_some(&operands[separator + 1..])
}

/// Run the development binary in the checkout the recipe expects it from.
fn run_development(binary: &OsStr, arguments: &[OsString]) -> io::Result<()> {
    let manifest = std::env::current_dir()?.join("crates/maestro-tooling");
    run_program(
        Command::new(binary)
            .args(arguments)
            .env("CARGO_MANIFEST_DIR", manifest),
    )
}

/// Forward the real Cargo's status, and after its build run the two example programs it stages.
fn run_cargo(cargo: &Path, operands: &[OsString]) -> io::Result<()> {
    let status = Command::new(cargo).args(operands).status()?;
    let builds = operands
        .first()
        .is_some_and(|operand| operand == OsStr::new("build"));
    if status.success() && builds {
        for (binary, arguments) in [
            ("maestro-models", &[][..]),
            ("development", &["--rebuild-marker"][..]),
        ] {
            run_staged(binary, arguments)?;
        }
    }
    leave(status.code().unwrap_or(1))
}

/// Run one program a real build staged, when the build emitted it.
fn run_staged(binary: &str, arguments: &[&str]) -> io::Result<()> {
    let path = Path::new("target/debug").join(binary);
    if !path.is_file() {
        return Ok(());
    }
    let status = Command::new(&path).args(arguments).status()?;
    if status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "{} ended with {status}",
        path.display()
    )))
}

/// Record the invocation, its working directory and the requested environment observations.
fn log(operands: &[OsString]) -> io::Result<()> {
    if let Some(path) = std::env::var_os("MAESTRO_CWD") {
        std::fs::write(
            path,
            std::env::current_dir()?
                .into_os_string()
                .into_encoded_bytes(),
        )?;
    }
    let operands: Vec<_> = operands
        .iter()
        .map(|operand| operand.to_string_lossy())
        .collect();
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(
        std::env::var_os("MAESTRO_LOG").ok_or_else(|| io::Error::other("MAESTRO_LOG is unset"))?,
    )?;
    writeln!(
        file,
        "{} {}",
        invoked().to_string_lossy(),
        operands.join("|")
    )?;
    if let Some(path) = std::env::var_os("MAESTRO_BRAND_RESULT") {
        let value = format!(
            "{}\n{}",
            std::env::var("MAESTRO_BRAND_PACK").unwrap_or_default(),
            std::env::var("MAESTRO_BRAND_MODE").unwrap_or_default()
        );
        std::fs::write(path, value)?;
    }
    if let Some(path) = std::env::var_os("MAESTRO_ENV_RESULT") {
        let home = std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME is unset"))?;
        let auth = Path::new(&home).join(".maestro/agent/auth.json");
        std::fs::write(
            path,
            format!(
                "key={} auth={} flag={}\n",
                std::env::var_os("OPENAI_API_KEY").is_some(),
                auth.is_file(),
                std::env::var("MAESTRO_NO_LOCAL_LLM").unwrap_or_default()
            ),
        )?;
    }
    Ok(())
}

/// The name this alias was installed under, which selects the recipe it answers.
fn invoked() -> OsString {
    let executable = std::env::args_os().next().unwrap_or_default();
    Path::new(&executable)
        .file_stem()
        .map(OsStr::to_os_string)
        .unwrap_or_default()
}

/// Run a real program and take its status.
fn run_program(command: &mut Command) -> io::Result<()> {
    let status = command.status()?;
    leave(status.code().unwrap_or(1))
}
