//! The consuming builder that starts one checked test child.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};

use crate::proof::{check_file, check_output};
use crate::{CheckedChild, TestProcessError, selection};

/// Variables a cleared environment keeps: the coverage runtime's profile path and the
/// Windows system root, without which a Windows child cannot load its socket library.
const KEPT: [&str; 2] = ["LLVM_PROFILE_FILE", "SYSTEMROOT"];

/// One ordered environment edit.
#[derive(Debug)]
enum EnvEdit {
    /// Drop the inherited environment, keeping the [`KEPT`] variables.
    Clear,
    /// Set a variable.
    Set(OsString, OsString),
    /// Remove a variable.
    Remove(OsString),
}

impl EnvEdit {
    /// Apply the edit to a command.
    fn apply(&self, command: &mut Command) {
        match self {
            Self::Clear => {
                command.env_clear();
                let kept = KEPT
                    .iter()
                    .filter_map(|name| Some((name, std::env::var_os(name)?)));
                command.envs(kept);
            }
            Self::Set(key, value) => {
                command.env(key, value);
            }
            Self::Remove(key) => {
                command.env_remove(key);
            }
        }
    }
}

/// A harness presentation or selection flag, in command-line order.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Flag {
    /// Print test output as it happens.
    Nocapture,
    /// Run on one test thread.
    SingleThread,
    /// Terse test lines.
    Quiet,
    /// Run only ignored tests.
    Ignored,
}

impl Flag {
    /// The command-line spelling of the flag.
    fn argument(&self) -> &'static str {
        match self {
            Self::Nocapture => "--nocapture",
            Self::SingleThread => "--test-threads=1",
            Self::Quiet => "--quiet",
            Self::Ignored => "--ignored",
        }
    }
}

/// A checked launch of one named test in a fresh process of the current test executable.
///
/// Launch operations resolve the exact identity and confirm eligibility. Checked-output
/// operations also require the child's one-test start prologue; [`TestProcess::invoke_with`]
/// leaves startup proof with its caller. The test need not pass. Environment edits apply
/// in call order without changing this process.
#[derive(Debug)]
pub struct TestProcess {
    /// Full libtest identity.
    name: String,
    /// Environment edits in call order.
    envs: Vec<EnvEdit>,
    /// Working directory of the child.
    dir: Option<PathBuf>,
    /// Caller-chosen standard input.
    stdin: Option<Stdio>,
    /// Caller-chosen standard output.
    stdout: Option<Stdio>,
    /// Caller-chosen standard error.
    stderr: Option<Stdio>,
    /// Harness flags.
    flags: BTreeSet<Flag>,
    /// Literal `--skip` operands in call order.
    skips: Vec<String>,
    /// Caller-supplied setup interpreter and the shell text it runs before the test
    /// executable replaces it.
    shell: Option<(PathBuf, String)>,
}

impl TestProcess {
    /// Select one case identifier in the child, or `"*"` for all cases.
    #[must_use]
    pub fn case(self, case: impl AsRef<OsStr>) -> Self {
        self.env(crate::environment::CASE, case)
    }

    /// Select the running test: `test` is its full identity or its leaf at a `::` boundary.
    ///
    /// # Errors
    ///
    /// Fails when the thread is unnamed or `test` is not the running test.
    pub fn current(test: &str) -> Result<Self, TestProcessError> {
        Self::named(selection::current_name(test)?)
    }

    /// Select an ignored child test by its function item, printing its own output.
    ///
    /// # Errors
    ///
    /// Fails when the item has no module-qualified path.
    pub fn child<F: Fn()>(_test: F) -> Result<Self, TestProcessError> {
        Err(TestProcessError::new(
            "child",
            "the child route is not implemented",
        ))
    }

    /// Select a test by its complete libtest identity, such as a sibling's.
    ///
    /// # Errors
    ///
    /// Fails when the identity is empty.
    pub fn named(test: impl Into<String>) -> Result<Self, TestProcessError> {
        let name = test.into();
        if name.is_empty() {
            return Err(TestProcessError::new(&name, "the test identity is empty"));
        }
        Ok(Self {
            name,
            envs: Vec::new(),
            dir: None,
            stdin: None,
            stdout: None,
            stderr: None,
            flags: BTreeSet::new(),
            skips: Vec::new(),
            shell: None,
        })
    }

