#![cfg(test)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

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
                        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\nforbidden_lint_groups = \"forbid\"\n[workspace.lints.clippy]\npedantic = { level = \"forbid\", priority = -1 }\ntoo_many_arguments = \"forbid\"\nfn_params_excessive_bools = \"forbid\"\ntoo_many_lines = \"forbid\"\ncognitive_complexity = \"forbid\"\nexcessive_nesting = \"forbid\"\nunwrap_used = \"forbid\"\nexpect_used = \"forbid\"\npanic = \"forbid\"\n",
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
        fs::write(
            root.join("Cargo.toml"),
            format!(
                "[package]\nname = {name:?}\nversion = \"0.1.0\"\nedition = \"2024\"\n[lints]\nworkspace = true\n{dependencies}\n"
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
        fs::write(self.root.join("Cargo.toml"), format!("[workspace]\nmembers = [\"crates/*\"]\nexclude = [\"external\"]\nresolver = \"3\"\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\nforbidden_lint_groups = \"forbid\"\n[workspace.lints.clippy]\npedantic = {{ level = \"forbid\", priority = -1 }}\ntoo_many_arguments = \"forbid\"\nfn_params_excessive_bools = \"forbid\"\ntoo_many_lines = \"forbid\"\ncognitive_complexity = \"forbid\"\nexcessive_nesting = \"forbid\"\nunwrap_used = \"forbid\"\nexpect_used = \"forbid\"\npanic = \"forbid\"\n[patch.crates-io]\n{name} = {{ path = \"external\" }}\n")).unwrap();
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
    reason = "Fixture matrices are shared by separate test executables."
)]
pub mod policy {
    pub const POLICY: &[(&str, &[&str])] = &[
        ("maestro-extensions-wasm", &[]),
        ("maestro-models", &[]),
        ("maestro-resources", &[]),
        ("maestro-settings", &[]),
        ("maestro-storage", &[]),
        ("maestro-test-conventions", &[]),
        ("maestro-tooling", &[]),
        ("maestro-tui", &[]),
        ("maestro-agent", &["maestro-models"]),
        ("maestro-credentials", &["maestro-models"]),
        (
            "maestro-packages",
            &["maestro-settings", "maestro-resources"],
        ),
        ("maestro-test-terminal", &["maestro-tui"]),
        ("maestro-theme", &["maestro-tui"]),
        ("maestro-tui-crossterm", &["maestro-tui"]),
        (
            "maestro-catalog",
            &["maestro-models", "maestro-credentials"],
        ),
        (
            "maestro-session",
            &["maestro-models", "maestro-agent", "maestro-storage"],
        ),
        (
            "maestro-tools",
            &[
                "maestro-models",
                "maestro-agent",
                "maestro-tui",
                "maestro-theme",
            ],
        ),
        (
            "maestro-export",
            &[
                "maestro-session",
                "maestro-models",
                "maestro-tools",
                "maestro-theme",
                "maestro-tui",
            ],
        ),
        (
            "maestro-extensions",
            &[
                "maestro-models",
                "maestro-agent",
                "maestro-session",
                "maestro-catalog",
                "maestro-tools",
                "maestro-theme",
                "maestro-tui",
                "maestro-resources",
            ],
        ),
        (
            "maestro-app",
            &[
                "maestro-models",
                "maestro-agent",
                "maestro-credentials",
                "maestro-settings",
                "maestro-storage",
                "maestro-catalog",
                "maestro-session",
                "maestro-tools",
                "maestro-resources",
                "maestro-packages",
                "maestro-extensions",
                "maestro-export",
                "maestro-theme",
                "maestro-tui",
            ],
        ),
        ("maestro-extensions-wasmtime", &["maestro-extensions"]),
        (
            "maestro-chat",
            &[
                "maestro-app",
                "maestro-tui",
                "maestro-tui-crossterm",
                "maestro-theme",
            ],
        ),
        (
            "maestro-cli",
            &[
                "maestro-app",
                "maestro-tui",
                "maestro-tui-crossterm",
                "maestro-theme",
            ],
        ),
        ("maestro-rpc", &["maestro-app", "maestro-theme"]),
        ("maestro-web", &["maestro-app", "maestro-theme"]),
        (
            "maestro",
            &[
                "maestro-app",
                "maestro-cli",
                "maestro-rpc",
                "maestro-chat",
                "maestro-web",
                "maestro-extensions-wasmtime",
            ],
        ),
    ];
}

#[allow(dead_code, reason = "Graph fixtures are unused by comment tests.")]
pub fn exact(name: &str) -> bool {
    matches!(
        name,
        "maestro-cli"
            | "maestro-chat"
            | "maestro-rpc"
            | "maestro-web"
            | "maestro-tui-crossterm"
            | "maestro-test-terminal"
    )
}

#[allow(dead_code, reason = "Graph fixtures are unused by comment tests.")]
pub fn class(name: &str) -> &'static str {
    if matches!(
        name,
        "maestro" | "maestro-test-conventions" | "maestro-test-terminal" | "maestro-tooling"
    ) {
        "dedicated"
    } else {
        "core"
    }
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
            let targets = policy::POLICY.iter().find(|row| row.0 == name).unwrap().1;
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
        use std::os::unix::fs::PermissionsExt;
        let path = self.root.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
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
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "metadata_graph_failures_cross_the_public_interface",
                "--nocapture",
            ])
            .env("MAESTRO_CONVENTIONS_PROBE_ROOT", &self.root)
            .env("MAESTRO_CONVENTIONS_EXPECT", expectation)
            .env("CARGO", cargo)
            .env("RUSTC", rustc)
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
