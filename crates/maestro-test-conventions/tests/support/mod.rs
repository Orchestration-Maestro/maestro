#![cfg(test)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// The guest crate's own lint table: the fixture workspace table with the three lints its
/// generated bindings need at `deny`.
pub const GUEST_LINTS: &str = "[lints.rust]\nunsafe_code = \"forbid\"\nforbidden_lint_groups = \"forbid\"\n[lints.clippy]\npedantic = { level = \"deny\", priority = -1 }\nmissing_docs_in_private_items = \"forbid\"\ntoo_many_arguments = \"deny\"\nfn_params_excessive_bools = \"forbid\"\ntoo_many_lines = \"forbid\"\ncognitive_complexity = \"forbid\"\nexcessive_nesting = \"deny\"\nunwrap_used = \"forbid\"\nexpect_used = \"forbid\"\npanic = \"forbid\"\n";

pub struct Workspace {
    pub root: PathBuf,
}

impl Workspace {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let root = std::env::temp_dir().join(format!(
                "maestro-conventions-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&root) {
                Ok(()) => {
                    let workspace = Self { root };
                    fs::write(
                        workspace.root.join("Cargo.toml"),
                        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\nforbidden_lint_groups = \"forbid\"\n[workspace.lints.clippy]\npedantic = { level = \"forbid\", priority = -1 }\nmissing_docs_in_private_items = \"forbid\"\nredundant_clone = \"forbid\"\ntoo_many_arguments = \"forbid\"\nfn_params_excessive_bools = \"forbid\"\ntoo_many_lines = \"forbid\"\ncognitive_complexity = \"forbid\"\nexcessive_nesting = \"forbid\"\nunwrap_used = \"forbid\"\nexpect_used = \"forbid\"\npanic = \"forbid\"\n",
                    )
                    .unwrap();
                    workspace.list(&[]);
                    return workspace;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("create fixture: {error}"),
            }
        }
    }

    pub fn member(&self, directory: &str, name: &str, dependencies: &str) {
        let root = self.root.join("crates").join(directory);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "").unwrap();
        let lints = if name == "maestro-extensions-wasm" {
            GUEST_LINTS
        } else {
            "[lints]\nworkspace = true\n"
        };
        fs::write(
            root.join("Cargo.toml"),
            format!(
                "[package]\nname = {name:?}\nversion = \"0.1.0\"\nedition = \"2024\"\n{lints}{dependencies}\n"
            ),
        )
        .unwrap();
    }

    #[allow(
        dead_code,
        reason = "External fixtures are unused by the comment-test executable."
    )]
    pub fn external(&self, name: &str) {
        let root = self.root.join("external");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "").unwrap();
        fs::write(
            root.join("Cargo.toml"),
            format!(
                "[workspace]\n[package]\nname = {name:?}\nversion = \"9.0.0\"\nedition = \"2024\"\n"
            ),
        )
        .unwrap();
        fs::write(self.root.join("Cargo.toml"), format!("[workspace]\nmembers = [\"crates/*\"]\nexclude = [\"external\"]\nresolver = \"3\"\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\nforbidden_lint_groups = \"forbid\"\n[workspace.lints.clippy]\npedantic = {{ level = \"forbid\", priority = -1 }}\nmissing_docs_in_private_items = \"forbid\"\nredundant_clone = \"forbid\"\ntoo_many_arguments = \"forbid\"\nfn_params_excessive_bools = \"forbid\"\ntoo_many_lines = \"forbid\"\ncognitive_complexity = \"forbid\"\nexcessive_nesting = \"forbid\"\nunwrap_used = \"forbid\"\nexpect_used = \"forbid\"\npanic = \"forbid\"\n[patch.crates-io]\n{name} = {{ path = \"external\" }}\n")).unwrap();
    }

    pub fn list(&self, entries: &[(&str, &str)]) {
        let entries: serde_json::Map<String, serde_json::Value> = entries
            .iter()
            .map(|(name, layer)| (name.to_string(), (*layer).into()))
            .collect();
        fs::write(
            self.root.join("workspace-crates.json"),
            serde_json::to_vec_pretty(&entries).unwrap(),
        )
        .unwrap();
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let result = fs::remove_dir_all(&self.root);
        if !std::thread::panicking() {
            result.expect("remove fixture workspace");
        }
    }
}

#[allow(
    dead_code,
    reason = "Policy fixtures are shared across test executables."
)]
#[path = "../../src/graph/policy.rs"]
pub mod policy;

#[allow(dead_code, reason = "Graph fixtures are unused by comment tests.")]
pub fn exact(name: &str) -> bool {
    policy::exact(name)
}

#[allow(dead_code, reason = "Graph fixtures are unused by comment tests.")]
pub fn class(name: &str) -> &'static str {
    policy::rule(name).unwrap().class
}

#[allow(dead_code, reason = "Graph fixtures are unused by comment tests.")]
impl Workspace {
    pub fn foundation(&self, names: &[&str]) {
        let mut pending = names.to_vec();
        let mut entries = std::collections::BTreeSet::new();
        while let Some(name) = pending.pop() {
            if !entries.insert(name) {
                continue;
            }
            let targets = policy::rule(name).unwrap().dependencies;
            let mut dependencies = String::new();
            if exact(name) {
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
#[allow(
    dead_code,
    reason = "Controlled commands are used only by process tests."
)]
impl Workspace {
    pub fn command(&self, name: &str, body: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, format!("{body}\n")).unwrap();
        path
    }

    pub fn metadata(&self) -> serde_json::Value {
        let output =
            std::process::Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
                .args(["metadata", "--format-version", "1", "--offline"])
                .current_dir(&self.root)
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
        fs::write(
            self.root.join("declared.json"),
            serde_json::to_vec(declared).unwrap(),
        )
        .unwrap();
        fs::write(
            self.root.join("resolved.json"),
            serde_json::to_vec(resolved).unwrap(),
        )
        .unwrap();
        let cargo = self.command(
            "cargo-probe",
            "case \" $* \" in *' --no-deps '*) cat declared.json ;; *) cat resolved.json ;; esac",
        );
        let rustc = self.command("rustc-probe", "printf 'host: x86_64-unknown-linux-gnu\\n'");
        self.probe(&cargo, &rustc, expectation);
    }

    pub fn probe(&self, cargo: &std::path::Path, rustc: &std::path::Path, expectation: &str) {
        let launcher =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/metadata-command.sh");
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "metadata_graph_failures_cross_the_public_interface",
                "--nocapture",
            ])
            .env("MAESTRO_CONVENTIONS_PROBE_ROOT", &self.root)
            .env("MAESTRO_CONVENTIONS_EXPECT", expectation)
            .env("CARGO", if cargo.is_file() { &launcher } else { cargo })
            .env("RUSTC", if rustc.is_file() { &launcher } else { rustc })
            .env("MAESTRO_FAKE_CARGO_SCENARIO", cargo)
            .env("MAESTRO_FAKE_RUSTC_SCENARIO", rustc)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
