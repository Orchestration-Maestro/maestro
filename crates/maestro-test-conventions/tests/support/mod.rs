#![cfg(test)]

use std::fs;
use std::path::PathBuf;

/// The guest crate's own lint table: the fixture workspace table with the three lints its
/// generated bindings need at `deny`.
pub const GUEST_LINTS: &str = "[lints.rust]\nunsafe_code = \"forbid\"\nforbidden_lint_groups = \"forbid\"\n[lints.clippy]\npedantic = { level = \"deny\", priority = -1 }\nmissing_docs_in_private_items = \"forbid\"\nredundant_clone = \"forbid\"\ntoo_many_arguments = \"deny\"\nfn_params_excessive_bools = \"forbid\"\ntoo_many_lines = \"forbid\"\ncognitive_complexity = \"forbid\"\nexcessive_nesting = \"deny\"\nunwrap_used = \"forbid\"\nexpect_used = \"forbid\"\npanic = \"forbid\"\n";

mod scratch;
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
        fs::write(
            workspace.root.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n[workspace.dependencies]\nserde_json = { version = \"=1.0.151\", features = [\"float_roundtrip\"] }\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\nforbidden_lint_groups = \"forbid\"\n[workspace.lints.clippy]\npedantic = { level = \"forbid\", priority = -1 }\nmissing_docs_in_private_items = \"forbid\"\nredundant_clone = \"forbid\"\ntoo_many_arguments = \"forbid\"\nfn_params_excessive_bools = \"forbid\"\ntoo_many_lines = \"forbid\"\ncognitive_complexity = \"forbid\"\nexcessive_nesting = \"forbid\"\nunwrap_used = \"forbid\"\nexpect_used = \"forbid\"\npanic = \"forbid\"\n",
        )
        .unwrap();
        workspace.list(&[]);
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
        fs::write(
            root.join("Cargo.toml"),
            format!(
                "[package]\nname = {name:?}\nversion = \"0.1.0\"\nedition = \"2024\"\n{lints}{dependencies}\n"
            ),
        )
        .unwrap();
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
        fs::write(self.root.path().join("Cargo.toml"), format!("[workspace]\nmembers = [\"crates/*\"]\nexclude = [\"external\"]\nresolver = \"3\"\n[workspace.dependencies]\nserde_json = {{ version = \"=1.0.151\", features = [\"float_roundtrip\"] }}\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\nforbidden_lint_groups = \"forbid\"\n[workspace.lints.clippy]\npedantic = {{ level = \"forbid\", priority = -1 }}\nmissing_docs_in_private_items = \"forbid\"\nredundant_clone = \"forbid\"\ntoo_many_arguments = \"forbid\"\nfn_params_excessive_bools = \"forbid\"\ntoo_many_lines = \"forbid\"\ncognitive_complexity = \"forbid\"\nexcessive_nesting = \"forbid\"\nunwrap_used = \"forbid\"\nexpect_used = \"forbid\"\npanic = \"forbid\"\n[patch.crates-io]\n{name} = {{ path = \"external\" }}\n")).unwrap();
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
    }
}

#[allow(
    dead_code,
    reason = "The policy file is included whole; some of its items are unused here."
)]
#[path = "../../src/graph/policy.rs"]
pub mod policy;

pub fn required(name: &str) -> &'static [&'static str] {
    policy::required(name)
}

pub fn class(name: &str) -> &'static str {
    policy::rule(name).unwrap().class
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
pub fn source(workspace: &Workspace, member: &str, relative: &str, contents: &str) -> PathBuf {
    let path = workspace
        .root
        .path()
        .join("crates")
        .join(member)
        .join(native_path(relative));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, contents).unwrap();
    path
}

impl Workspace {
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
    pub fn command(&self, name: &str, body: &str) -> PathBuf {
        let path = self.root.path().join(name);
        fs::write(&path, format!("{body}\n")).unwrap();
        path
    }

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

    pub fn metadata_probe(
        &self,
        declared: &serde_json::Value,
        resolved: &serde_json::Value,
        expectation: &str,
    ) {
        let (cargo, rustc) = self.metadata_commands(declared, resolved);
        self.probe(&cargo, &rustc, expectation);
    }

    /// Check every policy edge case in one probe child, which rewrites the declared metadata
    /// per case instead of starting a child per case.
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
        let cargo = self.command(
            "cargo-probe",
            "case \" $* \" in *' --no-deps '*) cat declared.json ;; *) cat resolved.json ;; esac",
        );
        let rustc = self.command("rustc-probe", "printf 'host: x86_64-unknown-linux-gnu\\n'");
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
        let launcher =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/metadata-command.sh");
        maestro_test_process::TestProcess::named(
            "architecture::metadata_graph_failures_cross_the_public_interface",
        )
        .unwrap()
        .nocapture()
        .env("MAESTRO_CONVENTIONS_PROBE_ROOT", self.root.path())
        .env("CARGO", if cargo.is_file() { &launcher } else { cargo })
        .env("RUSTC", if rustc.is_file() { &launcher } else { rustc })
        .env("MAESTRO_FAKE_CARGO_SCENARIO", cargo)
        .env("MAESTRO_FAKE_RUSTC_SCENARIO", rustc)
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