    /// The full libtest identity this launch selects.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Set one environment variable.
    #[must_use]
    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.envs
            .push(EnvEdit::Set(key.as_ref().into(), value.as_ref().into()));
        self
    }

    /// Set several environment variables in iteration order.
    #[must_use]
    pub fn envs<I, K, V>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        for (key, value) in values {
            self = self.env(key, value);
        }
        self
    }

    /// Remove one environment variable.
    #[must_use]
    pub fn env_remove(mut self, key: impl AsRef<OsStr>) -> Self {
        self.envs.push(EnvEdit::Remove(key.as_ref().into()));
        self
    }

    /// Drop the inherited environment, keeping the coverage profile path and the
    /// Windows system root.
    #[must_use]
    pub fn env_clear(mut self) -> Self {
        self.envs.push(EnvEdit::Clear);
        self
    }

    /// Point the child's temporary directory at `path` through the variables
    /// `std::env::temp_dir` reads: `TMPDIR` on Unix, `TMP` and `TEMP` on Windows.
    #[must_use]
    pub fn temp_dir(self, path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        if cfg!(windows) {
            self.env("TMP", path).env("TEMP", path)
        } else {
            self.env("TMPDIR", path)
        }
    }

    /// Set the child's working directory.
    #[must_use]
    pub fn current_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.dir = Some(path.as_ref().to_owned());
        self
    }

    /// Set the child's standard input.
    #[must_use]
    pub fn stdin(mut self, stream: impl Into<Stdio>) -> Self {
        self.stdin = Some(stream.into());
        self
    }

    /// Set the child's standard output; a checked read needs it piped or captured.
    #[must_use]
    pub fn stdout(mut self, stream: impl Into<Stdio>) -> Self {
        self.stdout = Some(stream.into());
        self
    }

    /// Set the child's standard error.
    #[must_use]
    pub fn stderr(mut self, stream: impl Into<Stdio>) -> Self {
        self.stderr = Some(stream.into());
        self
    }

    /// Print the test's own output as it happens.
    #[must_use]
    pub fn nocapture(self) -> Self {
        self.flag(Flag::Nocapture)
    }

    /// Use the harness's terse presentation.
    #[must_use]
    pub fn quiet(self) -> Self {
        self.flag(Flag::Quiet)
    }

    /// Run the harness on one test thread.
    #[must_use]
    pub fn single_threaded(self) -> Self {
        self.flag(Flag::SingleThread)
    }

    /// Run the selected ignored test; an ordinary test is rejected in this mode.
    #[must_use]
    pub fn ignored(self) -> Self {
        self.flag(Flag::Ignored)
    }

    /// Add one literal `--skip` operand; operands keep order and duplicates.
    ///
    /// With the exact filter, only an operand equal to the full identity excludes the test.
    #[must_use]
    pub fn skip(mut self, operand: impl Into<String>) -> Self {
        self.skips.push(operand.into());
        self
    }

    /// Run `setup` in `program` before the test executable replaces the shell.
    ///
    /// `program` names the setup interpreter, so the caller decides which shell runs; an
    /// absolute path is required and the process `PATH` is never searched. The executable
    /// and arguments are passed as shell positional parameters, never interpolated into
    /// the shell text.
    #[cfg(unix)]
    #[must_use]
    pub fn shell_setup(mut self, program: impl AsRef<Path>, setup: impl Into<String>) -> Self {
        self.shell = Some((program.as_ref().to_path_buf(), setup.into()));
        self
    }

    /// Run to completion with captured streams and return them unchanged.
    ///
    /// The standard input is null unless set. The returned status is the child's, so a
    /// failed or signalled body is still reported to the caller.
    ///
    /// # Errors
    ///
    /// Fails when the test is not eligible, the child cannot run, or its output does not
    /// begin with the start prologue.
    pub fn output(mut self) -> Result<Output, TestProcessError> {
        let output = self.run_captured()?;
        check_output(&self.name, &output)?;
        Ok(output)
    }

    /// Run to completion, then replay the captured standard output and standard error
    /// separately to this process's own streams. The standard input is inherited unless set.
    ///
    /// Not a live route: use [`TestProcess::spawn`] when the parent must interact.
    ///
    /// # Errors
    ///
    /// As [`TestProcess::output`], and when the replay cannot be written.
    pub fn status(mut self) -> Result<ExitStatus, TestProcessError> {
        self.stdin.get_or_insert_with(Stdio::inherit);
        let output = self.run_captured()?;
        check_output(&self.name, &output)?;
        let replay = std::io::stdout()
            .write_all(&output.stdout)
            .and_then(|()| std::io::stderr().write_all(&output.stderr));
        replay.map_err(|error| {
            self.failure("cannot replay the child's streams")
                .caused_by(error)
        })?;
        Ok(output.status)
    }

    /// Run to completion with standard output sent to `file`, then prove the start through
    /// the same open file, leaving its shared cursor where the child left it. The file
    /// must be empty with its cursor at zero, so existing bytes cannot prove startup.
    ///
    /// The standard input is inherited unless set.
    ///
    /// # Errors
    ///
    /// As [`TestProcess::output`], and when the file cannot be shared or inspected.
    pub fn status_with_stdout(mut self, file: File) -> Result<ExitStatus, TestProcessError> {
        let mut inspect = file.try_clone().map_err(|error| {
            self.failure("cannot share the standard output file")
                .caused_by(error)
        })?;
        let position = inspect.stream_position().map_err(|error| {
            self.failure("cannot inspect the standard output file")
                .caused_by(error)
        })?;
        let length = inspect
            .metadata()
            .map_err(|error| {
                self.failure("cannot inspect the standard output file")
                    .caused_by(error)
            })?
            .len();
        if position != 0 || length != 0 {
            return Err(self.failure("standard output file must be empty with its cursor at zero"));
        }
        self.stdout = Some(file.into());
        let status = self.prepare()?.status().map_err(|error| {
            self.failure("cannot run the selected test")
                .caused_by(error)
        })?;
        check_file(&self.name, &inspect, status)?;
        Ok(status)
    }

    /// Start the child with standard output piped, read only the start prologue and return
    /// the live child with its prologue replayed first.
    ///
    /// The other descriptors are the inherited defaults unless set. A child that fails the
    /// proof is closed and reaped.
    ///
    /// # Errors
    ///
    /// Fails when the test is not eligible, the child cannot start, or it does not begin
    /// with the start prologue.
    pub fn spawn(mut self) -> Result<CheckedChild, TestProcessError> {
        self.stdout.get_or_insert_with(Stdio::piped);
        let child = self.prepare()?.spawn().map_err(|error| {
            self.failure("cannot start the selected test")
                .caused_by(error)
        })?;
        CheckedChild::start(&self.name, child)
    }

    /// Hand the configured command to a caller that starts the child itself.
    ///
    /// The command re-runs the selected test with this builder's environment, directory,
    /// descriptors and harness flags, for production code that spawns its own children. This
    /// checks selection, not startup: the child's standard output begins with
    /// [`crate::HARNESS_START`], which the caller can look for before claiming the test ran.
    ///
    /// # Errors
    ///
    /// Fails when the test is not eligible.
    pub fn command(mut self) -> Result<Command, TestProcessError> {
        self.prepare()
    }

    /// Run through the caller's own invoker and prove the returned output started the test.
    ///
    /// `invoke` receives the final ordered harness arguments and runs the test executable
    /// itself, for production code under test. Environment, directory, descriptor and shell
    /// configuration cannot reach that invoker, so setting any of them is rejected before
    /// `invoke` runs.
    ///
    /// # Errors
    ///
    /// Fails when configuration was set, the test is not eligible, `invoke` fails, or its
    /// output does not begin with the start prologue.
    pub fn output_with<F, E>(self, invoke: F) -> Result<Output, TestProcessError>
    where
        F: FnOnce(&[String]) -> Result<Output, E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let name = self.name.clone();
        let output = self.invoke_with(invoke)?;
        check_output(&name, &output)?;
        Ok(output)
    }

    /// Supplies eligible exact harness arguments to a caller-owned invocation.
    ///
    /// This checks selection, not startup. The caller must observe the selected
    /// test's one-test prologue through its own transport before claiming startup.
    /// Environment, directory, descriptor and shell settings are rejected.
    ///
    /// # Errors
    /// Returns selection, unsupported configuration or caller invocation failures.
    pub fn invoke_with<T, F, E>(self, invoke: F) -> Result<T, TestProcessError>
    where
        F: FnOnce(&[String]) -> Result<T, E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let plain = self.envs.is_empty()
            && self.dir.is_none()
            && self.shell.is_none()
            && self.stdin.is_none()
            && self.stdout.is_none()
            && self.stderr.is_none();
        if !plain {
            return Err(self.failure(
                "the adapter route accepts no environment, directory, descriptor or shell setting",
            ));
        }
        self.eligible()?;
        invoke(&self.arguments()).map_err(|error| {
            self.failure("the adapter invocation failed")
                .caused_by(error)
        })
    }

    /// Add a harness flag.
    fn flag(mut self, flag: Flag) -> Self {
        self.flags.insert(flag);
        self
    }

    /// Describe a failed step of this launch.
    fn failure(&self, message: &str) -> TestProcessError {
        TestProcessError::new(&self.name, message)
    }

    /// The harness arguments in their final order.
    fn arguments(&self) -> Vec<String> {
        let mut arguments = vec!["--exact".to_owned(), self.name.clone()];
        arguments.extend(self.flags.iter().map(|flag| flag.argument().to_owned()));
        for operand in &self.skips {
            arguments.extend(["--skip".to_owned(), operand.clone()]);
        }
        arguments
    }

    /// Require the selected test to be listed and eligible for the chosen mode.
    fn eligible(&self) -> Result<PathBuf, TestProcessError> {
        let executable = std::env::current_exe().map_err(|error| {
            self.failure("cannot resolve the test executable")
                .caused_by(error)
        })?;
        selection::require_eligible(&executable, &self.name, self.flags.contains(&Flag::Ignored))?;
        Ok(executable)
    }

    /// Build the configured command once the test is known to be eligible.
    fn prepare(&mut self) -> Result<Command, TestProcessError> {
        let executable = self.eligible()?;
        let mut command = self.base_command(executable)?;
        command.args(self.arguments());
        self.envs.iter().for_each(|edit| edit.apply(&mut command));
        if let Some(dir) = &self.dir {
            command.current_dir(dir);
        }
        if let Some(stream) = self.stdin.take() {
            command.stdin(stream);
        }
        if let Some(stream) = self.stdout.take() {
            command.stdout(stream);
        }
        if let Some(stream) = self.stderr.take() {
            command.stderr(stream);
        }
        Ok(command)
    }

    /// Run the prepared command with captured streams.
    fn run_captured(&mut self) -> Result<Output, TestProcessError> {
        self.prepare()?.output().map_err(|error| {
            self.failure("cannot run the selected test")
                .caused_by(error)
        })
    }

    /// The command that runs the selected test, through the supplied setup interpreter
    /// when one is set.
    ///
    /// The interpreter path the caller supplied is used as it stands, so neither this
    /// process's `PATH` nor the child's decides which shell runs. A path that cannot run
    /// fails at launch with the operating system's error.
    fn base_command(&mut self, executable: PathBuf) -> Result<Command, TestProcessError> {
        if let Some((program, setup)) = self.shell.take() {
            if !program.is_absolute() {
                return Err(self.failure("the setup interpreter path is not absolute"));
            }
            let mut shell = Command::new(program);
            shell.args(["-c", &format!("{setup}; exec \"$@\""), "startup"]);
            shell.arg(executable);
            return Ok(shell);
        }
        Ok(Command::new(executable))
    }
}
