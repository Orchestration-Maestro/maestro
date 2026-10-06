#![cfg(unix)]
mod support;

use std::process::Command;
use support::Workspace;

fn tooling() -> &'static str {
    env!("CARGO_BIN_EXE_repository_tools")
}

fn git(workspace: &Workspace, args: &[&str]) -> std::process::Output {
    let output = Command::new("git")
        .current_dir(&workspace.root)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    output
}

#[test]
fn staged_paths_are_captured_before_formatting() {
    let workspace = Workspace::new();
    git(&workspace, &["init", "-q"]);
    let files = [
        "space name.rs",
        "tab\tname.rs",
        "line\nname.rs",
        "-leading.rs",
        "deleted.rs",
        "renamed.rs",
    ];
    for file in files {
        std::fs::write(workspace.root.join(file), "before").unwrap();
    }
    git(
        &workspace,
        &[
            "add",
            "--",
            "space name.rs",
            "tab\tname.rs",
            "line\nname.rs",
            "-leading.rs",
            "deleted.rs",
            "renamed.rs",
        ],
    );
    // Write a tree without requiring a commit hook or a real signing identity.
    let tree = git(&workspace, &["write-tree"]);
    let tree = String::from_utf8(tree.stdout).unwrap();
    git(&workspace, &["read-tree", tree.trim()]);
    git(&workspace, &["mv", "--", "renamed.rs", "new name.rs"]);
    std::fs::remove_file(workspace.root.join("deleted.rs")).unwrap();
    git(&workspace, &["add", "--", "deleted.rs"]);
    let cargo = workspace.command("formatter", "for file in *.rs; do printf 'formatted' > \"$file\"; done; printf 'new' > created.rs; git add -- created.rs; printf 'not staged' > created.rs");
    let output = Command::new(tooling())
        .arg("format-staged")
        .current_dir(&workspace.root)
        .env("CARGO", &cargo)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    for file in [
        "space name.rs",
        "tab\tname.rs",
        "line\nname.rs",
        "-leading.rs",
        "new name.rs",
    ] {
        assert_eq!(
            git(&workspace, &["show", &format!(":{file}")]).stdout,
            b"formatted"
        );
    }
    assert!(!workspace.root.join("deleted.rs").exists());
    assert_eq!(git(&workspace, &["show", ":created.rs"]).stdout, b"new");
    let empty = Workspace::new();
    git(&empty, &["init", "-q"]);
    let formatter = empty.command("format-empty", "printf 'new' > never-staged.rs");
    let output = Command::new(tooling())
        .arg("format-staged")
        .current_dir(&empty.root)
        .env("CARGO", formatter)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        git(&empty, &["diff", "--cached", "--name-only"])
            .stdout
            .is_empty()
    );
}

