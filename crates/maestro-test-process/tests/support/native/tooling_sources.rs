//! The Cargo and recipe stand-ins the tooling tests install.

use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use maestro_test_process::child::{leave, write_stderr, write_stdout};
use serde::Deserialize;

/// The source configuration `cargo vendor` prints.
const SOURCES: &[u8] = b"[source.crates-io]\nreplace-with = \"vendored-sources\"\n";

/// The units a build leaves: a registry crate, a registry crate named like a member plus a suffix,
/// the member `example`, and the binary `tool` of that member. Each is `<package>-<hash>`; the
/// library or binary output it compiles to follows.
const UNITS: [(&str, &str); 4] = [
    ("serde-0123456789abcdef", "libserde-0123456789abcdef.rlib"),
    (
        "example-extras-0123456789abcdef",
        "libexample_extras-0123456789abcdef.rlib",
    ),
    (
        "example-fedcba9876543210",
        "libexample-fedcba9876543210.rlib",
    ),
    ("example-1111111111111111", "tool-1111111111111111"),
];

/// The operation the stand-in fails after recording itself.
#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Operation {
    /// `cargo vendor` fails.
    Vendor,
    /// `cargo build` fails.
    Build,
}

/// One stand-in's complete behavior, parsed before any stream or file effect.
#[derive(Deserialize)]
#[serde(tag = "scenario", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Record {
    /// Record the vendor and build calls, fail one of them on request, and run the setup steps.
    Sources {
        /// File that receives the arguments of every vendor call.
        calls: PathBuf,
        /// File that receives the arguments of every build call.
        builds: PathBuf,
        /// The operation that fails, if any.
        fails: Option<Operation>,
    },
    /// Print each operand on its own line, then the authored diagnostic, and end with the status.
    Operands {
        /// Standard error text.
        stderr: String,
        /// Exit status.
        code: u8,
    },
    /// Act as a Trunk hook: copy the stylesheet, report the hook environment, end with its status.
    TrunkHook {},
    /// Append the call and the matching variables to a log and refuse one chosen first operand.
    CallLog {
        /// Destination appended to, created when it is absent.
        path: String,
        /// Variable name prefixes whose variables are recorded, sorted by their lines.
        prefixes: Vec<String>,
        /// First operand that ends the invocation with status 1.
        refuse: String,
    },
    /// Append the operands to the log `CARGO_CALLS` names and report one artifact per name.
    Artifacts {
        /// Targets reported as compiled to `/built/<name>.wasm`.
        names: Vec<String>,
    },
    /// Append the operands to the `cargo-log` beside the directory holding this alias, refuse the
    /// phase `MAESTRO_FAIL_PHASE` names, skip the phases that need no build, and run the real
    /// Cargo for the rest.
    ConsumerCargo {
        /// The real Cargo.
        cargo: String,
    },
    /// Print `cargo` and the operands, and replace one named program with a copy of this alias.
    Announce {
        /// A program, relative to the working directory, that prints `example`.
        rebuilt: Option<String>,
        /// The real Cargo that answers the workspace resolution the operands ask for.
        cargo: Option<String>,
    },
}

impl Record {
    /// Read a tooling-sources record, rejecting unknown scenarios and fields.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("tooling-sources\n")
            .ok_or_else(|| io::Error::other("missing tooling-sources family"))?;
        serde_json::from_str(json).map_err(io::Error::other)
    }
}

/// Run the selected stand-in with its untouched native operands.
pub(super) fn run(record: &Record, args: &[OsString]) -> io::Result<()> {
    match record {
        Record::Sources {
            calls,
            builds,
            fails,
        } => sources(calls, builds, *fails, args),
        Record::Operands { stderr, code } => {
            let lines: Vec<_> = args.iter().map(|arg| arg.to_string_lossy()).collect();
            write_stdout(format!("{}\n", lines.join("\n")).as_bytes())?;
            write_stderr(stderr.as_bytes())?;
            leave(i32::from(*code))
        }
        Record::TrunkHook {} => trunk_hook(args),
        Record::CallLog {
            path,
            prefixes,
            refuse,
        } => call_log(path, prefixes, refuse, args),
        Record::Artifacts { names } => artifacts(names, args),
        Record::ConsumerCargo { cargo } => consumer_cargo(Path::new(cargo), args),
        Record::Announce { rebuilt, cargo } => announce(rebuilt.as_deref(), cargo.as_deref(), args),
    }
}

/// Record the call, then build or vendor as its first operand asks.
fn sources(
    calls: &Path,
    builds: &Path,
    fails: Option<Operation>,
    args: &[OsString],
) -> io::Result<()> {
    let building = args.first().is_some_and(|operation| operation == "build");
    let (log, operation, message) = if building {
        (builds, Operation::Build, "build exploded\n")
    } else {
        (calls, Operation::Vendor, "vendor exploded\n")
    };
    append(log, &operands(args))?;
    if fails == Some(operation) {
        write_stderr(message.as_bytes())?;
        leave(1);
    }
    if building { build() } else { vendor(args) }
}

/// Create the vendor directory the last operand names and print the source configuration.
pub(super) fn vendor(args: &[OsString]) -> io::Result<()> {
    let destination = args
        .last()
        .ok_or_else(|| io::Error::other("missing vendor directory"))?;
    std::fs::create_dir_all(destination)?;
    write_stdout(SOURCES)
}

