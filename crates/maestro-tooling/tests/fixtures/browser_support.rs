#![cfg(test)]

use super::recipe_support::{Workspace, record};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

pub const FIXTURE: &str = "crates/maestro-tooling/tests/fixtures/browser_author";

pub struct Browser {
    pub workspace: Workspace,
    // Trunk writes binding/optimizer intermediates outside Cargo's build lock.
    _build_lock: Option<fs::File>,
}

impl Browser {
    pub fn new() -> Self {
        record("phase: browser setup / lock open");
        let lock_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/browser-author-tests.lock");
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(lock_path)
            .unwrap();
        record("phase: browser lock waiting");
        lock.lock().unwrap();
        record("phase: browser lock acquired");
        Self::prepare(Some(lock))
    }

    /// Prepare isolated inputs without locking the shared Trunk build output.
    /// Every build command must supply its own Cargo target directory.
    pub fn cold() -> Self {
        Self::prepare(None)
    }

    /// Copy authored inputs while retaining any shared-output lock.
    fn prepare(lock: Option<fs::File>) -> Self {
        record("phase: browser fixture copy");
        let mut workspace = Workspace::new();
        let renamed = workspace.path().with_file_name(format!(
            "Maestro's browser 東京 {}",
            workspace.path().file_name().unwrap().to_str().unwrap()
        ));
        workspace.rename(renamed.file_name().unwrap()).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for item in ["assets".to_owned()].into_iter().chain(
            [
                "Cargo.toml",
                "Cargo.lock",
                "Trunk.json",
                ".gitignore",
                "index.html",
                "debug.html",
                "package.json.in",
                "src",
                "assets",
            ]
            .map(|file| format!("{FIXTURE}/{file}")),
        ) {
            let destination = workspace.path().join(&item);
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
        let browser = Self {
            workspace,
            _build_lock: lock,
        };
        record("phase: workspace source preparation");
        browser.workspace_sources();
        record("phase: browser setup complete");
        browser
    }

    /// Recreate inputs at a locked cache path so Cargo retains workspace package identities.
    pub fn cached() -> Self {
        let mut browser = Self::new();
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/Maestro's browser 東京 checkout");
        let sources = [
            format!("{FIXTURE}/src/main.rs"),
            "crates/maestro-web/src/lib.rs".to_owned(),
        ];
        let modified = sources.each_ref().map(|source| {
            let previous = path.join(source);
            let restored = fs::read(browser.path(source)).unwrap();
            match fs::read(&previous) {
                Ok(bytes) if bytes == restored => {
                    fs::metadata(previous).unwrap().modified().unwrap()
                }
                _ => std::time::SystemTime::now(),
            }
        });
        browser.workspace.cache_at(&path);
        // Changed bytes invalidate compiled edits; identical bytes keep the warm fingerprint.
        for (source, modified) in sources.into_iter().zip(modified) {
            fs::File::open(browser.path(&source))
                .unwrap()
                .set_modified(modified)
                .unwrap();
        }
        browser
    }

    pub fn generate_lockfile(&self) {
        record("phase: cargo generate-lockfile");
        let status = super::recipe_support::setup_status(
            super::toolchain_support::command("cargo")
                .args(["generate-lockfile", "--offline", "--manifest-path"])
                .arg(self.fixture("Cargo.toml")),
        );
        record(format!("phase: cargo generate-lockfile complete {status}"));
        assert!(status.success());
    }

    pub fn command(&self, recipe: &str) -> Command {
        let mut command = super::toolchain_support::command("just");
        command.arg(recipe);
        self.configure(&mut command);
        command
    }

    /// Execute the built native helper without instrumenting its compiler children.
    pub fn native_command(&self, operation: &str) -> Command {
        let mut command = super::toolchain_support::command(env!("CARGO_BIN_EXE_development"));
        command
            .args(["browser-author", operation])
            .env("CARGO_MANIFEST_DIR", self.path("crates/maestro-tooling"));
        self.configure(&mut command);
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        command
    }

    /// Share output ownership and native tool settings between recipe and helper calls.
    fn configure(&self, command: &mut Command) {
        command
            .current_dir(self.workspace.path())
            .env("CARGO_BUILD_JOBS", "3")
            .env(
                "CARGO_TARGET_DIR",
                Path::new(env!("CARGO_BIN_EXE_development"))
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap(),
            )
            .env("TRUNK_SKIP_VERSION_CHECK", "true")
            // Trunk's completion emoji depends on LANG, even with piped output.
            .env("LANG", "C.UTF-8");
    }

    pub fn run(&self, recipe: &str) -> Output {
        self.command(recipe).output().unwrap()
    }

    pub fn path(&self, path: &str) -> PathBuf {
        self.workspace.path().join(path)
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
        if self.path("Cargo.toml").exists() {
            return;
        }
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
    record(format!("phase: waiting for {needle:?}"));
    let mut transcript = String::new();
    loop {
        let line = receive.recv().expect(&transcript);
        transcript.push_str(&line);
        transcript.push('\n');
        if line.contains(needle) {
            record(format!("phase: observed {needle:?}"));
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
    record(format!("phase: HTTP request 127.0.0.1:{port}/{path}"));
    let mut socket = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(socket, "GET /{path} HTTP/1.0\r\nHost: localhost\r\n\r\n").unwrap();
    let mut bytes = Vec::new();
    record("phase: HTTP waiting for EOF");
    socket.read_to_end(&mut bytes).unwrap();
    record(format!("phase: HTTP complete {} bytes", bytes.len()));
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

/// Capture authored inputs without including generated outputs or build caches.
pub fn authored(browser: &Browser) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut result = std::collections::BTreeMap::new();
    for path in [
        browser.path("Cargo.toml"),
        browser.path("Cargo.lock"),
        browser.path("assets"),
        browser.path(FIXTURE),
    ] {
        collect(&path, &mut result);
    }
    let home = std::env::var_os("CARGO_HOME").map_or_else(
        || PathBuf::from(std::env::var_os("HOME").unwrap()).join(".cargo"),
        PathBuf::from,
    );
    for file in ["config", "config.toml"] {
        if home.join(file).is_file() {
            collect(&home.join(file), &mut result);
        }
    }
    result
}

/// Observe final fixture bytes, not compiler configuration.
pub fn wasm(browser: &Browser, route: &str) -> Vec<u8> {
    let path = fs::read_dir(browser.path(&format!("target/maestro-browser/{route}")))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "wasm"))
        .unwrap();
    fs::read(path).unwrap()
}

/// Collect file bytes while excluding native output directories.
fn collect(path: &Path, result: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
    if path.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if !["target", "dist", ".trunk"]
                .iter()
                .any(|name| path.ends_with(name))
            {
                collect(&path, result);
            }
        }
    } else {
        result.insert(path.to_path_buf(), fs::read(path).unwrap());
    }
}