#[test]
fn cargo_runner_distinguishes_tests_docs_and_programs() {
    let workspace = Workspace::new();
    workspace.member("maestro-resources", "maestro-resources", "");
    std::fs::write(workspace.root.join("crates/maestro-resources/src/lib.rs"), "//! Resource fixture.\n//! ```\n//! assert!(std::env::var_os(\"GITHUB_TOKEN\").is_none()); assert!(std::env::var_os(\"DBUS_SESSION_BUS_ADDRESS\").is_none());\n//! assert_eq!(std::env::var(\"MAESTRO_NO_LOCAL_LLM\").unwrap(), \"1\");\n//! ```\n#[test] fn environment() { assert!(std::env::var_os(\"GITHUB_TOKEN\").is_none()); assert!(std::env::var_os(\"DBUS_SESSION_BUS_ADDRESS\").is_none()); assert_eq!(std::env::var(\"MAESTRO_NO_LOCAL_LLM\").unwrap(), \"1\"); }\n").unwrap();
    let scripts = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts")
        .canonicalize()
        .unwrap();
    std::fs::create_dir(workspace.root.join(".cargo")).unwrap();
    std::fs::write(
        workspace.root.join(".cargo/config.toml"),
        format!(
            "[build]\nrustdoc = {:?}\n[target.'cfg(unix)']\nrunner = [{:?}, \"cargo-target\"]\n",
            scripts.join("run-rustdoc.sh").to_str().unwrap(),
            scripts.join("run-isolated.sh").to_str().unwrap()
        ),
    )
    .unwrap();
    let output = fixture_cargo()
        .args(["test", "--workspace"])
        .current_dir(&workspace.root)
        .env("GITHUB_TOKEN", "canary")
        .env("CARGO_BUILD_JOBS", "3")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
    std::fs::write(workspace.root.join("crates/maestro-resources/src/lib.rs"), "//! Resource fixture.\n/// Cargo package version.\npub const VERSION: &str = env!(\"CARGO_PKG_VERSION\");\n").unwrap();
    let output = fixture_cargo()
        .args(["doc", "--workspace", "--no-deps"])
        .current_dir(&workspace.root)
        .env("CARGO_BUILD_JOBS", "3")
        .env("RUSTDOCFLAGS", "-D warnings -D missing_docs")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");

    std::fs::write(workspace.root.join("crates/maestro-resources/src/lib.rs"), "//! ```\n//! assert!(false);\n//! ```\n#[test] fn failing() { panic!(\"controlled failure\"); }\n").unwrap();
    let output = fixture_cargo()
        .args(["test", "--doc"])
        .current_dir(&workspace.root)
        .env("CARGO_BUILD_JOBS", "3")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("FAILED"));
}

fn fixture_cargo() -> Command {
    let capped = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|path| path.join("capped"))
        .find(|path| path.is_file());
    if let Some(capped) = capped {
        let uid = Command::new("id").arg("-u").output().unwrap();
        assert!(uid.status.success());
        let uid = String::from_utf8(uid.stdout).unwrap();
        let mut command = Command::new(capped);
        command.env(
            "DBUS_SESSION_BUS_ADDRESS",
            format!("unix:path=/run/user/{}/bus", uid.trim()),
        );
        command
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/run-isolated.sh"
            ))
            .args(["isolate", "cargo"]);
        command
    } else {
        Command::new("cargo")
    }
}

#[test]
fn per_case_deadlines_do_not_become_suite_deadlines() {
    let workspace = Workspace::new();
    workspace.member("maestro-models", "maestro-models", "");
    std::fs::write(workspace.root.join("crates/maestro-models/src/lib.rs"), "#[test] fn below_a() { std::thread::sleep(std::time::Duration::from_millis(750)); }\n#[test] fn below_b() { std::thread::sleep(std::time::Duration::from_millis(750)); }\n#[test] fn at_deadline() { std::thread::sleep(std::time::Duration::from_millis(1000)); }\n#[test] fn over_deadline() { println!(\"scratch={}\", std::env::var(\"HOME\").unwrap()); std::thread::sleep(std::time::Duration::from_millis(1200)); }\n").unwrap();
    let output = fixture_cargo()
        .args(["test", "--no-run", "--message-format=json"])
        .current_dir(&workspace.root)
        .env("CARGO_BUILD_JOBS", "3")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let executable = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find_map(|value| value["executable"].as_str().map(str::to_owned))
        .unwrap();
    let variant = workspace.root.join("tooling-clock-fixture");
    let output = Command::new("rustc")
        .args(["--edition=2024", "--cfg", "test"])
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/bin/repository_tools.rs"
        ))
        .arg("-o")
        .arg(&variant)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let output = Command::new(&variant)
        .args(["--root"])
        .arg(&workspace.root)
        .arg("cargo-target")
        .arg(&executable)
        .args(["below", "--test-threads=1"])
        .env("MAESTRO_FIXTURE_DEADLINE_MS", "1000")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("2 passed; 0 failed"));
    for name in ["at_deadline", "over_deadline"] {
        let output = Command::new(&variant)
            .args(["--root"])
            .arg(&workspace.root)
            .arg("cargo-target")
            .arg(&executable)
            .args([name, "--exact", "--nocapture"])
            .env("MAESTRO_FIXTURE_DEADLINE_MS", "1000")
            .output()
            .unwrap();
        assert!(!output.status.success(), "{name}");
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains(&format!("repository tools: test timed out: {name}")),
            "{output:?}"
        );
        if name == "over_deadline" {
            let text = String::from_utf8_lossy(&output.stdout);
            let home = text
                .lines()
                .find_map(|line| line.split_once("scratch=").map(|(_, path)| path))
                .unwrap();
            assert!(
                !std::path::Path::new(home).exists(),
                "timed-out scratch remains: {home}"
            );
        }
    }
    let output = Command::new(&variant)
        .arg("--root")
        .arg(&workspace.root)
        .args(["cargo-target", &executable, "--test-threads=2"])
        .env("MAESTRO_FIXTURE_DEADLINE_MS", "1000")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("2 passed; 2 failed; 0 ignored"));
}