/// Copy the stylesheet, report the hook's operand and environment, end with `MAESTRO_HOOK_STATUS`.
fn trunk_hook(args: &[OsString]) -> io::Result<()> {
    std::fs::copy("assets/app.css", "hook.css")?;
    let mut lines = vec![
        args.first()
            .ok_or_else(|| io::Error::other("missing hook operand"))?
            .to_string_lossy()
            .into_owned(),
    ];
    for name in [
        "TRUNK_PROFILE",
        "TRUNK_HTML_FILE",
        "TRUNK_SOURCE_DIR",
        "TRUNK_DIST_DIR",
        "TRUNK_PUBLIC_URL",
        "TRUNK_STAGING_DIR",
        "MAESTRO_HOOK_INHERITED",
    ] {
        lines.push(required(name)?);
    }
    std::fs::write("hook-environment", lines.join("\n") + "\n")?;
    let status = required("MAESTRO_HOOK_STATUS")?
        .parse::<u8>()
        .map_err(io::Error::other)?;
    leave(i32::from(status))
}

/// Record the call and the selected variables, then refuse the first operand the record names.
fn call_log(path: &str, prefixes: &[String], refuse: &str, args: &[OsString]) -> io::Result<()> {
    let mut environment: Vec<_> = std::env::vars_os()
        .map(|(name, value)| format!("{}={}", name.to_string_lossy(), value.to_string_lossy()))
        .filter(|line| {
            prefixes
                .iter()
                .any(|prefix| line.starts_with(prefix.as_str()))
        })
        .collect();
    environment.sort_unstable();
    let mut text = format!("call {}", operands(args));
    for line in environment {
        text.push('\n');
        text.push_str(&line);
    }
    append(path, &text)?;
    if args
        .first()
        .is_some_and(|first| first == OsStr::new(refuse))
    {
        leave(1);
    }
    Ok(())
}

/// Record the build and report each named target as compiled.
fn artifacts(names: &[String], args: &[OsString]) -> io::Result<()> {
    append(required("CARGO_CALLS")?, &operands(args))?;
    for name in names {
        let message = serde_json::json!({
            "reason": "compiler-artifact",
            "target": {"name": name},
            "filenames": [format!("/built/{name}.wasm")],
        });
        write_stdout(format!("{message}\n").as_bytes())?;
    }
    Ok(())
}

/// Record the call in the log beside the directory holding this alias, then refuse, skip or
/// forward it.
fn consumer_cargo(cargo: &Path, args: &[OsString]) -> io::Result<()> {
    let executable = std::env::current_exe()?;
    let log = executable
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| io::Error::other("the alias has no checkout directory"))?
        .join("cargo-log");
    append(log, &operands(args))?;
    let phase = args.first().map(|first| first.to_string_lossy());
    if std::env::var("MAESTRO_FAIL_PHASE")
        .is_ok_and(|fail| !fail.is_empty() && phase.as_deref() == Some(fail.as_str()))
    {
        leave(17);
    }
    if matches!(phase.as_deref(), Some("hakari" | "test" | "nextest")) {
        return Ok(());
    }
    let status = Command::new(cargo).args(args).status()?;
    leave(status.code().unwrap_or(1))
}

/// Print the call; where a program is named, replace it with one that prints `example`.
fn announce(rebuilt: Option<&str>, cargo: Option<&str>, args: &[OsString]) -> io::Result<()> {
    if let Some(cargo) = cargo
        && args.first().is_some_and(|arg| arg == "metadata")
    {
        return resolve(Path::new(cargo), args);
    }
    write_stdout(format!("cargo {}\n", operands(args)).as_bytes())?;
    let Some(path) = rebuilt else {
        return Ok(());
    };
    super::install(Path::new(path))?;
    std::fs::write(format!("{path}.fixture"), "echo\nexample\n")
}

/// Answer a workspace resolution with the real Cargo, leaving its streams to the caller.
fn resolve(cargo: &Path, args: &[OsString]) -> io::Result<()> {
    let mut command = Command::new(cargo);
    command.args(args);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec())
    }
    #[cfg(not(unix))]
    {
        let status = command.status()?;
        leave(status.code().unwrap_or(1));
    }
}

/// Append `text` and a newline, creating the log when it is absent.
pub(super) fn append(path: impl AsRef<Path>, text: &str) -> io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{text}")
}

/// The operands as one space-separated line.
fn operands(args: &[OsString]) -> String {
    let words: Vec<_> = args.iter().map(|arg| arg.to_string_lossy()).collect();
    words.join(" ")
}

/// The text of a variable this stand-in requires.
fn required(name: &str) -> io::Result<String> {
    std::env::var(name).map_err(|error| io::Error::other(format!("{name}: {error}")))
}

/// Leave the units of a build in the profile directory of `CARGO_TARGET_DIR`.
pub(super) fn build() -> io::Result<()> {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .ok_or_else(|| io::Error::other("CARGO_TARGET_DIR is unset"))?;
    let profile = Path::new(&target).join("debug");
    std::fs::create_dir_all(profile.join("deps"))?;
    std::fs::create_dir_all(profile.join("examples"))?;
    std::fs::create_dir_all(profile.join("incremental/example-1"))?;
    for (unit, output) in UNITS {
        std::fs::create_dir_all(profile.join(".fingerprint").join(unit))?;
        std::fs::create_dir_all(profile.join("build").join(unit))?;
        std::fs::write(profile.join("deps").join(output), "")?;
    }
    std::fs::write(profile.join("deps/example-fedcba9876543210.d"), "")?;
    std::fs::write(profile.join("deps/tool-1111111111111111.d"), "")?;
    std::fs::write(profile.join("development"), "")?;
    std::fs::write(profile.join("examples/browser_author_inputs"), "")
}
