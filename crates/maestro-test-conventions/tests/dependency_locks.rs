#![cfg(test)]

use crate::support::Scratch;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cargo_metadata::{DependencyKind, Metadata, MetadataCommand};

/// Resolve the complete delivered workspace without rewriting its owning inputs.
#[test]
fn maestro_locked_resolution_keeps_checkout_clean() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tracked = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "Cargo.lock",
            "Cargo.toml",
            "**/Cargo.toml",
        ])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(tracked.status.success());
    let paths: Vec<_> = tracked
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| root.join(std::str::from_utf8(path).unwrap()))
        .collect();
    assert!(paths.contains(&root.join("Cargo.lock")));
    let before: Vec<_> = paths.iter().map(|path| fs::read(path).unwrap()).collect();
    let mut command = MetadataCommand::new();
    command
        .current_dir(&root)
        .other_options(vec!["--offline".into(), "--locked".into()]);
    let cold = command.exec().unwrap();
    let cached = command.exec().unwrap();
    assert!(!cold.packages.is_empty());
    assert!(
        cold.resolve
            .as_ref()
            .is_some_and(|graph| !graph.nodes.is_empty())
    );
    assert_eq!(cold.packages, cached.packages);
    assert_eq!(cold.resolve, cached.resolve);
    for (path, bytes) in paths.iter().zip(before) {
        assert_eq!(
            fs::read(path).unwrap(),
            bytes,
            "owning input changed: {}",
            path.display()
        );
    }
}

/// The shipped binary's production and build graph never enables tokio's test clock.
#[test]
fn shipped_binary_tokio_has_no_test_util() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let tree = Command::new(cargo)
        .args([
            "tree",
            "--locked",
            "--offline",
            "-p",
            "maestro",
            "--target",
            "all",
            "-e",
            "features,normal,build",
            "-i",
            "tokio",
        ])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        tree.status.success(),
        "{}",
        String::from_utf8_lossy(&tree.stderr)
    );
    let tree = String::from_utf8(tree.stdout).unwrap();
    assert!(
        tree.lines()
            .any(|line| line.contains("tokio feature \"rt\""))
    );
    let leaked: Vec<_> = tree
        .lines()
        .filter(|line| line.contains("tokio feature \"test-util\""))
        .collect();
    assert!(
        leaked.is_empty(),
        "tokio test-util reaches production: {leaked:?}"
    );
}

/// Production sources under `dir`, in the order found, excluding unit-test files.
fn production_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "tests" {
                production_sources(&path, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") && name != "tests.rs" {
            out.push(path);
        }
    }
}

/// Every member whose production code calls a tokio macro declares tokio's `macros` feature.
#[test]
fn members_declare_tokio_macros_where_production_code_uses_them() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let metadata = MetadataCommand::new()
        .current_dir(&root)
        .other_options(vec!["--offline".into(), "--locked".into()])
        .exec()
        .unwrap();
    let mut undeclared = Vec::new();
    for package in metadata.workspace_packages() {
        let declared: Vec<&String> = package
            .dependencies
            .iter()
            .filter(|dep| dep.name == "tokio" && dep.kind == DependencyKind::Normal)
            .flat_map(|dep| &dep.features)
            .collect();
        if declared.is_empty() || declared.iter().any(|feature| *feature == "macros") {
            continue;
        }
        let mut sources = Vec::new();
        production_sources(
            package
                .manifest_path
                .parent()
                .unwrap()
                .join("src")
                .as_std_path(),
            &mut sources,
        );
        for path in sources {
            let text = fs::read_to_string(&path).unwrap();
            let production = text.split("#[cfg(test)]").next().unwrap();
            if production.lines().any(|line| {
                !line.trim_start().starts_with("//")
                    && ["select", "join", "try_join", "pin"]
                        .iter()
                        .any(|name| line.contains(&format!("tokio::{name}!")))
            }) {
                undeclared.push(format!("{}: {}", package.name, path.display()));
            }
        }
    }
    assert!(
        undeclared.is_empty(),
        "tokio macros used without the `macros` feature: {undeclared:#?}"
    );
}

/// A local-only resolver workspace with child-owned Cargo state.
struct ResolverFixture {
    /// Owns cleanup after every child exits.
    scratch: Scratch,
    /// The compiler selected before entering the fixture's fresh Cargo home.
    rustc: PathBuf,
    /// Host projection read from that compiler.
    host: String,
}