fn recipe_fixture() -> Workspace {
    let workspace = Workspace::new();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    std::fs::copy(root.join("justfile"), workspace.root.join("justfile")).unwrap();
    std::fs::create_dir(workspace.root.join("scripts")).unwrap();
    let adapter = workspace.command(
        "adapter",
        &format!("exec {:?} \"$@\"", root.join("scripts/run-isolated.sh")),
    );
    std::os::unix::fs::symlink(adapter, workspace.root.join("scripts/run-isolated.sh")).unwrap();
    std::fs::create_dir(workspace.root.join("bin")).unwrap();
    let cargo = workspace.command("cargo-fixture", "printf '%s|%s\\n' \"$*\" \"${RUSTDOCFLAGS-}\" >> operations; case \"$*\" in *isolated_cli*--ignored*) printf '%s\\0' \"$@\" > isolated-ignored-arguments;; *--ignored*) printf '%s\\0' \"$@\" > ignored-arguments;; *) printf '%s\\0' \"$@\" > arguments;; esac; case \"$*\" in *\"${RUST_TEST_THREADS:-never-match}\"*) exit 27;; esac");
    std::os::unix::fs::symlink(cargo, workspace.root.join("bin/cargo")).unwrap();
    workspace
}

fn recipe(workspace: &Workspace, name: &str, failure: &str) -> std::process::Output {
    let path = std::env::join_paths(
        std::iter::once(workspace.root.join("bin"))
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    Command::new("just")
        .arg(name)
        .current_dir(&workspace.root)
        .env("PATH", path)
        .env("RUST_TEST_THREADS", failure)
        .output()
        .unwrap()
}

#[test]
#[ignore = "requires the repository's pinned just/prek; runs in just test until shared CI provisions pinned tools"]
fn workspace_recipes_keep_build_check_test_and_prepublish_order() {
    let workspace = recipe_fixture();
    for (name, expected) in [
        ("build", "build --workspace --locked|\n"),
        (
            "check",
            "fmt --all --check|\nclippy --workspace --all-targets --locked -- -D warnings|\ndoc --workspace --no-deps --locked|-D warnings -D missing_docs\ntest -p maestro-test-conventions --locked|\ntest -p maestro-test-conventions --test build_recipes --locked -- --ignored|\ntest -p maestro-test-conventions --test isolated_cli --locked -- --ignored|\n",
        ),
        (
            "test",
            "test --workspace --locked|\ntest -p maestro-test-conventions --test build_recipes --locked -- --ignored|\ntest -p maestro-test-conventions --test isolated_cli --locked -- --ignored|\n",
        ),
        (
            "prepublish",
            "clean|\nbuild --workspace --locked|\nfmt --all --check|\nclippy --workspace --all-targets --locked -- -D warnings|\ndoc --workspace --no-deps --locked|-D warnings -D missing_docs\ntest -p maestro-test-conventions --locked|\ntest -p maestro-test-conventions --test build_recipes --locked -- --ignored|\ntest -p maestro-test-conventions --test isolated_cli --locked -- --ignored|\n",
        ),
    ] {
        let _ = std::fs::remove_file(workspace.root.join("operations"));
        let output = recipe(&workspace, name, "never-match");
        assert!(output.status.success(), "{name}: {output:?}");
        assert_eq!(
            std::fs::read_to_string(workspace.root.join("operations")).unwrap(),
            expected
        );
    }
    for failure in ["build", "fmt", "clippy", "doc", "test -p"] {
        let _ = std::fs::remove_file(workspace.root.join("operations"));
        let output = recipe(&workspace, "prepublish", failure);
        assert!(!output.status.success(), "{failure}");
        let operations = std::fs::read_to_string(workspace.root.join("operations")).unwrap();
        assert!(operations.lines().last().unwrap().contains(failure));
    }
    let _ = std::fs::remove_file(workspace.root.join("operations"));
    assert!(recipe(&workspace, "ci", "never-match").status.success());
    assert!(
        std::fs::read_to_string(workspace.root.join("operations"))
            .unwrap()
            .ends_with("test -p maestro-test-conventions --locked|\ntest -p maestro-test-conventions --test build_recipes --locked -- --ignored|\ntest -p maestro-test-conventions --test isolated_cli --locked -- --ignored|\ntest --workspace --locked|\ntest -p maestro-test-conventions --test build_recipes --locked -- --ignored|\ntest -p maestro-test-conventions --test isolated_cli --locked -- --ignored|\n")
    );
    let path = std::env::join_paths(
        std::iter::once(workspace.root.join("bin"))
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let output = Command::new("just")
        .args(["test", "literal filter", "", "--", "--exact"])
        .current_dir(&workspace.root)
        .env("PATH", path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        std::fs::read(workspace.root.join("arguments")).unwrap(),
        b"test\0--workspace\0--locked\0literal filter\0\0--\0--exact\0"
    );
    assert_eq!(
        std::fs::read(workspace.root.join("ignored-arguments")).unwrap(),
        b"test\0-p\0maestro-test-conventions\0--test\0build_recipes\0--locked\0--\0--ignored\0"
    );
    assert_eq!(
        std::fs::read(workspace.root.join("isolated-ignored-arguments")).unwrap(),
        b"test\0-p\0maestro-test-conventions\0--test\0isolated_cli\0--locked\0--\0--ignored\0"
    );
}

#[test]
fn clean_rebuild_preserves_sources_and_unrelated_outputs() {
    let workspace = Workspace::new();
    workspace.member("maestro-resources", "maestro-resources", "");
    let source = workspace.root.join("crates/maestro-resources/src/lib.rs");
    std::fs::write(&source, "pub fn value() -> u8 { 9 }\n").unwrap();
    std::fs::write(workspace.root.join("unknown-output"), "keep").unwrap();
    let sibling = Workspace::new();
    std::fs::write(sibling.root.join("output"), "keep").unwrap();
    for operation in ["build", "clean", "build"] {
        let output = fixture_cargo()
            .arg(operation)
            .current_dir(&workspace.root)
            .env("CARGO_BUILD_JOBS", "3")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            workspace
                .root
                .join("target/debug/libmaestro_resources.rlib")
                .exists(),
            operation == "build"
        );
        assert_eq!(
            std::fs::read_to_string(&source).unwrap(),
            "pub fn value() -> u8 { 9 }\n"
        );
        assert_eq!(
            std::fs::read_to_string(workspace.root.join("unknown-output")).unwrap(),
            "keep"
        );
        assert_eq!(
            std::fs::read_to_string(sibling.root.join("output")).unwrap(),
            "keep"
        );
    }
}

#[test]
fn dev_watch_rebuilds_changed_sources_without_self_triggering() {
    use std::io::BufRead;
    let workspace = Workspace::new();
    std::fs::create_dir(workspace.root.join("src")).unwrap();
    std::fs::create_dir(workspace.root.join("target")).unwrap();
    let cargo = workspace.command("builder", "printf 'build\\n'; if test -f fail; then exit 29; fi; if test -f hold; then while test -f hold; do sleep 0.01; done; fi; printf 'generated' > target/output; printf 'done\\n'");
    let watcher = std::path::Path::new(tooling()).with_file_name("dev_watch");
    assert!(watcher.is_file(), "missing persistent watcher");
    let mut child = Command::new(tooling())
        .arg("isolate")
        .arg(&watcher)
        .arg(&cargo)
        .current_dir(&workspace.root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    let stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines() {
            send.send(line.unwrap()).unwrap();
        }
    });
    let line = || {
        receive
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap()
    };
    assert_eq!(line(), "build");
    assert_eq!(line(), "done");
    std::fs::write(workspace.root.join("fail"), "").unwrap();
    std::fs::write(workspace.root.join("src/input.rs"), "first").unwrap();
    assert_eq!(line(), "build");
    std::fs::remove_file(workspace.root.join("fail")).unwrap();
    std::fs::write(workspace.root.join("hold"), "").unwrap();
    std::fs::write(workspace.root.join("src/input.rs"), "second").unwrap();
    assert_eq!(line(), "build");
    std::fs::write(workspace.root.join("Cargo.toml"), "changed config").unwrap();
    std::fs::remove_file(workspace.root.join("hold")).unwrap();
    assert_eq!(line(), "done");
    assert_eq!(line(), "build");
    assert_eq!(line(), "done");
    assert!(
        receive
            .recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "generated artifacts triggered a build"
    );
    Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap();
    assert_eq!(child.wait().unwrap().code(), Some(143));
    reader.join().unwrap();
}

