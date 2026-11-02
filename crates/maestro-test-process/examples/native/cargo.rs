//! Cargo boundary used by repository preparation's existing shared fixture.

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use maestro_test_process::child::{leave, write_stdout};

/// Inputs of the preparation fixture, supplied beside its Cargo alias.
pub(super) struct Record {
    /// Real Cargo used only for workspace metadata.
    cargo: String,
    /// Existing setup-example source copied for author-component builds.
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

/// The two example builds this fixture supports.
enum Example {
    /// The shared setup example whose retained source the caller supplies.
    Author,
    /// The single raw native fixture itself.
    Native,
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
            _ => return Err(io::Error::other("unknown preparation example")),
        };
        Ok(Self {
            target: option(args, "--target-dir")?.into(),
            example,
        })
    }
}

/// Forward real metadata unchanged; stage the two finite example targets for other calls.
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
        Some("build" | "run") => build(record, &Build::parse(args)?, args[0] == "run"),
        _ => Err(io::Error::other("unknown fixture Cargo operation")),
    }
}

/// Stage an existing fixture, counting author and native builds independently.
fn build(record: &Record, build: &Build, start: bool) -> io::Result<()> {
    let (name, source, log) = match build.example {
        Example::Author => (
            "prepare_author_component",
            PathBuf::from(&record.template),
            "builds",
        ),
        Example::Native => ("native_fixture", std::env::current_exe()?, "native-builds"),
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
    writeln!(
        std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(log)?,
        "build"
    )?;
    if matches!(build.example, Example::Native) {
        let message = serde_json::json!({"reason": "compiler-artifact", "target": {"name": "native_fixture"}, "executable": path});
        write_stdout(&serde_json::to_vec(&message)?)?;
        write_stdout(b"\n")?;
    }
    if start {
        leave(Command::new(path).status()?.code().unwrap_or(1));
    }
    Ok(())
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
        .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_hours(438_288))
}
