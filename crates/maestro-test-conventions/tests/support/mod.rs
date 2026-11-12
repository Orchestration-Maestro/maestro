#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The guest crate's own lint table: the fixture workspace table with the three lints its
/// generated bindings need at `deny`.
pub const GUEST_LINTS: &str = "[lints.rust]\nunsafe_code = \"forbid\"\nforbidden_lint_groups = \"forbid\"\n[lints.clippy]\npedantic = { level = \"deny\", priority = -1 }\nmodule_name_repetitions = \"forbid\"\nmissing_docs_in_private_items = \"forbid\"\nredundant_clone = \"forbid\"\ntoo_many_arguments = \"deny\"\nfn_params_excessive_bools = \"forbid\"\ntoo_many_lines = \"forbid\"\ncognitive_complexity = \"forbid\"\nexcessive_nesting = \"deny\"\nunwrap_used = \"forbid\"\nexpect_used = \"forbid\"\npanic = \"forbid\"\n";

mod manifest;
mod scratch;

use manifest::WORKSPACE_MANIFEST;
pub use scratch::Scratch;

pub struct Workspace {
    scratch: Scratch,
}

impl std::ops::Deref for Workspace {
    type Target = Scratch;

    fn deref(&self) -> &Scratch {
        &self.scratch
    }
}

impl Workspace {
    pub fn new() -> Self {
        let workspace = Self {
            scratch: Scratch::new(),
        };
        fs::write(workspace.root.path().join("Cargo.toml"), WORKSPACE_MANIFEST).unwrap();
        workspace.list(&[]);
        workspace
    }

    /// A workspace with the one core member `maestro-text`.
    pub fn with_text_member() -> Self {
        let workspace = Self::new();
        workspace.member("text", "maestro-text", "");
        workspace.list(&[("maestro-text", "core")]);
        workspace
    }

    pub fn member(&self, directory: &str, name: &str, dependencies: &str) {
        let root = self.root.path().join("crates").join(directory);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "").unwrap();
        let lints = if name == "maestro-extensions-wasm" {
            GUEST_LINTS.to_owned()
        } else {
            "[lints]\nworkspace = true\n".to_owned()
        };
        let manifest = format!(
            "[package]\nname = {name:?}\nversion = \"0.1.0\"\nedition = \"2024\"\n{lints}{dependencies}\n"
        );
        // A caller that declares the library target itself keeps one `[lib]` table.
        let manifest = if manifest.contains("[lib]\n") {
            manifest.replace("[lib]\n", "[lib]\ntest = false\n")
        } else {
            format!("{manifest}[lib]\ntest = false\n")
        };
        fs::write(root.join("Cargo.toml"), manifest).unwrap();
    }

    pub fn external(&self, name: &str) {
        let root = self.root.path().join("external");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "").unwrap();
        fs::write(
            root.join("Cargo.toml"),
            format!(
                "[workspace]\n[package]\nname = {name:?}\nversion = \"9.0.0\"\nedition = \"2024\"\n"
            ),
        )
        .unwrap();
        fs::write(self.root.path().join("Cargo.toml"), format!("[workspace]\nmembers = [\"crates/*\"]\nexclude = [\"external\"]\nresolver = \"3\"\n[workspace.dependencies]\nserde_json = {{ version = \"=1.0.151\", features = [\"float_roundtrip\"] }}\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\nforbidden_lint_groups = \"forbid\"\n[workspace.lints.clippy]\npedantic = {{ level = \"forbid\", priority = -1 }}\nmodule_name_repetitions = \"forbid\"\nmissing_docs_in_private_items = \"forbid\"\nredundant_clone = \"forbid\"\ntoo_many_arguments = \"forbid\"\nfn_params_excessive_bools = \"forbid\"\ntoo_many_lines = \"forbid\"\ncognitive_complexity = \"forbid\"\nexcessive_nesting = \"forbid\"\nunwrap_used = \"forbid\"\nexpect_used = \"forbid\"\npanic = \"forbid\"\n[patch.crates-io]\n{name} = {{ path = \"external\" }}\n")).unwrap();
    }

    /// Write the member's `[lib] test` setting: `true` keeps the library test executable, `false` skips it.
    pub fn library_test_target(&self, directory: &str, tests: bool) {
        let path = self
            .root
            .path()
            .join("crates")
            .join(directory)
            .join("Cargo.toml");
        let text = fs::read_to_string(&path).unwrap();
        let flag = "[lib]\ntest = false\n";
        let updated = if tests {
            text.replace(flag, "[lib]\n")
        } else if text.contains(flag) {
            text
        } else if text.contains("[lib]\n") {
            text.replace("[lib]\n", flag)
        } else {
            format!("{text}{flag}")
        };
        fs::write(path, updated).unwrap();
    }

    /// Package a fixture's top-level test files in the canonical executable.
    pub fn integration(&self, directory: &str) {
        let root = self.root.path().join("crates").join(directory);
        let tests = root.join("tests");
        let modules: Vec<_> = std::fs::read_dir(&tests)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
            .map(|path| format!("mod {};\n", path.file_stem().unwrap().to_str().unwrap()))
            .collect();
        let manifest = root.join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest).unwrap();
        std::fs::write(
            manifest,
            format!(
                "{}\n[[test]]\nname = \"integration\"\npath = \"tests/integration.rs\"\n",
                text.replace("[package]", "[package]\nautotests = false")
            ),
        )
        .unwrap();
        std::fs::write(tests.join("integration.rs"), modules.concat()).unwrap();
    }

    /// Writes the crate list, overwriting nextest config when it includes the test-process owner.
    pub fn list(&self, entries: &[(&str, &str)]) {
        let entries: serde_json::Map<String, serde_json::Value> = entries
            .iter()
            .map(|(name, layer)| (name.to_string(), (*layer).into()))
            .collect();
        fs::write(
            self.root.path().join("workspace-crates.json"),
            serde_json::to_vec_pretty(&entries).unwrap(),
        )
        .unwrap();
        if entries.contains_key(policy::TEST_PROCESS) {
            let filter = policy::TEST_PROCESS_CALLERS
                .iter()
                .copied()
                .chain([policy::TEST_PROCESS])
                .map(|name| format!("package(={name})"))
                .collect::<Vec<_>>()
                .join(" | ");
            let config = self.root.path().join(".config");
            fs::create_dir_all(&config).unwrap();
            fs::write(
                config.join("nextest.toml"),
                format!(
                    "[profile.default]\nslow-timeout = {{ period = '30s', terminate-after = 4 }}\n[scripts.setup.native-fixture]\ncommand = 'development nextest-setup native-fixture'\n[[profile.default.scripts]]\nsetup = 'native-fixture'\nfilter = '{filter}'\n"
                ),
            )
            .unwrap();
        }
    }
}