#[test]
fn tool_pins_and_hooks_keep_their_existing_checks() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mise = std::fs::read_to_string(root.join("mise.toml")).unwrap();
    assert!(mise.contains("just = \"1.58.0\""));
    assert!(mise.contains("version = \"0.5.4\""));
    assert!(!mise.contains("jaq"));
    assert!(
        !std::fs::read_to_string(root.join("mise.lock"))
            .unwrap()
            .contains("jaq")
    );
    assert!(
        std::fs::read_to_string(root.join("rust-toolchain.toml"))
            .unwrap()
            .contains("1.98.0")
    );
    let hooks = std::fs::read_to_string(root.join(".pre-commit-config.yaml")).unwrap();
    for hook in [
        "check-merge-conflict",
        "check-yaml",
        "check-toml",
        "conventional-commit-header",
        "eighty-columns",
    ] {
        assert!(hooks.contains(hook));
    }
    assert!(
        std::fs::read_to_string(root.join("rustfmt.toml"))
            .unwrap()
            .contains("edition = \"2024\"")
    );
}

#[test]
#[ignore = "requires the repository's pinned just/prek; runs in just test until shared CI provisions pinned tools"]
fn hook_failures_keep_messages_and_stop_the_commit() {
    let workspace = recipe_fixture();
    git(&workspace, &["init", "-q"]);
    std::fs::write(workspace.root.join("staged.rs"), "before").unwrap();
    git(&workspace, &["add", "--", "staged.rs"]);
    for failure in ["fmt", "clippy", "never-match"] {
        let output = recipe(&workspace, "pre-commit", failure);
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(
            text.starts_with("Running formatting, linting, and type checking...\n"),
            "{text:?}"
        );
        if failure == "never-match" {
            assert!(output.status.success(), "{output:?}");
            assert!(text.ends_with("✅ All pre-commit checks passed!\n"));
        } else {
            assert!(!output.status.success());
            assert!(text.ends_with("❌ Checks failed. Please fix the errors before committing.\n"));
            assert!(!text.contains("✅"));
        }
        assert!(!text.contains("Running tests without API keys..."));
        assert!(!text.contains("Running browser smoke check..."));
    }
    std::fs::create_dir(workspace.root.join(".github")).unwrap();
    std::fs::write(
        workspace.root.join(".github/ci.toml"),
        "browser_build = true\n",
    )
    .unwrap();
    let output = recipe(&workspace, "pre-commit", "never-match");
    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.ends_with("Running browser smoke check...\n❌ Browser smoke check failed.\n"),
        "{text}"
    );
    assert!(!text.contains("✅"));
}

