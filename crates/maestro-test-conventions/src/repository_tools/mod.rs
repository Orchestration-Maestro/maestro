mod assets;
mod cargo_target;
mod format_staged;
mod isolation;
mod pre_commit;
mod rustdoc;
mod source_launch;

use std::ffi::OsString;
use std::process::{Command, ExitCode};

pub(super) fn main() -> ExitCode {
    match execute(std::env::args_os().skip(1).collect()) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn execute(mut args: Vec<OsString>) -> Result<u8, String> {
    if args.is_empty() {
        return Err("repository tools: expected a command".into());
    }
    let root = if args.first().is_some_and(|arg| arg == "--root") {
        if args.len() < 3 {
            return Err("repository tools: expected repository root and command".into());
        }
        args.remove(0);
        std::path::PathBuf::from(args.remove(0))
    } else {
        std::env::current_dir().map_err(|error| error.to_string())?
    };
    let operation = args.remove(0);
    match operation.to_str() {
        Some("isolate") => {
            let executable = args
                .first()
                .ok_or("repository tools: expected an executable")?;
            let mut command = Command::new(executable);
            command.args(&args[1..]);
            isolation::run(command)
        }
        Some("docs") => rustdoc::docs(),
        Some("inactive") => {
            if args.len() != 2 {
                return Err("repository tools: expected hook and owner".into());
            }
            Err(format!(
                "repository tools: inactive hook: {}; owner: {}",
                args[0].to_string_lossy(),
                args[1].to_string_lossy()
            ))
        }
        Some("copy-assets") => assets::run(args),
        Some("rustdoc") => rustdoc::run(&root, args),
        Some("cargo-target") => cargo_target::run(&root, args),
        Some("cargo-case") => {
            let executable = args
                .first()
                .ok_or("repository tools: expected test case executable")?;
            let mut command = Command::new(executable);
            command.args(&args[1..]);
            cargo_target::runtime_environment(&mut command);
            isolation::run(command)
        }
        Some("format-staged") if args.is_empty() => format_staged::run(),
        Some("pre-commit") if args.is_empty() => pre_commit::run(),
        Some("source") => {
            if args.len() < 2 {
                return Err("repository tools: expected shell and repository root".into());
            }
            let shell = args.remove(0);
            let root = args.remove(0);
            source_launch::run(
                shell.to_str().ok_or("repository tools: invalid shell")?,
                std::path::Path::new(&root),
                args,
            )
        }
        _ => Err("repository tools: unsupported command".into()),
    }
}

fn status_code(status: std::process::ExitStatus) -> u8 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        u8::try_from(
            status
                .code()
                .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)),
        )
        .unwrap_or(1)
    }
    #[cfg(not(unix))]
    {
        status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .unwrap_or(1)
    }
}