#[allow(
    dead_code,
    reason = "The policy file is included whole; some of its items are unused here."
)]
#[path = "../../src/graph/policy.rs"]
pub mod policy;

/// Run Git in `root` without any inherited repository, ignore or global configuration.
///
/// A test hook exports `GIT_DIR` and `GIT_INDEX_FILE`, so a fixture must clear them or
/// its commands reach the repository that runs the suite.
pub fn git(root: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(root);
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("GIT_") {
            command.env_remove(name);
        }
    }
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("LC_ALL", "C")
        .args(["-c", "core.excludesFile=/dev/null"]);
    command
}

pub fn required(name: &str) -> &'static [&'static str] {
    policy::required(name)
}

pub fn class(name: &str) -> &'static str {
    policy::rule(name).unwrap().class
}

/// A copy of the declared document in which `owner` also declares `dependency`.
pub fn with_dependency(
    document: &serde_json::Value,
    owner: &str,
    dependency: &serde_json::Value,
) -> serde_json::Value {
    let mut patched = document.clone();
    let package = patched["packages"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|package| package["name"] == owner)
        .expect("the declared document holds every fixture crate");
    let mut dependencies = package["dependencies"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    dependencies.push(dependency.clone());
    package["dependencies"] = serde_json::json!(dependencies);
    patched
}

/// Convert an authored slash-delimited relative path to native components.
pub fn native_path(relative: &str) -> PathBuf {
    relative.split('/').collect()
}

/// Spell an authored relative path with native separators for a diagnostic assertion.
pub fn native_path_text(relative: &str) -> String {
    native_path(relative).display().to_string()
}

/// Write one authored source under a member's directory, creating its parents, and return
/// its path.
///
/// The authored relative path is written with slashes and returned with the native
/// separators the file system reports, so a diagnostic comparison matches on every host.
/// A library source that holds tests turns the member's library test target back on, so
/// every fixture keeps the agreement the workspace check requires.
pub fn source(workspace: &Workspace, member: &str, relative: &str, contents: &str) -> PathBuf {
    let path = workspace
        .root
        .path()
        .join("crates")
        .join(member)
        .join(native_path(relative));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, contents).unwrap();
    if contains_tests(relative, contents) {
        workspace.library_test_target(member, true);
    }
    path
}

/// Whether an authored library source holds tests: a test layout, a `#[cfg(test)]` gate or a `#[test]` attribute.
fn contains_tests(relative: &str, contents: &str) -> bool {
    relative.starts_with("src/")
        && (contents.contains("#[cfg(test)]")
            || contents.contains("#[test]")
            || relative
                .split('/')
                .any(|part| part == "tests" || part == "tests.rs"))
}