#[test]
#[ignore = "requires the repository's pinned just/prek; runs in just test until shared CI provisions pinned tools"]
fn syntax_and_commit_hooks_reject_invalid_inputs() {
    let workspace = Workspace::new();
    git(&workspace, &["init", "-q"]);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::copy(
        root.join(".pre-commit-config.yaml"),
        workspace.root.join(".pre-commit-config.yaml"),
    )
    .unwrap();
    for (hook, file, invalid, valid) in [
        ("check-yaml", "sample.yaml", "bad: [\n", "good: true\n"),
        ("check-toml", "sample.toml", "bad = [\n", "good = true\n"),
        (
            "check-merge-conflict",
            "sample.rs",
            "<<<<<<< ours\n=======\n>>>>>>> theirs\n",
            "fn main() {}\n",
        ),
    ] {
        for (contents, passes) in [(invalid, false), (valid, true)] {
            std::fs::write(workspace.root.join(file), contents).unwrap();
            git(&workspace, &["add", "--", file]);
            let output = Command::new("prek")
                .args(["run", hook, "--files", file])
                .current_dir(&workspace.root)
                .output()
                .unwrap();
            assert_eq!(output.status.success(), passes, "{hook}: {output:?}");
        }
    }
    for (contents, passes) in [
        ("not conventional\n".to_owned(), false),
        (format!("feat: {}\n", "x".repeat(81)), false),
        (
            "feat: check controlled inputs\n\nA valid body.\n".to_owned(),
            true,
        ),
    ] {
        std::fs::write(workspace.root.join("message"), contents).unwrap();
        let mut success = true;
        for hook in ["conventional-commit-header", "eighty-columns"] {
            let output = Command::new("prek")
                .args([
                    "run",
                    hook,
                    "--hook-stage",
                    "commit-msg",
                    "--commit-msg-filename",
                    "message",
                ])
                .current_dir(&workspace.root)
                .output()
                .unwrap();
            success &= output.status.success();
        }
        assert_eq!(success, passes);
    }
}