impl ResolverFixture {
    /// Create ordinary, build, development, optional and inactive-target inputs.
    fn new() -> Self {
        let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let sysroot = Command::new(&compiler)
            .args(["--print", "sysroot"])
            .output()
            .unwrap();
        assert!(sysroot.status.success());
        let rustc = Path::new(std::str::from_utf8(&sysroot.stdout).unwrap().trim())
            .join("bin")
            .join(format!("rustc{}", std::env::consts::EXE_SUFFIX));
        let version = Command::new(&rustc).arg("-vV").output().unwrap();
        assert!(version.status.success());
        let host = std::str::from_utf8(&version.stdout)
            .unwrap()
            .lines()
            .find_map(|line| line.strip_prefix("host: "))
            .unwrap()
            .to_owned();
        let scratch = Scratch::new();
        let inactive = if cfg!(windows) { "unix" } else { "windows" };
        fs::write(
            scratch.root.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"app\"]\nexclude = [\"deps/*\"]\nresolver = \"3\"\n",
        )
        .unwrap();
        for kind in ["ordinary", "build", "development", "optional", "foreign"] {
            let path = scratch.root.path().join("deps").join(kind);
            fs::create_dir_all(path.join("src")).unwrap();
            fs::write(path.join("src/lib.rs"), "").unwrap();
            fs::write(path.join("Cargo.toml"), format!(
                "[package]\nname = \"maestro-lock-{kind}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[features]\nmarker = []\n")).unwrap();
        }
        fs::create_dir_all(scratch.root.path().join("app/src")).unwrap();
        fs::write(scratch.root.path().join("app/src/lib.rs"), "").unwrap();
        fs::write(
            scratch.root.path().join("app/Cargo.toml"),
            format!(
                r#"[package]
name = "maestro-lock-fixture"
version = "0.1.0"
edition = "2024"
[dependencies]
maestro-lock-ordinary = {{ path = "../deps/ordinary", features = ["marker"] }}
maestro-lock-optional = {{ path = "../deps/optional", optional = true }}
[features]
extra = ["dep:maestro-lock-optional"]
[build-dependencies]
maestro-lock-build = {{ path = "../deps/build" }}
[dev-dependencies]
maestro-lock-development = {{ path = "../deps/development" }}
[target.'cfg({inactive})'.dependencies]
maestro-lock-foreign = {{ path = "../deps/foreign" }}
"#
            ),
        )
        .unwrap();
        Self {
            scratch,
            rustc,
            host,
        }
    }

    /// Prepare a child with isolated cache state and the selected compiler.
    fn cargo(&self) -> Command {
        let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
        command
            .current_dir(self.scratch.root.path())
            .env("CARGO_HOME", self.scratch.root.path().join("cargo-home"))
            .env("RUSTC", &self.rustc);
        command
    }

