#![cfg(test)]

use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

pub struct Workspace {
    parent: maestro_test_directory::Scratch,
    name: OsString,
}

/// Write one fixture input below its owning directory.
pub fn write_input(root: &Path, name: &str, bytes: &str) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// Copies an executable through a child `install`, so this process never holds a
/// writable descriptor to a file it later runs (a concurrent fork would otherwise
/// make `exec` fail with "Text file busy").
pub fn install_executable(source: &Path, target: &Path) {
    let status = Command::new("install")
        .args(["-m", "0700"])
        .arg(source)
        .arg(target)
        .status()
        .unwrap();
    assert!(status.success());
}

impl Workspace {
    pub fn new() -> Self {
        static TOOL: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        record("phase: workspace setup");
        let workspace = Self {
            parent: maestro_test_directory::Scratch::new().unwrap(),
            name: OsString::from("checkout"),
        };
        let root = workspace.path();
        fs::create_dir_all(root.join("bin")).unwrap();
        super::toolchain_support::declare_toolchain(
            &root,
            include_str!("../../../../rust-toolchain.toml"),
        )
        .unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../justfile"),
            root.join("justfile"),
        )
        .unwrap();
        let tool = TOOL.get_or_init(|| {
            let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/recipe_tools/Cargo.toml");
            let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/recipe-tools");
            record("phase: recipe tool cargo build");
            let status = setup_status(
                super::toolchain_support::command("cargo")
                    .args(["build", "--locked", "--manifest-path"])
                    .arg(manifest)
                    .arg("--target-dir")
                    .arg(&target),
            );
            record(format!("phase: recipe tool cargo complete {status}"));
            assert!(status.success());
            target.join("debug/recipe-command")
        });
        install_executable(tool, &root.join("bin/cargo"));
        for name in ["mise", "prek", "watchexec", "trunk", "miniserve"] {
            install_executable(tool, &root.join("bin").join(name));
        }
        workspace
    }

    /// The currently selected checkout below the retained disposable parent.
    pub fn path(&self) -> PathBuf {
        self.parent.path().join(&self.name)
    }

    /// Move the checkout, retaining its old selection when the rename fails.
    pub fn rename(&mut self, name: &OsStr) -> std::io::Result<()> {
        fs::rename(self.path(), self.parent.path().join(name))?;
        self.name = name.to_owned();
        Ok(())
    }

    /// Keep a persistent copy at an absolute path while the original remains disposable.
    #[cfg(unix)]
    pub fn cache_at(&mut self, path: &Path) {
        assert!(path.is_absolute());
        if path.exists() {
            fs::remove_dir_all(path).unwrap();
        }
        assert!(
            Command::new("cp")
                .arg("-a")
                .arg(self.path())
                .arg(path)
                .status()
                .unwrap()
                .success()
        );
        self.name = path.as_os_str().to_owned();
    }

    /// Keeps build output inside the fixture's configured target directory.
    pub fn toolchain_command(&self, program: impl AsRef<OsStr>) -> Command {
        let mut command = super::toolchain_support::command(program);
        command
            .current_dir(self.path())
            .env_remove("CARGO_TARGET_DIR");
        command
    }

    pub fn command(&self, recipe: &str) -> Command {
        let mut command = self.toolchain_command("just");
        command
            .arg(recipe)
            .env("MAESTRO_LOG", self.path().join("log"))
            .env("MAESTRO_CWD", self.path().join("cwd"))
            .env("PATH", {
                // The fixture tools come first, joined with the host's list separator.
                let inherited = std::env::var_os("PATH").unwrap();
                let entries = std::iter::once(self.path().join("bin"))
                    .chain(std::env::split_paths(&inherited));
                std::env::join_paths(entries).unwrap()
            });
        command
    }

    pub fn run(&self, recipe: &str) -> Output {
        self.command(recipe).output().unwrap()
    }
    pub fn log(&self) -> String {
        fs::read_to_string(self.path().join("log")).unwrap()
    }
}