fn compile_tests(workspace: &Workspace, owner: &str, source: &str) -> String {
    workspace.member(owner, owner, "");
    std::fs::write(
        workspace.root.join("crates").join(owner).join("src/lib.rs"),
        source,
    )
    .unwrap();
    let output = fixture_cargo()
        .args(["test", "--no-run", "--message-format=json"])
        .current_dir(&workspace.root)
        .env("CARGO_BUILD_JOBS", "3")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find_map(|value| value["executable"].as_str().map(str::to_owned))
        .unwrap()
}

#[test]
fn test_selection_preserves_filters_ignores_and_counts() {
    let workspace = Workspace::new();
    let executable = compile_tests(
        &workspace,
        "maestro-models",
        "#[test] fn pass_a() {}\n#[test] fn pass_b() {}\n#[test] #[ignore = \"controlled reason\"] fn ignored() {}\n#[test] fn failing() { assert!(false); }\n#[test] fn panicking() { panic!(\"fixture panic\"); }\n",
    );
    for (args, passed, failed, ignored, filtered) in [
        (vec!["pass"], 2, 0, 0, 3),
        (vec!["pass_a", "--exact"], 1, 0, 0, 4),
        (
            vec![
                "pass_a",
                "pass_b",
                "--skip",
                "pass_b",
                "--skip",
                "panicking",
            ],
            1,
            0,
            0,
            4,
        ),
        (vec!["--ignored"], 1, 0, 0, 4),
        (vec!["--include-ignored"], 3, 2, 0, 0),
        (vec!["no_matches"], 0, 0, 0, 5),
        (vec![], 2, 2, 1, 0),
    ] {
        let output = Command::new(tooling())
            .args(["--root"])
            .arg(&workspace.root)
            .args(["cargo-target", &executable])
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(output.status.success(), failed == 0, "{args:?}: {output:?}");
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains(&format!("{passed} passed; {failed} failed; {ignored} ignored; 0 measured; {filtered} filtered out")), "{args:?}: {text}");
        if ignored != 0 {
            assert!(text.contains("controlled reason"), "{text}");
        }
    }
    let output = Command::new(tooling())
        .arg("--root")
        .arg(&workspace.root)
        .args(["cargo-target", &executable, "--list"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("5 tests, 0 benchmarks"));
    let terminal = Workspace::new();
    let executable = compile_tests(
        &terminal,
        "maestro-tui",
        "#[test] fn wrap_ansi() {}\n#[test] fn other_terminal_case() {}\n",
    );
    let output = Command::new(tooling())
        .arg("--root")
        .arg(&terminal.root)
        .args(["cargo-target", &executable])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("2 passed; 0 failed"));
}

