#![cfg(test)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

pub struct Workspace(pub PathBuf);

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
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "maestro-recipes-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
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
            assert!(
                super::toolchain_support::command("cargo")
                    .args(["build", "--locked", "--manifest-path"])
                    .arg(manifest)
                    .arg("--target-dir")
                    .arg(&target)
                    .status()
                    .unwrap()
                    .success()
            );
            target.join("debug/recipe-command")
        });
        install_executable(tool, &root.join("bin/cargo"));
        for name in ["mise", "prek", "watchexec", "trunk", "miniserve"] {
            install_executable(tool, &root.join("bin").join(name));
        }
        Self(root)
    }

    pub fn command(&self, recipe: &str) -> Command {
        let mut command = super::toolchain_support::command("just");
        command
            .current_dir(&self.0)
            .arg(recipe)
            .env("MAESTRO_LOG", self.0.join("log"))
            .env("MAESTRO_CWD", self.0.join("cwd"))
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.0.join("bin").display(),
                    std::env::var("PATH").unwrap()
                ),
            );
        command
    }

    pub fn run(&self, recipe: &str) -> Output {
        self.command(recipe).output().unwrap()
    }
    pub fn log(&self) -> String {
        fs::read_to_string(self.0.join("log")).unwrap()
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// Owns the recipe's process group, including watcher commands and their children.
#[cfg(unix)]
pub struct ProcessGroup(pub std::process::Child);

#[cfg(unix)]
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        // The fixture disables nested watcher groups so every child inherits this group.
        let _ = Command::new("kill")
            .args(["-KILL", "--", &format!("-{}", self.0.id())])
            .status();
        let _ = self.0.wait();
    }
}

/// Captures complete lines from the reader supplied to the thread.
pub fn capture_lines<R: std::io::Read>(
    reader: impl FnOnce() -> R + Send + 'static,
    send: std::sync::mpsc::Sender<String>,
) -> std::thread::JoinHandle<()> {
    use std::io::{BufRead, BufReader};
    std::thread::spawn(move || {
        for line in BufReader::new(reader()).lines() {
            if send.send(line.unwrap()).is_err() {
                break;
            }
        }
    })
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
    let child = ProcessGroup(command.spawn().unwrap());
    drop(command);
    let (send, receive) = std::sync::mpsc::channel();
    let thread = capture_lines(move || reader, send);
    (child, receive, thread)
}
