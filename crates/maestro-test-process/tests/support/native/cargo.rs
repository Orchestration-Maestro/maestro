//! Cargo boundary used by repository preparation's existing shared fixture.

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use maestro_test_process::child::{leave, write_stdout};

/// Offset from the Unix epoch to 2020-01-01T00:00Z for deterministic cache-age checks.
const FIXTURE_MODIFIED_OFFSET: std::time::Duration = std::time::Duration::from_hours(438_288);

/// Inputs of the preparation fixture, supplied beside its Cargo alias.
pub(super) struct Record {
    /// Real Cargo used only for workspace metadata.
    cargo: String,
    /// Finite author-example declaration installed beside the built fixture.
    template: String,
}

impl Record {
    /// Require exactly the family, native Cargo and template paths before effects.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let fields: Vec<_> = text.lines().collect();
        let ["tooling-cargo", cargo, template] = fields.as_slice() else {
            return Err(io::Error::other(
                "expected tooling-cargo, Cargo and template paths",
            ));
        };
        if cargo.is_empty() || template.is_empty() {
            return Err(io::Error::other(
                "Cargo and template paths must not be empty",
            ));
        }
        Ok(Self {
            cargo: (*cargo).to_owned(),
            template: (*template).to_owned(),
        })
    }
}

/// The example builds this fixture supports.
enum Example {
    /// The shared setup example whose declaration the caller supplies.
    Author,
    /// The single raw native fixture itself.
    Native,
    /// The instrumented parent the toolchain counter test launches.
    Probe,
}

/// A build parsed before any directory or log is written.
struct Build {
    /// Destination Cargo target directory.
    target: PathBuf,
    /// Which exact example to stage.
    example: Example,
}

/// The value immediately after a named Cargo option.
fn option<'a>(args: &'a [OsString], name: &str) -> io::Result<&'a OsString> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| &pair[1])
        .ok_or_else(|| io::Error::other(format!("missing Cargo option {name}")))
}

impl Build {
    /// Accept only the existing dev-profile example operations.
    fn parse(args: &[OsString]) -> io::Result<Self> {
        if option(args, "--profile")? != "dev" {
            return Err(io::Error::other("fixture build requires the dev profile"));
        }
        let example = match option(args, "--example")?.to_str() {
            Some("native_fixture") => Example::Native,
            Some("prepare_author_component") => Example::Author,
            Some("child_toolchain_probe") => Example::Probe,
            _ => return Err(io::Error::other("unknown preparation example")),
        };
        Ok(Self {
            target: option(args, "--target-dir")?.into(),
            example,
        })
    }
}

/// Forward real metadata unchanged; stage the two finite example targets and the locked
/// installs for other calls.
pub(super) fn run(record: &Record, args: &[OsString]) -> io::Result<()> {
    match args.first().and_then(|value| value.to_str()) {
        Some("metadata") => {
            let mut command = Command::new(&record.cargo);
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
        Some("clippy") => Ok(()),
        Some("install") => install(args),
        Some("build") if args.iter().any(|arg| arg == "--target") => build_wasm(args),
        Some(operation @ ("build" | "run" | "rustc")) => {
            let parsed = Build::parse(args)?;
            if operation == "rustc" && !matches!(parsed.example, Example::Probe) {
                return Err(io::Error::other("only the probe compiles through rustc"));
            }
            build(record, &parsed, operation == "run")
        }
        _ => Err(io::Error::other("unknown fixture Cargo operation")),
    }
}

/// Record a locked install and stage the executable it would place.
///
/// The record keeps the operands the preparation passed, so a test reads the exact command
/// instead of a shell imitation of Cargo.
fn install(args: &[OsString]) -> io::Result<()> {
    let root = Path::new(option(args, "--root")?);
    let name = ["--example", "--bin"]
        .into_iter()
        .find_map(|flag| option(args, flag).ok())
        .ok_or_else(|| io::Error::other("install names no target"))?;
    let words: Vec<_> = args.iter().map(|arg| arg.to_string_lossy()).collect();
    writeln!(
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("install-args")?,
        "{}",
        words.join(" ")
    )?;
    let binary = root.join("bin").join(format!(
        "{}{}",
        name.to_string_lossy(),
        std::env::consts::EXE_SUFFIX
    ));
    std::fs::create_dir_all(
        binary
            .parent()
            .ok_or_else(|| io::Error::other("missing install bin directory"))?,
    )?;
    std::fs::copy(std::env::current_exe()?, &binary)?;
    executable(&binary)
}

/// Stage an existing fixture, counting author, native and probe builds independently.
fn build(record: &Record, build: &Build, start: bool) -> io::Result<()> {
    let (name, source, log) = match build.example {
        Example::Author => (
            "prepare_author_component",
            std::env::current_exe()?,
            "builds",
        ),
        Example::Native => ("native_fixture", std::env::current_exe()?, "native-builds"),
        Example::Probe => (
            "child_toolchain_probe",
            std::env::current_exe()?,
            "probe-builds",
        ),
    };
    let declaration = if matches!(build.example, Example::Author) {
        let text = std::fs::read_to_string(&record.template)?;
        super::author::Record::parse(&text)?;
        Some(text)
    } else {
        None
    };
    let path = build
        .target
        .join("debug/examples")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    std::fs::create_dir_all(
        path.parent()
            .ok_or_else(|| io::Error::other("missing example directory"))?,
    )?;
    std::fs::copy(source, &path)?;
    executable(&path)?;
    if let Some(text) = declaration {
        let mut sibling = path.as_os_str().to_owned();
        sibling.push(".fixture");
        std::fs::write(sibling, text)?;
    }
    writeln!(
        std::fs::File::options()
            .append(true)
            .create(true)
            .open(log)?,
        "build"
    )?;
    if !matches!(build.example, Example::Author) {
        let message = serde_json::json!({"reason": "compiler-artifact", "target": {"name": name}, "executable": path});
        write_stdout(&serde_json::to_vec(&message)?)?;
        write_stdout(b"\n")?;
    }
    if start {
        leave(Command::new(path).status()?.code().unwrap_or(1));
    }
    Ok(())
}

/// Report the two finite cross-target artifacts used by build-check preparation tests.
fn build_wasm(args: &[OsString]) -> io::Result<()> {
    let name = match option(args, "--target")?.to_str() {
        Some("wasm32-unknown-unknown") if option(args, "--example")? == "browser_import_check" => {
            "browser_import_check"
        }
        Some("wasm32-wasip2") if args.iter().any(|arg| arg == "--lib") => "hello_extension",
        _ => return Err(io::Error::other("unknown cross-target preparation build")),
    };
    let target = Path::new(option(args, "--target-dir")?);
    std::fs::create_dir_all(target)?;
    let artifact = target.join(format!("{name}.wasm"));
    std::fs::write(&artifact, b"\0asmfixture")?;
    let message = serde_json::json!({
        "reason": "compiler-artifact", "target": {"name": name}, "filenames": [artifact]
    });
    write_stdout(&serde_json::to_vec(&message)?)?;
    write_stdout(b"\n")
}

/// Give the staged example its original permissions and deterministic cache age.
fn executable(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::File::options()
        .write(true)
        .open(path)?
        .set_modified(std::time::UNIX_EPOCH + FIXTURE_MODIFIED_OFFSET)
}