#[test]
fn package_manifests_keep_scripts_and_asset_inventories() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("distribution/npm/package.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["private"], true);
    assert_eq!(manifest["scripts"]["prepublishOnly"], "just prepublish");
    for owner in ["models", "agent", "app", "tui"] {
        let path = root.join(format!("distribution/npm/maestro-{owner}/package.json"));
        let package: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(package["private"], true);
        assert_eq!(
            package["name"],
            format!("@orchestration-maestro/maestro-{owner}")
        );
        for script in ["clean", "build", "dev", "test", "prepublishOnly"] {
            assert!(package["scripts"][script].is_string(), "{owner}/{script}");
        }
        assert!(package["exports"].is_null());
        assert_eq!(
            package["files"],
            if owner == "app" {
                serde_json::json!(["dist", "README.md", "docs", "examples", "CHANGELOG.md"])
            } else {
                serde_json::json!(["dist", "README.md"])
            }
        );
        if owner == "app" {
            assert_eq!(
                package["maestroAssets"]["library"],
                serde_json::json!([
                    "src/modes/interactive/theme/*.json",
                    "src/modes/interactive/assets/*.png",
                    "src/core/export-html/template.html",
                    "src/core/export-html/template.css",
                    "src/core/export-html/template.js",
                    "src/core/export-html/vendor/*.js"
                ])
            );
            assert_eq!(
                package["maestroAssets"]["standalone"],
                serde_json::json!([
                    "package.json",
                    "README.md",
                    "CHANGELOG.md",
                    "src/modes/interactive/theme/*.json",
                    "src/modes/interactive/assets/*.png",
                    "src/core/export-html/template.html",
                    "src/core/export-html/vendor/*.js",
                    "docs/**",
                    "examples/**"
                ])
            );
        }

        if ["models", "agent"].contains(&owner) {
            assert!(
                root.join(format!("crates/maestro-{owner}/src/lib.rs"))
                    .is_file()
            );
        } else {
            assert!(package["maestroActivation"]["owner"].is_string());
        }
    }
    let sources = [
        "package.json",
        "README.md",
        "CHANGELOG.md",
        "src/modes/interactive/theme/dark.json",
        "src/modes/interactive/assets/icon.png",
        "src/core/export-html/template.html",
        "src/core/export-html/template.css",
        "src/core/export-html/template.js",
        "src/core/export-html/vendor/vendor.js",
        "docs/nested/help.md",
        "examples/nested/example.rs",
    ];
    for layout in ["library", "standalone"] {
        let workspace = Workspace::new();
        for source in sources {
            let path = workspace.root.join(source);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, source.as_bytes()).unwrap();
        }
        let destination = workspace.root.join("output");
        let output = Command::new(tooling())
            .args(["copy-assets", layout])
            .arg(&workspace.root)
            .arg(&destination)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let expected: &[(&str, &str)] = if layout == "library" {
            &[
                (
                    "src/modes/interactive/theme/dark.json",
                    "modes/interactive/theme/dark.json",
                ),
                (
                    "src/modes/interactive/assets/icon.png",
                    "modes/interactive/assets/icon.png",
                ),
                (
                    "src/core/export-html/template.html",
                    "core/export-html/template.html",
                ),
                (
                    "src/core/export-html/template.css",
                    "core/export-html/template.css",
                ),
                (
                    "src/core/export-html/template.js",
                    "core/export-html/template.js",
                ),
                (
                    "src/core/export-html/vendor/vendor.js",
                    "core/export-html/vendor/vendor.js",
                ),
            ]
        } else {
            &[
                ("package.json", "package.json"),
                ("README.md", "README.md"),
                ("CHANGELOG.md", "CHANGELOG.md"),
                ("src/modes/interactive/theme/dark.json", "theme/dark.json"),
                ("src/modes/interactive/assets/icon.png", "assets/icon.png"),
                (
                    "src/core/export-html/template.html",
                    "export-html/template.html",
                ),
                (
                    "src/core/export-html/vendor/vendor.js",
                    "export-html/vendor/vendor.js",
                ),
                ("docs/nested/help.md", "docs/nested/help.md"),
                ("examples/nested/example.rs", "examples/nested/example.rs"),
            ]
        };
        for (source, target) in expected {
            assert_eq!(
                std::fs::read(destination.join(target)).unwrap(),
                source.as_bytes()
            );
        }
        if layout == "standalone" {
            assert!(!destination.join("export-html/template.css").exists());
            assert!(!destination.join("export-html/template.js").exists());
        }
        std::fs::remove_file(workspace.root.join("src/core/export-html/template.html")).unwrap();
        let output = Command::new(tooling())
            .args(["copy-assets", layout])
            .arg(&workspace.root)
            .arg(&destination)
            .output()
            .unwrap();
        assert!(!output.status.success());
    }
}

