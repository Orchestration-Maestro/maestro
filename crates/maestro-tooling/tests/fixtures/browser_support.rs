#![cfg(test)]

use super::recipe_support::Workspace;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

pub const FIXTURE: &str = "crates/maestro-tooling/tests/fixtures/browser_author";

pub struct Browser {
    pub workspace: Workspace,
    // Trunk writes binding/optimizer intermediates outside Cargo's build lock.
    _build_lock: fs::File,
}

impl Browser {
    pub fn new() -> Self {
        let lock_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/browser-author-tests.lock");
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(lock_path)
            .unwrap();
        lock.lock().unwrap();
        let mut workspace = Workspace::new();
        let renamed = workspace.0.with_file_name(format!(
            "Maestro's browser 東京 {}",
            workspace.0.file_name().unwrap().to_str().unwrap()
        ));
        fs::rename(&workspace.0, &renamed).unwrap();
        workspace.0 = renamed;
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for item in ["assets".to_owned()].into_iter().chain(
            [
                "Cargo.toml",
                "Cargo.lock",
                "Trunk.toml",
                ".gitignore",
                "index.html",
                "debug.html",
                "package.json.in",
                "build.rs",
                "src",
                "assets",
            ]
            .map(|file| format!("{FIXTURE}/{file}")),
        ) {
            let destination = workspace.0.join(&item);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            assert!(
                Command::new("cp")
                    .args(["-a"])
                    .arg(source.join(&item))
                    .arg(&destination)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let manifest = workspace.0.join(FIXTURE).join("Cargo.toml");
        let text = fs::read_to_string(&manifest).unwrap().replace(
            "../../../../maestro-theme",
            source.join("crates/maestro-theme").to_str().unwrap(),
        );
        fs::write(manifest, text).unwrap();
        Self {
            workspace,
            _build_lock: lock,
        }
    }

    pub fn command(&self, recipe: &str) -> Command {
        let mut command = Command::new("just");
        command
            .current_dir(&self.workspace.0)
            .arg(recipe)
            .env("CARGO_BUILD_JOBS", "3")
            .env(
                "CARGO_TARGET_DIR",
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/browser-author-tests"),
            )
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env_remove("CARGO_LLVM_COV")
            .env_remove("CARGO_LLVM_COV_TARGET_DIR")
            .env_remove("LLVM_PROFILE_FILE")
            .env("TRUNK_SKIP_VERSION_CHECK", "true");
        command
    }

    pub fn run(&self, recipe: &str) -> Output {
        self.command(recipe).output().unwrap()
    }

    pub fn path(&self, path: &str) -> PathBuf {
        self.workspace.0.join(path)
    }

    pub fn fixture(&self, path: &str) -> PathBuf {
        self.path(FIXTURE).join(path)
    }
}

pub fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

impl Browser {
    pub fn workspace_sources(&self) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for name in [
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "clippy.toml",
        ] {
            fs::copy(root.join(name), self.path(name)).unwrap();
        }
        for name in ["vendor", "docs"] {
            std::os::unix::fs::symlink(root.join(name), self.path(name)).unwrap();
        }
        for entry in fs::read_dir(root.join("crates")).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            if name == "maestro-tooling" {
                fs::copy(
                    entry.path().join("Cargo.toml"),
                    self.path("crates/maestro-tooling/Cargo.toml"),
                )
                .unwrap();
                std::os::unix::fs::symlink(
                    entry.path().join("src"),
                    self.path("crates/maestro-tooling/src"),
                )
                .unwrap();
            } else if name == "maestro-web" {
                assert!(
                    Command::new("cp")
                        .arg("-a")
                        .arg(entry.path())
                        .arg(self.path("crates"))
                        .status()
                        .unwrap()
                        .success()
                );
            } else {
                std::os::unix::fs::symlink(entry.path(), self.path("crates").join(name)).unwrap();
            }
        }
    }
}

pub use super::recipe_support::start_output;

pub fn wait_for(receive: &std::sync::mpsc::Receiver<String>, needle: &str) -> String {
    let mut transcript = String::new();
    loop {
        let line = receive.recv().expect(&transcript);
        transcript.push_str(&line);
        transcript.push('\n');
        if line.contains(needle) {
            return transcript;
        }
        assert!(
            !line.contains("error:") && !line.contains("error from build pipeline"),
            "{transcript}"
        );
    }
}

pub fn get(port: u16, path: &str) -> (String, Vec<u8>) {
    use std::io::{Read, Write};
    let mut socket = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(socket, "GET /{path} HTTP/1.0\r\nHost: localhost\r\n\r\n").unwrap();
    let mut bytes = Vec::new();
    socket.read_to_end(&mut bytes).unwrap();
    let end = bytes
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .unwrap();
    (
        String::from_utf8(bytes[..end].to_vec()).unwrap(),
        bytes[end + 4..].to_vec(),
    )
}

pub fn native_tool(browser: &Browser, command: &mut Command, name: &str) -> PathBuf {
    let tool = Command::new("which").arg(name).output().unwrap();
    assert!(tool.status.success());
    let directory = browser.path("native-bin");
    fs::create_dir_all(&directory).unwrap();
    super::recipe_support::install_executable(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/native_tool.sh"),
        &directory.join(name),
    );
    let log = browser.path("native-log");
    command
        .env(
            "PATH",
            format!("{}:{}", directory.display(), std::env::var("PATH").unwrap()),
        )
        .env(
            "MAESTRO_NATIVE_TOOL",
            std::str::from_utf8(&tool.stdout).unwrap().trim(),
        )
        .env("MAESTRO_NATIVE_LOG", &log);
    log
}

pub fn measure(path: &Path) -> (u64, u64) {
    if path.is_dir() {
        return fs::read_dir(path)
            .unwrap()
            .map(|file| measure(&file.unwrap().path()))
            .fold((0, 0), |(raw, gzip), (r, g)| (raw + r, gzip + g));
    }
    let output = Command::new("gzip")
        .args(["-n", "-c", "--"])
        .arg(path)
        .output()
        .unwrap();
    success(&output);
    (
        fs::metadata(path).unwrap().len(),
        u64::try_from(output.stdout.len()).unwrap(),
    )
}
