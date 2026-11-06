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
    /// The lane whose target the warm recipes build in, held while the fixture lives.
    lane: Option<Lane>,
}

/// One prepared Cargo target that a single browser fixture builds in at a time: tests change
/// sources and rebuild, and Trunk writes its outputs outside Cargo's build lock.
struct Lane {
    /// The lane's position among the announced targets.
    index: usize,
    /// The Cargo target directory.
    target: PathBuf,
    /// The lock that holds the lane.
    _lock: fs::File,
}

impl Lane {
    /// Hold the lane of this test's group slot, waiting while another fixture holds it.
    ///
    /// Nextest numbers the browser author tests that run at once from zero, so each takes a lane
    /// of its own; a test outside the group, or a run outside nextest, takes the first lane.
    fn take() -> Self {
        let mut targets = target_directories();
        let slot = std::env::var("NEXTEST_TEST_GROUP_SLOT")
            .ok()
            .and_then(|slot| slot.parse::<usize>().ok())
            .unwrap_or(0);
        let index = slot % targets.len();
        let target = targets.swap_remove(index);
        fs::create_dir_all(&target).unwrap();
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(target.join("browser-author-tests.lock"))
            .unwrap();
        record(format!("phase: browser lane {index} waiting"));
        lock.lock().unwrap();
        record(format!("phase: browser lane {index} acquired"));
        Self {
            index,
            target,
            _lock: lock,
        }
    }
}

impl Browser {
    pub fn new() -> Self {
        Self::prepare(Some(Lane::take()))
    }

    /// Prepare isolated inputs without taking a lane.
    /// Every build command must supply its own Cargo target directory.
    pub fn cold() -> Self {
        Self::prepare(None)
    }

    /// Copy authored inputs while holding any lane.
    fn prepare(lane: Option<Lane>) -> Self {
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
        let browser = Self { workspace, lane };
        record("phase: workspace source preparation");
        browser.workspace_sources();
        record("phase: browser setup complete");
        browser
    }

    /// Recreate inputs at the lane's cache path so Cargo retains workspace package identities.
    pub fn cached() -> Self {
        let lane = Lane::take();
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/Maestro's browser 東京 checkout {}",
            lane.index
        ));
        let mut browser = Self::prepare(Some(lane));
        browser.workspace.cache_at(&path);
        // Restored mutable sources must be newer than any compiled edited version.
        for source in [
            browser.fixture("src/main.rs"),
            browser.path("crates/maestro-web/src/lib.rs"),
        ] {
            fs::File::open(source)
                .unwrap()
                .set_modified(std::time::SystemTime::now())
                .unwrap();
        }
        browser
    }

    /// Resolve the fixture manifest's crates from the run's prepared sources, in a private Cargo
    /// home. Builds that share the test build's target, and resolution of the whole workspace,
    /// keep the shared home, so their compiled dependencies stay fresh.
    pub fn cargo(&self) -> Command {
        self.workspace.use_prepared_sources();
        self.workspace.toolchain_command(env!("CARGO"))
    }

    pub fn generate_lockfile(&self) {
        record("phase: cargo generate-lockfile");
        let status = super::recipe_support::setup_status(
            self.cargo()
                .args(["generate-lockfile", "--offline", "--manifest-path"])
                .arg(self.fixture("Cargo.toml")),
        );
        record(format!("phase: cargo generate-lockfile complete {status}"));
        assert!(status.success());
    }

    pub fn command(&self, recipe: &str) -> Command {
        record(format!("phase: recipe {recipe}"));
        let mut command = super::toolchain_support::command("just");
        command.arg(recipe);
        self.configure(&mut command);
        command.env("CARGO_BUILD_JOBS", lane_jobs());
        command
    }

    /// Run a cold recipe with private Cargo state excluded from compiler-cache keys. Its compile
    /// jobs keep the caller's cap, or every core without one: the test reserves every core for the
    /// cold compile.
    pub fn cold_command(&self, recipe: &str) -> Command {
        let mut command = self.workspace.toolchain_command("just");
        command.arg(recipe);
        self.configure(&mut command);
        command
    }

    /// Execute the built native helper without instrumenting its compiler children.
    pub fn native_command(&self, operation: &str) -> Command {
        let mut command = super::toolchain_support::command(env!("CARGO_BIN_EXE_development"));
        command
            .args(["browser-author", operation])
            .env("CARGO_MANIFEST_DIR", self.path("crates/maestro-tooling"))
            .env("CARGO_BUILD_JOBS", lane_jobs());
        self.configure(&mut command);
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        command
    }

    /// The Cargo target the warm recipes build in: the lane the fixture holds, or the first lane.
    pub fn target(&self) -> PathBuf {
        self.lane.as_ref().map_or_else(
            || target_directories().swap_remove(0),
            |lane| lane.target.clone(),
        )
    }

    /// Share output ownership and native tool settings between recipe and helper calls.
    fn configure(&self, command: &mut Command) {
        command
            .current_dir(self.workspace.path())
            .env("CARGO_TARGET_DIR", self.target())
            .env("TRUNK_SKIP_VERSION_CHECK", "true")
            // Trunk's completion emoji depends on LANG, even with piped output.
            .env("LANG", "C.UTF-8");
    }

    pub fn run(&self, recipe: &str) -> Output {
        let output = self.command(recipe).output().unwrap();
        record(format!("phase: recipe {recipe} {}", output.status));
        output
    }

    pub fn path(&self, path: &str) -> PathBuf {
        self.workspace.path().join(path)
    }

    pub fn fixture(&self, path: &str) -> PathBuf {
        self.path(FIXTURE).join(path)
    }
}