#[test]
fn ci_declaration_keeps_all_targets_and_owner_entries() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new("python3")
        .args([
            "-c",
            "import json,sys,tomllib; print(json.dumps(tomllib.load(open(sys.argv[1], 'rb'))))",
        ])
        .arg(root.join(".github/ci.toml"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["check_targets"],
        serde_json::json!([
            "aarch64-unknown-linux-gnu",
            "x86_64-apple-darwin",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc"
        ])
    );
    assert_eq!(value["browser_build"], false);
    for owner in value["wasm_crates"].as_array().unwrap() {
        assert!(
            root.join("crates")
                .join(owner.as_str().unwrap())
                .join("Cargo.toml")
                .is_file()
        );
    }
}

#[test]
#[ignore = "requires the repository's pinned just/prek; runs in just test until shared CI provisions pinned tools"]
fn pending_hooks_have_explicit_owners_not_fake_success() {
    let workspace = recipe_fixture();
    for (hook, owner) in [
        ("browser-smoke", "#113"),
        ("generate-models", "#135"),
        ("component-build", "component author"),
        ("examples-check", "application"),
        ("profile-tui", "#167"),
        ("profile-rpc", "#167"),
        ("binary", "#111"),
        ("assets", "application"),
        ("binary-assets", "application"),
    ] {
        let output = recipe(&workspace, hook, "never-match");
        assert!(!output.status.success());
        let text = String::from_utf8_lossy(&output.stderr);
        assert!(
            text.contains(&format!(
                "repository tools: inactive hook: {hook}; owner: {owner}"
            )),
            "{text}"
        );
        assert!(!workspace.root.join("operations").exists());
    }
}

#[test]
#[ignore = "requires the repository's pinned just/prek; runs in just test until shared CI provisions pinned tools"]
fn development_text_keeps_every_section_and_paragraph() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (page, expected) in [
        (
            "development.md",
            include_str!("support/development.expected.md"),
        ),
        (
            "rust_build.md",
            include_str!("support/rust_build.expected.md"),
        ),
    ] {
        assert_eq!(
            std::fs::read_to_string(root.join("docs").join(page)).unwrap(),
            expected
        );
    }
    for path in [
        "AGENTS.md",
        "scripts/run-source.sh",
        "scripts/run-source.ps1",
        "distribution/npm/maestro-app/package.json",
        ".github/ci.toml",
    ] {
        assert!(root.join(path).is_file());
    }
    let output = Command::new("just")
        .arg("--list")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(output.status.success());
    let recipes = String::from_utf8_lossy(&output.stdout);
    for name in [
        "setup",
        "build",
        "test",
        "check",
        "ci",
        "prepublish",
        "dev",
        "browser-smoke",
        "generate-models",
        "component-build",
        "examples-check",
        "profile-tui",
        "profile-rpc",
        "binary",
        "assets",
        "binary-assets",
    ] {
        assert!(recipes.contains(name), "{name}");
    }
}
