//! The Cargo the fixture-sources setup runs: it vendors a source set and builds the seed units.

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

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
enum Operation {
    /// `cargo vendor` fails.
    Vendor,
    /// `cargo build` fails.
    Build,
}

/// Where the stand-in records its calls and which operation fails, parsed before any effect.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    /// File that receives the arguments of every vendor call.
    calls: PathBuf,
    /// File that receives the arguments of every build call.
    builds: PathBuf,
    /// The operation that fails, if any.
    fails: Option<Operation>,
}

impl Record {
    /// Read a tooling-sources record, rejecting unknown fields.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("tooling-sources\n")
            .ok_or_else(|| io::Error::other("missing tooling-sources family"))?;
        serde_json::from_str(json).map_err(io::Error::other)
    }
}

/// Record the call, then build or vendor as its first operand asks.
pub(super) fn run(record: &Record, args: &[OsString]) -> io::Result<()> {
    let building = args.first().is_some_and(|operation| operation == "build");
    let (log, operation, message) = if building {
        (&record.builds, Operation::Build, "build exploded\n")
    } else {
        (&record.calls, Operation::Vendor, "vendor exploded\n")
    };
    let line = args
        .iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    writeln!(
        std::fs::File::options()
            .append(true)
            .create(true)
            .open(log)?,
        "{line}"
    )?;
    if record.fails == Some(operation) {
        write_stderr(message.as_bytes())?;
        leave(1);
    }
    if building {
        build()
    } else {
        let destination = args
            .last()
            .ok_or_else(|| io::Error::other("missing vendor directory"))?;
        std::fs::create_dir_all(destination)?;
        write_stdout(SOURCES)
    }
}

/// Leave the units of a build in the profile directory of `CARGO_TARGET_DIR`.
fn build() -> io::Result<()> {
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