    /// Generate native lock data without a registry or compilation.
    fn lock(&self) {
        let output = self
            .cargo()
            .args(["generate-lockfile", "--offline"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Configure typed full metadata, with independently selectable feature and target views.
    fn metadata(&self, host_only: bool, features: &[String]) -> MetadataCommand {
        let mut command = MetadataCommand::new();
        let mut options = vec!["--offline".into(), "--locked".into()];
        if host_only {
            options.extend(["--filter-platform".into(), self.host.clone()]);
        }
        if !features.is_empty() {
            options.extend(["--features".into(), features.join(",")]);
        }
        command
            .current_dir(self.scratch.root.path())
            .env("CARGO_HOME", self.scratch.root.path().join("cargo-home"))
            .env("RUSTC", &self.rustc)
            .other_options(options);
        command
    }
}

/// Compare real resolved edges, including dependency kind and target, plus the named feature.
fn assert_edges(metadata: &Metadata, expected: &[&str]) {
    let graph = metadata.resolve.as_ref().unwrap();
    let app = graph
        .nodes
        .iter()
        .find(|node| metadata[&node.id].name == "maestro-lock-fixture")
        .unwrap();
    let mut names: Vec<_> = app
        .deps
        .iter()
        .map(|edge| {
            let name = metadata[&edge.pkg].name.as_str();
            assert_eq!(edge.dep_kinds.len(), 1);
            let kind = &edge.dep_kinds[0];
            let expected_kind = match name {
                "maestro-lock-build" => cargo_metadata::DependencyKind::Build,
                "maestro-lock-development" => cargo_metadata::DependencyKind::Development,
                _ => cargo_metadata::DependencyKind::Normal,
            };
            assert_eq!(kind.kind, expected_kind);
            assert_eq!(kind.target.is_some(), name == "maestro-lock-foreign");
            name.strip_prefix("maestro-lock-").unwrap()
        })
        .collect();
    names.sort_unstable();
    assert_eq!(names, expected);
    let ordinary = graph
        .nodes
        .iter()
        .find(|node| metadata[&node.id].name == "maestro-lock-ordinary")
        .unwrap();
    assert_eq!(ordinary.features, ["marker"]);
}

/// Locks retain inactive declarations while graph views independently select their edges.
#[test]
fn maestro_locked_graph_keeps_inactive_dependencies() {
    let fixture = ResolverFixture::new();
    fixture.lock();
    let cold = fixture.metadata(false, &[]).exec().unwrap();
    let cached = fixture.metadata(false, &[]).exec().unwrap();
    assert_eq!(cold.packages, cached.packages);
    assert_eq!(cold.resolve, cached.resolve);
    let lock: toml::Table = fs::read_to_string(fixture.scratch.root.path().join("Cargo.lock"))
        .unwrap()
        .parse()
        .unwrap();
    let packages = lock["package"].as_array().unwrap();
    for kind in ["ordinary", "build", "development", "optional", "foreign"] {
        let name = format!("maestro-lock-{kind}");
        let package = packages
            .iter()
            .find(|package| package["name"].as_str() == Some(&name))
            .unwrap();
        assert_eq!(package["version"].as_str(), Some("0.1.0"));
    }
    assert_edges(&cold, &["build", "development", "foreign", "ordinary"]);
    let host = fixture.metadata(true, &[]).exec().unwrap();
    assert_edges(&host, &["build", "development", "ordinary"]);
    let enabled = fixture.metadata(false, &["extra".into()]).exec().unwrap();
    assert_edges(
        &enabled,
        &["build", "development", "foreign", "optional", "ordinary"],
    );
    let host_enabled = fixture.metadata(true, &["extra".into()]).exec().unwrap();
    assert_edges(
        &host_enabled,
        &["build", "development", "optional", "ordinary"],
    );
}

/// Locked requests reject stale inactive declarations and never repair or create lock data.
#[test]
fn maestro_locked_resolution_rejects_stale_inputs() {
    let fixture = ResolverFixture::new();
    fixture.lock();
    let lock_path = fixture.scratch.root.path().join("Cargo.lock");
    let original = fs::read(&lock_path).unwrap();
    let baseline = fixture.metadata(true, &[]).exec().unwrap();
    for kind in ["ordinary", "build", "development", "optional", "foreign"] {
        let manifest = fixture
            .scratch
            .root
            .path()
            .join("deps")
            .join(kind)
            .join("Cargo.toml");
        let bytes = fs::read_to_string(&manifest).unwrap();
        fs::write(&manifest, bytes.replace("0.1.0", "0.1.1")).unwrap();
        let output = fixture
            .metadata(true, &[])
            .cargo_command()
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(101), "stale {kind}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("cannot update the lock file"),
            "stale {kind}: {stderr}"
        );
        assert!(stderr.contains("--locked"));
        assert_eq!(fs::read(&lock_path).unwrap(), original);
        fs::write(&manifest, bytes).unwrap();
        assert_restored(&fixture, &baseline, &original);
    }
    fs::remove_file(&lock_path).unwrap();
    let output = fixture
        .metadata(false, &[])
        .cargo_command()
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(101));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("cannot create the lock file"), "{stderr}");
    assert!(stderr.contains("--locked"));
    assert!(!lock_path.exists());
    fs::write(&lock_path, &original).unwrap();
    assert_restored(&fixture, &baseline, &original);
}

/// A repaired input recovers its original graph without updating the lock.
fn assert_restored(fixture: &ResolverFixture, baseline: &Metadata, original: &[u8]) {
    let restored = fixture.metadata(true, &[]).exec().unwrap();
    assert_eq!(restored.packages, baseline.packages);
    assert_eq!(restored.resolve, baseline.resolve);
    assert_eq!(
        fs::read(fixture.scratch.root.path().join("Cargo.lock")).unwrap(),
        original
    );
}