/// Owns the recipe's process group, including watcher commands and their children.
#[cfg(unix)]
pub struct ProcessGroup(pub std::process::Child);

#[cfg(unix)]
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        record(format!("phase: terminate group {}", self.0.id()));
        // The fixture disables nested watcher groups so every child inherits this group.
        let _ = Command::new("kill")
            .args(["-KILL", "--", &format!("-{}", self.0.id())])
            .status();
        record(format!("phase: child wait {}", self.0.id()));
        let status = self.0.wait();
        record(format!("phase: child wait result {status:?}"));
    }
}

/// Open this test attempt's transcript outside disposable fixture directories.
pub fn transcript() -> fs::File {
    let thread = std::thread::current();
    let name = thread.name().unwrap_or("unnamed").replace("::", "_");
    let directory = std::env::var_os("MAESTRO_TEST_DIAGNOSTICS_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/browser-author-diagnostics"),
        PathBuf::from,
    );
    fs::create_dir_all(&directory).unwrap();
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join(format!("{}-{name}.log", std::process::id())))
        .unwrap()
}

/// Write a phase or child-output record before the test can block again.
pub fn record(message: impl std::fmt::Display) {
    use std::io::Write;
    let mut file = transcript();
    file.write_all(format!("{message}\n").as_bytes()).unwrap();
}

/// Join a capture reader between phase records, so a blocked join is visible.
#[cfg(unix)]
pub fn join_reader(thread: std::thread::JoinHandle<()>) {
    record("phase: reader join");
    thread.join().unwrap();
    record("phase: reader joined");
}

/// Write child lines to the transcript before echoing to stderr and forwarding to the consumer.
pub fn capture_lines<R: std::io::Read>(
    reader: impl FnOnce() -> R + Send + 'static,
    send: std::sync::mpsc::Sender<String>,
) -> std::thread::JoinHandle<()> {
    use std::io::{BufRead, BufReader};
    std::thread::Builder::new()
        .name(
            std::thread::current()
                .name()
                .unwrap_or("unnamed")
                .to_owned(),
        )
        .spawn(move || {
            for line in BufReader::new(reader()).lines() {
                let line = line.unwrap();
                record(&line);
                eprintln!("{line}");
                if send.send(line).is_err() {
                    break;
                }
            }
            record("phase: reader finished");
        })
        .unwrap()
}

/// Run setup with both streams delivered through the shared capture owner.
pub fn setup_status(command: &mut Command) -> std::process::ExitStatus {
    use std::process::Stdio;
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    let readers = [
        capture_lines(move || stdout, send.clone()),
        capture_lines(move || stderr, send),
    ];
    for _ in receive {}
    for reader in readers {
        reader.join().unwrap();
    }
    child.wait().unwrap()
}

#[cfg(unix)]
pub fn start_output(
    mut command: Command,
) -> (
    ProcessGroup,
    std::sync::mpsc::Receiver<String>,
    std::thread::JoinHandle<()>,
) {
    use std::os::unix::process::CommandExt;
    let (reader, writer) = std::os::unix::net::UnixStream::pair().unwrap();
    let stdout: std::os::fd::OwnedFd = writer.try_clone().unwrap().into();
    let stderr: std::os::fd::OwnedFd = writer.into();
    command.stdout(stdout).stderr(stderr).process_group(0);
    record(format!(
        "phase: spawn {:?} {:?}; cwd {:?}",
        command.get_program(),
        command.get_args().collect::<Vec<_>>(),
        command.get_current_dir()
    ));
    let child = ProcessGroup(command.spawn().unwrap());
    record(format!("phase: spawned pid/group {}", child.0.id()));
    drop(command);
    let (send, receive) = std::sync::mpsc::channel();
    let thread = capture_lines(move || reader, send);
    (child, receive, thread)
}