/// The lanes' target directories: the run's prepared ones when nextest's setup announces them,
/// otherwise the one the test build wrote, as the only lane.
pub fn target_directories() -> Vec<PathBuf> {
    std::env::var_os("MAESTRO_BROWSER_TARGET_DIRS").map_or_else(
        || {
            vec![
                Path::new(env!("CARGO_BIN_EXE_development"))
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .to_path_buf(),
            ]
        },
        |targets| std::env::split_paths(&targets).collect(),
    )
}

/// The compile jobs of one lane: the caller's job cap, or every core without one, shared between
/// the lanes, at least one each. Every lane can build at once, so the lanes together ask for no
/// more jobs than the run has cores.
fn lane_jobs() -> String {
    let cores = std::env::var("CARGO_BUILD_JOBS")
        .ok()
        .and_then(|jobs| jobs.parse::<usize>().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().unwrap().get());
    (cores / target_directories().len()).max(1).to_string()
}

/// The Cargo target directory `command` builds in.
pub fn cargo_target(command: &Command) -> PathBuf {
    PathBuf::from(
        command
            .get_envs()
            .find(|(key, _)| *key == "CARGO_TARGET_DIR")
            .unwrap()
            .1
            .unwrap(),
    )
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
    super::recipe_support::forwarder(&browser.path("native-bin"), name);
    let log = browser.path("native-log");
    command
        .env(
            "PATH",
            format!(
                "{}:{}",
                browser.path("native-bin").display(),
                std::env::var("PATH").unwrap()
            ),
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

/// Reuse locked dependency sources with initially empty Cargo home and target directories.
pub fn cold_sources(browser: &Browser) -> (PathBuf, PathBuf) {
    let sources = super::recipe_support::prepared_workspace_sources();
    let home = browser.path("cold-home");
    let target = browser.path("cold-target");
    fs::create_dir(&home).unwrap();
    fs::create_dir(&target).unwrap();
    assert_eq!(fs::read_dir(&home).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    record(format!(
        "phase: copy prepared sources {}",
        sources.display()
    ));
    fs::copy(sources, home.join("config.toml")).unwrap();
    (home, target)
}

/// Fill the cold target with the registry units the workspace-sources setup compiled, so only
/// Maestro's own crates compile from empty state.
pub fn seed_target(target: &Path) {
    let seed = super::recipe_support::prepared_workspace_sources().with_file_name("seed");
    assert!(
        seed.is_dir(),
        "the workspace-sources setup must prepare {}",
        seed.display()
    );
    record(format!("phase: copy registry seed {}", seed.display()));
    let mut source = seed.into_os_string();
    source.push("/.");
    assert!(
        Command::new("cp")
            .arg("-a")
            .arg(source)
            .arg(target)
            .status()
            .unwrap()
            .success()
    );
    let units = fs::read_dir(target.join("debug/.fingerprint"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    assert!(!units.is_empty(), "the seed holds no compiled units");
    assert!(
        units.iter().all(|unit| !unit.starts_with("maestro-")),
        "the seed holds workspace member units: {units:?}"
    );
    let outputs = fs::read_dir(target.join("debug/deps"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    assert!(
        outputs
            .iter()
            .all(|output| !output.starts_with("development-")),
        "the seed holds a workspace binary: {outputs:?}"
    );
    assert!(!target.join("debug/incremental").exists());
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