impl Workspace {
    pub fn metadata(&self) -> serde_json::Value {
        let output =
            std::process::Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
                .args(["metadata", "--format-version", "1", "--offline"])
                .current_dir(self.root.path())
                .output()
                .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    pub fn foundation(&self, names: &[&str]) {
        let mut pending = names.to_vec();
        let mut entries = std::collections::BTreeSet::new();
        while let Some(name) = pending.pop() {
            if !entries.insert(name) {
                continue;
            }
            let targets = required(name);
            let mut dependencies = String::new();
            if !targets.is_empty() {
                dependencies.push_str("[dependencies]\n");
                pending.extend(targets);
                dependencies.extend(
                    targets
                        .iter()
                        .map(|target| format!("{target} = {{ path = \"../{target}\" }}\n")),
                );
            }
            self.member(name, name, &dependencies);
        }
        self.list(
            &entries
                .into_iter()
                .map(|name| (name, class(name)))
                .collect::<Vec<_>>(),
        );
    }
}

#[cfg(unix)]
impl Workspace {
    /// Install the prepared native fixture as `name` in the workspace root, answering as the
    /// `command` family `record` says. The alias is copied by a child, and its record is a
    /// sibling file that is read, never executed.
    pub fn command(&self, name: &str, record: &serde_json::Value) -> PathBuf {
        let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
            .expect("run just prepare-tests, then run these tests with cargo nextest");
        let path = self.root.path().join(name);
        maestro_test_process::install_executable(std::path::Path::new(&artifact), &path).unwrap();
        fs::write(record_path(&path), format!("command\n{record}")).unwrap();
        path
    }

    /// A Cargo that serves `declared` for `--no-deps` and `resolved.json` otherwise, both read
    /// from the workspace root when it runs, and logs its operands as `mark` when one is given.
    pub fn metadata_command(&self, name: &str, declared: &str, mark: Option<&str>) -> PathBuf {
        let root = self.root.path();
        let mut record = serde_json::json!({
            "scenario": "metadata",
            "declared": root.join(declared),
            "resolved": root.join("resolved.json"),
        });
        if let Some(mark) = mark {
            record["argv_log"] = serde_json::json!({"path": root.join("order"), "mark": mark});
        }
        self.command(name, &record)
    }

    pub fn metadata_probe(
        &self,
        declared: &serde_json::Value,
        resolved: &serde_json::Value,
        expectation: &str,
    ) {
        let (cargo, rustc) = self.metadata_commands(declared, resolved);
        self.probe(&cargo, &rustc, expectation);
    }

    /// Check every policy edge case in one probe child against one parsed metadata document.
    pub fn policy_probe(&self, valid: &serde_json::Value, dev: bool) {
        let (cargo, rustc) = self.metadata_commands(valid, valid);
        let matrix = if dev { "dev" } else { "production" };
        run_probe(
            self.probe_command(&cargo, &rustc)
                .env("MAESTRO_CONVENTIONS_POLICY_MATRIX", matrix),
        );
    }

    /// Serve `declared` to the declared pass and `resolved` to the host pass.
    fn metadata_commands(
        &self,
        declared: &serde_json::Value,
        resolved: &serde_json::Value,
    ) -> (PathBuf, PathBuf) {
        fs::write(
            self.root.path().join("declared.json"),
            serde_json::to_vec(declared).unwrap(),
        )
        .unwrap();
        fs::write(
            self.root.path().join("resolved.json"),
            serde_json::to_vec(resolved).unwrap(),
        )
        .unwrap();
        let cargo = self.metadata_command("cargo-probe", "declared.json", None);
        let rustc = self.command("rustc-probe", &host_report());
        (cargo, rustc)
    }

    pub fn probe(&self, cargo: &std::path::Path, rustc: &std::path::Path, expectation: &str) {
        run_probe(
            self.probe_command(cargo, rustc)
                .env("MAESTRO_CONVENTIONS_EXPECT", expectation),
        );
    }

    /// The probe child with fake Cargo and compiler commands.
    fn probe_command(
        &self,
        cargo: &std::path::Path,
        rustc: &std::path::Path,
    ) -> maestro_test_process::TestProcess {
        maestro_test_process::TestProcess::named(
            "architecture::metadata_graph_failures_cross_the_public_interface",
        )
        .unwrap()
        .nocapture()
        .env("MAESTRO_CONVENTIONS_PROBE_ROOT", self.root.path())
        .env("CARGO", cargo)
        .env("RUSTC", rustc)
    }
}

/// Run a probe child and require that it ran its one probe test successfully.
#[cfg(unix)]
fn run_probe(command: maestro_test_process::TestProcess) {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The sibling file that holds an installed alias's scenario record.
#[cfg(unix)]
pub fn record_path(alias: &std::path::Path) -> PathBuf {
    let mut record = alias.as_os_str().to_owned();
    record.push(".fixture");
    record.into()
}

/// A stand-in that writes the given bytes to each stream and ends with `code`.
#[cfg(unix)]
pub fn output(stdout: &[u8], stderr: &[u8], code: u8) -> serde_json::Value {
    serde_json::json!({"scenario": "output", "stdout": stdout, "stderr": stderr, "code": code})
}

/// The answer of a compiler whose host triple is the Linux one.
#[cfg(unix)]
pub fn host_report() -> serde_json::Value {
    output(b"host: x86_64-unknown-linux-gnu\n", b"", 0)
}