/// Report locked-source staging time before creating initially empty build directories.
pub fn cold_sources(browser: &Browser) -> (PathBuf, PathBuf) {
    let started = std::time::Instant::now();
    let sources = browser.path("local-sources");
    record("phase: cargo vendor cold sources");
    let mut child = super::toolchain_support::command(env!("CARGO"))
        .current_dir(browser.workspace.path())
        .args(["vendor", "--locked", "--offline", "--sync"])
        .arg(browser.fixture("Cargo.toml"))
        .arg(&sources)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    let reader = super::recipe_support::capture_lines(move || stderr, send);
    let staged = child.wait_with_output().unwrap();
    reader.join().unwrap();
    drop(receive);
    record(format!(
        "phase: cargo vendor complete {} in {:?}",
        staged.status,
        started.elapsed()
    ));
    success(&staged);
    let home = browser.path("cold-home");
    let target = browser.path("cold-target");
    fs::create_dir(&home).unwrap();
    fs::create_dir(&target).unwrap();
    assert_eq!(fs::read_dir(&home).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    fs::write(
        home.join("config.toml"),
        format!(
            "{}\n[net]\noffline = true\n",
            String::from_utf8(staged.stdout).unwrap()
        ),
    )
    .unwrap();
    (home, target)
}

/// Wait for the native rejected pipeline, which may leave its watcher running.
pub fn rejected_pipeline(receive: &std::sync::mpsc::Receiver<String>) -> String {
    record("phase: waiting for rejected pipeline");
    let mut transcript = String::new();
    loop {
        let line = receive.recv().expect(&transcript);
        transcript.push_str(&line);
        if line.contains("error from build pipeline") {
            return transcript;
        }
    }
}

/// Restrict the cold witness to bindings and the prepared include it consumes.
pub fn bootstrap_source(browser: &Browser) {
    let manifest = browser.fixture("Cargo.toml");
    let mut value: toml::Value = toml::from_str(&fs::read_to_string(&manifest).unwrap()).unwrap();
    value["dependencies"]
        .as_table_mut()
        .unwrap()
        .retain(|name, _| name == "wasm-bindgen");
    fs::write(manifest, toml::to_string(&value).unwrap()).unwrap();
    fs::write(
        browser.fixture("src/main.rs"),
        include_str!("browser_bootstrap.rs"),
    )
    .unwrap();
    browser.generate_lockfile();
}
