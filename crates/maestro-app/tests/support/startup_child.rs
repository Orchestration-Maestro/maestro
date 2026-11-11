//! One startup conversion in a process of its own, without libtest arguments or stream framing.
//!
//! The first argument names the operation, `auth` or `sessions`. `--file-limit-kib N` runs it
//! under a file-size limit that is lifted before the outcome is reported, and
//! `--exit-write PATH` then writes 4096 bytes to `PATH`, as the exit-time profile write of an
//! instrumented process does. The outcome is the one `PROBE` line on standard output.

#[cfg(not(unix))]
fn main() {}

#[cfg(unix)]
fn main() -> std::io::Result<()> {
    unix::run()
}

/// The conversion and its report, which need a Unix file-size limit.
#[cfg(unix)]
mod unix {
    use maestro_app::configuration::conversion::{
        migrate_auth_to_auth_json, migrate_sessions_from_agent_root,
    };
    use maestro_test_process::IsolatedSetup;
    use rustix::process::{Resource, Rlimit, getrlimit, setrlimit};
    use serde::Serialize;
    use std::{ffi::OsString, io, path::PathBuf};

    /// The conversion a launch selects.
    #[derive(Clone, Copy)]
    enum Operation {
        /// The credential conversion into `auth.json`.
        Auth,
        /// The session move out of the agent root.
        Sessions,
    }

    /// What one launch asks for.
    struct Request {
        /// The conversion to run.
        operation: Operation,
        /// The file-size limit, in kibibytes, the conversion runs under.
        file_limit_kib: Option<u64>,
        /// Where to write 4096 bytes once the limit is lifted.
        exit_write: Option<PathBuf>,
    }

    /// The failure category the corpus names.
    #[derive(Serialize)]
    struct Failure {
        /// The error's name, such as `ENOENT`.
        code: &'static str,
    }

    /// What one launch reports.
    #[derive(Serialize)]
    struct Outcome {
        /// The providers the credential conversion read, in order.
        providers: Option<Vec<String>>,
        /// The failure, when the conversion failed.
        error: Option<Failure>,
    }

    /// A launch the child refuses.
    fn refusal(text: impl Into<String>) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidInput, text.into())
    }

    /// Read the launch's arguments.
    fn parse(arguments: impl IntoIterator<Item = OsString>) -> io::Result<Request> {
        let mut arguments = arguments.into_iter().map(OsString::into_string);
        let operation = match arguments.next().transpose() {
            Ok(Some(name)) if name == "auth" => Operation::Auth,
            Ok(Some(name)) if name == "sessions" => Operation::Sessions,
            _ => return Err(refusal("the first argument is `auth` or `sessions`")),
        };
        let mut request = Request {
            operation,
            file_limit_kib: None,
            exit_write: None,
        };
        while let Some(flag) = arguments.next() {
            let value = arguments
                .next()
                .and_then(Result::ok)
                .ok_or_else(|| refusal("a flag needs a text value"))?;
            match flag.as_deref() {
                Ok("--file-limit-kib") => {
                    request.file_limit_kib = Some(value.parse().map_err(|_| refusal(&value))?);
                }
                Ok("--exit-write") => request.exit_write = Some(PathBuf::from(value)),
                _ => return Err(refusal("unknown flag")),
            }
        }
        Ok(request)
    }

    /// Run the selected conversion, under the file-size limit when the launch names one.
    fn convert(request: &Request) -> io::Result<Option<Vec<String>>> {
        let previous = request
            .file_limit_kib
            .map(|limit| {
                let previous = getrlimit(Resource::Fsize);
                setrlimit(
                    Resource::Fsize,
                    Rlimit {
                        current: Some(limit * 1024),
                        ..previous
                    },
                )
                .map(|()| previous)
            })
            .transpose()?;
        let result = match request.operation {
            Operation::Auth => migrate_auth_to_auth_json().map(Some),
            Operation::Sessions => migrate_sessions_from_agent_root().map(|()| None),
        };
        // Lift the limit before anything reports, so exit-time writes are not refused.
        if let Some(previous) = previous {
            setrlimit(Resource::Fsize, previous)?;
        }
        result
    }

    /// The category the corpus names for a native error.
    fn category(error: &io::Error) -> io::Result<&'static str> {
        match error.raw_os_error() {
            Some(2) => Ok("ENOENT"),
            Some(13) => Ok("EACCES"),
            Some(27) => Ok("EFBIG"),
            _ => Err(io::Error::other(format!(
                "unexpected native error: {error}"
            ))),
        }
    }

    /// Apply the launch's isolation, run the conversion and print its outcome.
    pub(super) fn run() -> io::Result<()> {
        let _setup = IsolatedSetup::apply_requested().map_err(io::Error::other)?;
        let request = parse(std::env::args_os().skip(1))?;
        let outcome = match convert(&request) {
            Ok(providers) => Outcome {
                providers,
                error: None,
            },
            Err(error) => Outcome {
                providers: None,
                error: Some(Failure {
                    code: category(&error)?,
                }),
            },
        };
        if let Some(path) = &request.exit_write {
            std::fs::write(path, [0_u8; 4096])?;
        }
        println!(
            "PROBE {}",
            serde_json::to_string(&outcome).map_err(io::Error::other)?
        );
        Ok(())
    }
}
