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
fn workspace_recipes_keep_build_check_test_and_prepublish_order() {
    let workspace = recipe_fixture();
    for (name, expected) in [
        ("build", "build --workspace --locked|\n"),
        (
            "check",
            "fmt --all --check|\nclippy --workspace --all-targets --locked -- -D warnings|\ndoc --workspace --no-deps --locked|-D warnings -D missing_docs\ntest -p maestro-test-conventions --locked|\n",
        ),
        ("test", "test --workspace --locked|\n"),
        (
            "prepublish",
            "clean|\nbuild --workspace --locked|\nfmt --all --check|\nclippy --workspace --all-targets --locked -- -D warnings|\ndoc --workspace --no-deps --locked|-D warnings -D missing_docs\ntest -p maestro-test-conventions --locked|\n",
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
            .ends_with("test -p maestro-test-conventions --locked|\ntest --workspace --locked|\n")
    );
    assert!(!workspace.root.join("ignored-arguments").exists());
    assert!(!workspace.root.join("isolated-ignored-arguments").exists());
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
    assert!(!workspace.root.join("ignored-arguments").exists());
    assert!(!workspace.root.join("isolated-ignored-arguments").exists());
    selected_test_recipes_never_replay_unrequested_cases();
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
    let cargo = workspace.command("builder", "if test \"$1\" = metadata; then printf '{\"target_directory\":\"%s/target\",\"packages\":[{\"id\":\"watch-fixture\",\"name\":\"watch-fixture\",\"source\":null,\"manifest_path\":\"%s/Cargo.toml\"}],\"workspace_members\":[\"watch-fixture\"],\"resolve\":{\"nodes\":[{\"id\":\"watch-fixture\",\"dependencies\":[]}]}}\\n' \"$PWD\" \"$PWD\"; exit 0; fi; if test -f fail; then printf 'build\\n'; exit 29; fi; printf 'build\\n'; if test -f hold; then while test -f hold; do sleep 0.01; done; fi; printf 'generated' > target/output; printf 'done\\n'");
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
    git(&workspace, &["add", "--", "Cargo.toml"]);
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
    assert_eq!(manifest["scripts"]["dev"], "just dev");
    assert_eq!(manifest["scripts"]["dev:tsc"], "just dev-tsc");
    let recipes = std::fs::read_to_string(root.join("justfile")).unwrap();
    assert!(recipes.contains("dev:\n    {{tooling}} isolate cargo run -p maestro-test-conventions --bin dev_watch --locked -- cargo --package maestro-models --package maestro-agent"));
    assert!(recipes.contains("dev-tsc:\n    {{tooling}} isolate cargo run -p maestro-test-conventions --bin dev_watch --locked -- cargo --package maestro-models\n"));
    assert!(recipes.contains("dev-package package:\n    {{tooling}} isolate cargo run -p maestro-test-conventions --bin dev_watch --locked -- cargo --package \"$1\""));
    for owner in ["models", "agent", "app", "tui"] {
        let path = root.join(format!("distribution/npm/maestro-{owner}/package.json"));
        let package: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(package["private"], true);
        assert_eq!(
            package["scripts"]["dev"],
            format!("just dev-package maestro-{owner}")
        );
        if owner == "models" {
            assert_eq!(
                package["scripts"]["dev:tsc"],
                "just dev-package maestro-models"
            );
        }
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
    assert_eq!(value["test_tools"], true);
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

#[test]
fn outcome_counts_do_not_depend_on_test_output_or_presentation_mode() {
    let workspace = Workspace::new();
    let executable = compile_tests(
        &workspace,
        "maestro-models",
        "#[test] fn passing() { println!(\"... ignored\"); }\n#[test] #[ignore] fn skipped() {}\n",
    );
    for args in [
        vec!["--nocapture"],
        vec!["--quiet"],
        vec!["--format", "terse"],
    ] {
        let output = Command::new(tooling())
            .arg("--root")
            .arg(&workspace.root)
            .args(["cargo-target", &executable])
            .args(&args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(
            text.ends_with(
                "test result: ok; 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out\n"
            ),
            "{args:?}: {text}"
        );
    }
}

#[test]
fn dev_watch_excludes_cargos_configured_artifact_directory() {
    use std::io::BufRead;
    let workspace = Workspace::new();
    workspace.member("maestro-resources", "maestro-resources", "");
    std::fs::write(
        workspace.root.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = 'maestro-resources'\nversion = '0.1.0'\n",
    )
    .unwrap();
    std::fs::create_dir(workspace.root.join(".cargo")).unwrap();
    std::fs::write(
        workspace.root.join(".cargo/config.toml"),
        "[build]\ntarget-dir = 'alternate artifacts'\n",
    )
    .unwrap();
    std::fs::create_dir(workspace.root.join("alternate artifacts")).unwrap();
    let real = workspace.cargo();
    let cargo = workspace.command("configured-builder", &format!("if test \"$1\" = metadata; then exec {:?} \"$@\"; fi; printf 'build\\n'; printf 'generated' > 'alternate artifacts/generated.rs'; printf 'done\\n'", real));
    let watcher = std::path::Path::new(tooling()).with_file_name("dev_watch");
    let mut child = Command::new(tooling())
        .arg("isolate")
        .arg(watcher)
        .arg(cargo)
        .current_dir(&workspace.root)
        .env_remove("CARGO_TARGET_DIR")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    let stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines() {
            if send.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    assert_eq!(
        receive
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap(),
        "build"
    );
    assert_eq!(
        receive
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap(),
        "done"
    );
    let self_triggered = receive
        .recv_timeout(std::time::Duration::from_millis(200))
        .is_ok();
    Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap();
    assert_eq!(child.wait().unwrap().code(), Some(143));
    reader.join().unwrap();
    assert!(
        !self_triggered,
        "configured Cargo artifacts triggered another build"
    );
}

#[test]
fn browser_smoke_requires_activation_and_matching_precaptured_staged_paths() {
    for activated in [false, true] {
        for (file, matching) in [
            ("notes.txt", false),
            ("crates/maestro-models/src/model.rs", true),
            ("crates/maestro-web/src/view.rs", true),
            ("Cargo.toml", true),
            ("Cargo.lock", true),
        ] {
            let workspace = Workspace::new();
            git(&workspace, &["init", "-q"]);
            std::fs::create_dir(workspace.root.join(".github")).unwrap();
            std::fs::write(
                workspace.root.join(".github/ci.toml"),
                format!("browser_build = {activated}\n"),
            )
            .unwrap();
            let staged = workspace.root.join(file);
            std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
            std::fs::write(&staged, "controlled").unwrap();
            git(&workspace, &["add", "--", file]);
            let bin = workspace.root.join("bin");
            std::fs::create_dir(&bin).unwrap();
            let just = workspace.command("controlled-just", "printf '%s\\n' \"$*\" >> operations");
            std::os::unix::fs::symlink(just, bin.join("just")).unwrap();
            let cargo = workspace.command(
                "controlled-format",
                "printf 'generated' > Cargo.lock; git add -- Cargo.lock",
            );
            let path = std::env::join_paths(
                std::iter::once(bin)
                    .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
            )
            .unwrap();
            let output = Command::new(tooling())
                .arg("pre-commit")
                .current_dir(&workspace.root)
                .env("PATH", path)
                .env("CARGO", cargo)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
            let expected = if activated && matching {
                "check\nbrowser-smoke\n"
            } else {
                "check\n"
            };
            assert_eq!(
                std::fs::read_to_string(workspace.root.join("operations")).unwrap(),
                expected,
                "activated={activated}, file={file}"
            );
        }
    }
}

fn selected_test_recipes_never_replay_unrequested_cases() {
    let workspace = recipe_fixture();
    let path = std::env::join_paths(
        std::iter::once(workspace.root.join("bin"))
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    for args in [
        vec!["--", "--list"],
        vec!["--", "selected_case"],
        vec!["--", "--include-ignored"],
        vec!["--", "--ignored"],
    ] {
        let _ = std::fs::remove_file(workspace.root.join("operations"));
        let output = Command::new("just")
            .arg("test")
            .args(&args)
            .current_dir(&workspace.root)
            .env("PATH", &path)
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        assert_eq!(
            std::fs::read_to_string(workspace.root.join("operations")).unwrap(),
            format!("test --workspace --locked {}|\n", args.join(" ")),
            "{args:?}"
        );
    }
}

#[test]
fn term_ignoring_case_times_out_and_removes_nested_scratch() {
    cancelled_timed_case(false, false);
}

#[test]
fn interrupted_timed_runner_reaps_case_and_exits_143() {
    cancelled_timed_case(true, false);
}

#[test]
fn outer_deadline_reaps_nested_timed_cases_and_scratch() {
    cancelled_timed_case(false, true);
}

#[test]
fn outer_term_reaps_nested_timed_cases_and_scratch() {
    cancelled_timed_case(true, true);
}

#[test]
fn inner_term_reaps_grandchild_holding_stdout_before_outer_deadline() {
    cancelled_case_tree(true, true, true);
}

fn cancelled_timed_case(interrupt: bool, nested_timed: bool) {
    cancelled_case_tree(interrupt, nested_timed, false);
}

fn cancelled_case_tree(interrupt: bool, nested_timed: bool, inner_term: bool) {
    let workspace = Workspace::new();
    workspace.member("maestro-models", "maestro-models", "");
    let record_paths =
        ["case-a", "nested-a", "case-b", "nested-b"].map(|name| workspace.root.join(name));
    let runner_paths = ["runner-a", "runner-b"].map(|name| workspace.root.join(name));
    let grandchild_paths = ["grandchild-a", "grandchild-b"].map(|name| workspace.root.join(name));
    let nested_executable = if nested_timed {
        let source = record_paths.as_chunks::<2>().0.iter().enumerate().map(|(index, paths)| {
            let record = &paths[1];
            let grandchild = if inner_term {
                format!("let mut child = std::process::Command::new(\"/bin/sh\").args([\"-c\", {:?}]).spawn().unwrap(); child.wait().unwrap();", format!("trap '' TERM; printf '%s\\n%s\\n' \"$HOME\" \"$$\" > {:?}; exec sleep 60", grandchild_paths[index]))
            } else { String::new() };
            format!(r#"
#[test] fn nested_{index}() {{
    unsafe extern "C" {{ fn signal(n: i32, handler: usize) -> usize; }}
    unsafe {{ signal(15, 1); }}
    std::fs::write({record:?}, format!("{{}}\n{{}}\n", std::env::var("HOME").unwrap(), std::process::id())).unwrap();
    {grandchild}
    std::thread::sleep(std::time::Duration::from_secs(60));
}}
"#)
        }).collect::<String>();
        std::fs::create_dir_all(workspace.root.join("crates/maestro-models/tests")).unwrap();
        std::fs::write(
            workspace.root.join("crates/maestro-models/tests/nested.rs"),
            source,
        )
        .unwrap();
        let output = fixture_cargo()
            .args([
                "test",
                "--test",
                "nested",
                "--no-run",
                "--message-format=json",
            ])
            .current_dir(&workspace.root)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        Some(
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .find_map(|value| value["executable"].as_str().map(str::to_owned))
                .unwrap(),
        )
    } else {
        None
    };
    let source = record_paths.as_chunks::<2>().0.iter().enumerate().map(|(index, paths)| {
        let record = &paths[0];
        let nested = &paths[1];
        format!(
        r#"
#[test] fn ignores_term_{index}() {{
    unsafe extern "C" {{ fn signal(n: i32, handler: usize) -> usize; }}
    unsafe {{ signal(15, 1); }}
    std::fs::write({record:?}, format!("{{}}\n{{}}\n", std::env::var("HOME").unwrap(), std::process::id())).unwrap();
    let mut child = std::process::Command::new({tool:?}).args(["isolate", "/bin/sh", "-c", {script:?}]).spawn().unwrap();
    child.wait().unwrap();
}}
"#,
        tool = tooling(),
        script = if let Some(executable) = &nested_executable {
            format!("printf '%s\\n%s\\n' \"$HOME\" \"$$\" > {runner:?}; exec {tool:?} --root {root:?} cargo-target {executable:?} --exact nested_{index}", runner = runner_paths[index], tool = tooling(), root = workspace.root)
        } else {
            format!("trap '' TERM; printf '%s\\n%s\\n' \"$HOME\" \"$$\" > {nested:?}; exec sleep 60")
        }
    )}).collect::<String>();
    std::fs::write(
        workspace.root.join("crates/maestro-models/src/lib.rs"),
        source,
    )
    .unwrap();
    let output = fixture_cargo()
        .args(["test", "--lib", "--no-run", "--message-format=json"])
        .current_dir(&workspace.root)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let executable = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find_map(|value| value["executable"].as_str().map(str::to_owned))
        .unwrap();
    let variant = workspace.root.join("timed-tools");
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
    let child = Command::new(&variant)
        .arg("--root")
        .arg(&workspace.root)
        .arg("cargo-target")
        .arg(&executable)
        .args(["--nocapture", "--test-threads=2"])
        .env(
            "MAESTRO_FIXTURE_DEADLINE_MS",
            if interrupt { "10000" } else { "500" },
        )
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let runner_pid = child.id();
    let (send, receive) = std::sync::mpsc::channel();
    let waiter = std::thread::spawn(move || {
        let _ = send.send(child.wait_with_output().unwrap());
    });
    let start = std::time::Instant::now();
    while !record_paths
        .iter()
        .all(|path| std::fs::read_to_string(path).is_ok_and(|text| text.lines().count() == 2))
    {
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if inner_term {
        for path in &grandchild_paths {
            while !std::fs::read_to_string(path).is_ok_and(|text| text.lines().count() == 2) {
                assert!(start.elapsed() < std::time::Duration::from_secs(5));
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        for path in &runner_paths {
            let record = std::fs::read_to_string(path).unwrap();
            assert!(
                Command::new("kill")
                    .args(["-TERM", record.lines().nth(1).unwrap()])
                    .status()
                    .unwrap()
                    .success()
            );
        }
    } else if interrupt {
        assert!(
            Command::new("kill")
                .args(["-TERM", &runner_pid.to_string()])
                .status()
                .unwrap()
                .success()
        );
    }
    let result = receive.recv_timeout(std::time::Duration::from_secs(3));
    let mut records = record_paths
        .map(|path| std::fs::read_to_string(path).unwrap())
        .to_vec();
    if nested_timed {
        records.extend(runner_paths.map(|path| std::fs::read_to_string(path).unwrap()));
    }
    if inner_term {
        records.extend(grandchild_paths.map(|path| std::fs::read_to_string(path).unwrap()));
    }
    let alive = records
        .iter()
        .map(|record| {
            Command::new("kill")
                .args(["-0", record.lines().nth(1).unwrap()])
                .stderr(std::process::Stdio::null())
                .status()
                .unwrap()
                .success()
        })
        .collect::<Vec<_>>();
    let remains = records
        .iter()
        .map(|record| std::path::Path::new(record.lines().next().unwrap()).exists())
        .collect::<Vec<_>>();
    // Bound failed-regression cleanup too, without letting it count as runner cleanup.
    if result.is_err() || alive.iter().any(|alive| *alive) {
        for record in &records {
            let _ = Command::new("kill")
                .args(["-KILL", record.lines().nth(1).unwrap()])
                .status();
        }
        let _ = Command::new("kill")
            .args(["-KILL", &runner_pid.to_string()])
            .status();
    }
    waiter.join().unwrap();
    for record in &records {
        let _ = std::fs::remove_dir_all(
            std::path::Path::new(record.lines().next().unwrap())
                .parent()
                .unwrap(),
        );
    }
    let output =
        result.expect("runner did not reap a TERM-ignoring case within bounded cancellation");
    assert_eq!(
        output.status.code(),
        Some(if inner_term {
            0
        } else if interrupt {
            143
        } else {
            1
        }),
        "{output:?}"
    );
    if !interrupt {
        for index in 0..2 {
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains(&format!("test timed out: ignores_term_{index}")),
                "{output:?}"
            );
        }
    }
    assert!(
        alive.iter().all(|alive| !alive),
        "case descendants remain: {alive:?}"
    );
    assert!(
        remains.iter().all(|remains| !remains),
        "case scratch remains: {remains:?}"
    );
    for records in records[..4].as_chunks::<2>().0 {
        let outer = std::path::Path::new(records[0].lines().next().unwrap())
            .parent()
            .unwrap();
        let inner = std::path::Path::new(records[1].lines().next().unwrap());
        assert!(
            inner.starts_with(outer),
            "nested scratch must belong to the case owner: {inner:?}, {outer:?}"
        );
    }
}

#[test]
fn asset_globs_exclude_dot_files_but_recursive_copies_keep_them() {
    let sources: &[&str] = &[
        "package.json",
        "README.md",
        "CHANGELOG.md",
        "src/modes/interactive/theme/dark.json",
        "src/modes/interactive/theme/.hidden.json",
        "src/modes/interactive/theme/nested/child.json",
        "src/modes/interactive/theme/no.txt",
        "src/modes/interactive/assets/icon.png",
        "src/modes/interactive/assets/.hidden.png",
        "src/modes/interactive/assets/nested/child.png",
        "src/modes/interactive/assets/no.txt",
        "src/core/export-html/template.html",
        "src/core/export-html/template.css",
        "src/core/export-html/template.js",
        "src/core/export-html/vendor/vendor.js",
        "src/core/export-html/vendor/.hidden.js",
        "src/core/export-html/vendor/nested/child.js",
        "src/core/export-html/vendor/no.txt",
        "docs/nested/help.md",
        "docs/.hidden",
        "docs/.nested/file.txt",
        "examples/nested/example.rs",
        "examples/.hidden",
        "examples/.nested/file.txt",
    ];
    for (layout, expected) in [
        (
            "library",
            &[
                "core/export-html/template.css",
                "core/export-html/template.html",
                "core/export-html/template.js",
                "core/export-html/vendor/vendor.js",
                "modes/interactive/assets/icon.png",
                "modes/interactive/theme/dark.json",
            ] as &[&str],
        ),
        (
            "standalone",
            &[
                "CHANGELOG.md",
                "README.md",
                "assets/icon.png",
                "docs/.hidden",
                "docs/.nested/file.txt",
                "docs/nested/help.md",
                "examples/.hidden",
                "examples/.nested/file.txt",
                "examples/nested/example.rs",
                "export-html/template.html",
                "export-html/vendor/vendor.js",
                "package.json",
                "theme/dark.json",
            ] as &[&str],
        ),
    ] {
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
        fn files(root: &std::path::Path, directory: &std::path::Path, result: &mut Vec<String>) {
            for entry in std::fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    files(root, &path, result);
                } else {
                    result.push(
                        path.strip_prefix(root)
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_owned(),
                    );
                }
            }
        }
        let mut actual = Vec::new();
        files(&destination, &destination, &mut actual);
        actual.sort();
        assert_eq!(actual, expected, "{layout}: copied file set");
        for file in actual {
            let input = if layout == "library" {
                format!("src/{file}")
            } else if file.starts_with("theme/") || file.starts_with("assets/") {
                format!("src/modes/interactive/{file}")
            } else if file.starts_with("export-html/") {
                format!("src/core/{file}")
            } else {
                file.clone()
            };
            assert_eq!(
                std::fs::read(destination.join(file)).unwrap(),
                input.as_bytes()
            );
        }
    }
}

#[test]
fn watch_selection_rebuilds_only_selected_packages() {
    watch_selection_inputs(
        &[
            vec!["maestro-models"],
            vec!["maestro-models", "maestro-agent"],
            vec!["maestro-app"],
            vec!["maestro-tui"],
        ],
        false,
    );
}

#[test]
fn watch_selection_ignores_unselected_package_edits() {
    watch_selection_inputs(&[vec!["maestro-models"]], false);
}

#[test]
fn watch_selection_observes_path_dependencies_and_shared_configuration() {
    watch_selection_inputs(&[vec!["maestro-agent"]], true);
}

fn watch_selection_inputs(selections: &[Vec<&str>], dependencies: bool) {
    use std::io::BufRead;
    for selected in selections {
        let workspace = Workspace::new();
        let owners = [
            "maestro-models",
            "maestro-agent",
            "maestro-app",
            "maestro-tui",
            "maestro-settings",
        ];
        for owner in owners {
            workspace.member(
                owner,
                owner,
                if dependencies && owner == "maestro-agent" {
                    "[dependencies]\nmaestro-models = { path = \"../maestro-models\" }"
                } else {
                    ""
                },
            );
            std::fs::write(
                workspace.root.join(format!("crates/{owner}/src/lib.rs")),
                "pub const VALUE: u8 = 1;",
            )
            .unwrap();
        }
        let output = fixture_cargo()
            .arg("generate-lockfile")
            .current_dir(&workspace.root)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let real = workspace.cargo();
        let cargo = workspace.command("observed-builder", &format!("if test \"$1\" = metadata; then exec {real:?} \"$@\"; fi; {real:?} \"$@\" --message-format=json > artifacts || exit $?; printf 'built\\n'"));
        let watcher = std::path::Path::new(tooling()).with_file_name("dev_watch");
        let mut command = Command::new(tooling());
        command
            .arg("isolate")
            .arg(watcher)
            .arg(cargo)
            .current_dir(&workspace.root)
            .stdout(std::process::Stdio::piped());
        for owner in selected {
            command.args(["--package", owner]);
        }
        let mut child = command.spawn().unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        let stdout = child.stdout.take().unwrap();
        let reader = std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                if send.send(line.unwrap()).is_err() {
                    break;
                }
            }
        });
        assert_eq!(
            receive
                .recv_timeout(std::time::Duration::from_secs(20))
                .unwrap(),
            "built"
        );
        std::fs::write(
            workspace.root.join("crates/maestro-settings/src/lib.rs"),
            "pub const VALUE: u8 = 3;",
        )
        .unwrap();
        let unrelated = receive.recv_timeout(std::time::Duration::from_millis(500));
        if unrelated.is_ok() {
            Command::new("kill")
                .args(["-TERM", &child.id().to_string()])
                .status()
                .unwrap();
            child.wait().unwrap();
            reader.join().unwrap();
            panic!("unselected package edit triggered a build: {unrelated:?}");
        }
        assert!(matches!(
            unrelated,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ));
        let artifact = |owner: &str| {
            let messages = std::fs::read_to_string(workspace.root.join("artifacts")).unwrap();
            messages
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .find_map(|message| {
                    if message["reason"] != "compiler-artifact"
                        || message["target"]["name"] != owner.replace('-', "_")
                    {
                        return None;
                    }
                    message["filenames"]
                        .as_array()?
                        .iter()
                        .filter_map(|value| value.as_str())
                        .find(|path| path.ends_with(".rlib"))
                        .map(std::path::PathBuf::from)
                })
        };
        let before = owners.map(|owner| artifact(owner).and_then(|path| std::fs::read(path).ok()));
        if dependencies {
            std::fs::write(
                workspace.root.join("crates/maestro-models/src/lib.rs"),
                "pub const VALUE: u8 = 4;",
            )
            .unwrap();
            assert_eq!(
                receive
                    .recv_timeout(std::time::Duration::from_secs(20))
                    .unwrap(),
                "built"
            );
            assert_ne!(
                before[0],
                artifact("maestro-models").and_then(|path| std::fs::read(path).ok()),
                "path dependency did not rebuild"
            );
            std::fs::write(workspace.root.join("Cargo.toml"), "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n# shared configuration change\n").unwrap();
            assert_eq!(
                receive
                    .recv_timeout(std::time::Duration::from_secs(20))
                    .unwrap(),
                "built"
            );
        }
        for owner in owners {
            std::fs::write(
                workspace.root.join(format!("crates/{owner}/src/lib.rs")),
                "pub const VALUE: u8 = 2;",
            )
            .unwrap();
        }
        assert_eq!(
            receive
                .recv_timeout(std::time::Duration::from_secs(20))
                .unwrap(),
            "built"
        );
        let after = owners.map(|owner| artifact(owner).and_then(|path| std::fs::read(path).ok()));
        Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .status()
            .unwrap();
        assert_eq!(child.wait().unwrap().code(), Some(143));
        reader.join().unwrap();
        for (index, owner) in owners.iter().enumerate() {
            if selected.contains(owner) || (dependencies && *owner == "maestro-models") {
                assert!(before[index].is_some(), "selected artifact absent: {owner}");
                assert_ne!(
                    before[index], after[index],
                    "selected crate did not rebuild: {owner}"
                );
            } else {
                assert!(
                    before[index].is_none() && after[index].is_none(),
                    "unselected crate built: {owner}; selected={selected:?}"
                );
            }
        }
    }
}

fn compiled_timed_fixture(workspace: &Workspace, source: &str) -> String {
    workspace.member("maestro-models", "maestro-models", "");
    std::fs::write(
        workspace.root.join("crates/maestro-models/src/lib.rs"),
        source,
    )
    .unwrap();
    let output = fixture_cargo()
        .args(["test", "--lib", "--no-run", "--message-format=json"])
        .current_dir(&workspace.root)
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

fn short_deadline_tools(workspace: &Workspace) -> std::path::PathBuf {
    let variant = workspace.root.join("timed-tools");
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
    variant
}

#[test]
fn literal_separator_discovery_does_not_execute_selected_tests() {
    separator_discovery(&["--", "selected_case"]);
}

#[test]
fn terse_separator_discovery_does_not_execute_selected_tests() {
    separator_discovery(&["--format", "terse", "--", "selected_case"]);
    separator_discovery(&["--format=terse", "--", "selected_case"]);
}

fn separator_discovery(args: &[&str]) {
    let workspace = Workspace::new();
    let marker = workspace.root.join("discovery-executed");
    let executable = compiled_timed_fixture(
        &workspace,
        &format!(
            r#"
#[test] fn selected_case() {{
    if !std::env::args().any(|arg| arg == "--exact") {{
        std::fs::write({marker:?}, "executed during discovery").unwrap();
    }}
}}
#[test] fn excluded_case() {{ panic!("unselected"); }}
"#
        ),
    );
    let output = Command::new(tooling())
        .arg("--root")
        .arg(&workspace.root)
        .arg("cargo-target")
        .arg(executable)
        .args(args)
        .output()
        .unwrap();
    assert!(!marker.exists(), "discovery executed a test: {output:?}");
    assert!(output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out"),
        "{output:?}"
    );
}

#[test]
fn hanging_discovery_is_cancelled_at_the_case_deadline() {
    let workspace = Workspace::new();
    let executable = compiled_timed_fixture(&workspace, "#[test] fn selected_case() {}\n");
    let record = workspace.root.join("discovery-child");
    std::fs::write(&executable, format!("#!/bin/sh\ntrap '' TERM\nprintf '%s\\n%s\\n' \"$HOME\" \"$$\" > {record:?}\nexec sleep 60\n")).unwrap();
    let variant = short_deadline_tools(&workspace);
    let child = Command::new(variant)
        .arg("--root")
        .arg(&workspace.root)
        .arg("cargo-target")
        .arg(executable)
        .env("MAESTRO_FIXTURE_DEADLINE_MS", "200")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id();
    let (send, receive) = std::sync::mpsc::channel();
    let waiter = std::thread::spawn(move || {
        send.send(child.wait_with_output().unwrap()).unwrap();
    });
    let result = receive.recv_timeout(std::time::Duration::from_secs(2));
    if result.is_err() {
        Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .unwrap();
    }
    waiter.join().unwrap();
    let output = result.expect("discovery bypassed the case deadline");
    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("test discovery timed out"),
        "{output:?}"
    );
    let record = std::fs::read_to_string(record).unwrap();
    assert!(!std::path::Path::new(record.lines().next().unwrap()).exists());
    assert!(
        !Command::new("kill")
            .args(["-0", record.lines().nth(1).unwrap()])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn watch_refreshes_registrations_after_adding_a_path_dependency() {
    dynamic_watch_configuration(false);
}

fn dynamic_watch_configuration(target_change: bool) {
    use std::io::BufRead;
    let workspace = Workspace::new();
    let external = Workspace::new();
    workspace.member("watch-fixture", "watch-fixture", "");
    std::fs::create_dir_all(workspace.root.join(".cargo")).unwrap();
    std::fs::write(external.root.join("dependency.rs"), "before").unwrap();
    std::fs::create_dir(workspace.root.join("crates/watch-fixture/new-target")).unwrap();
    let metadata = serde_json::json!({
        "target_directory": workspace.root.join("target"),
        "packages": [{"id":"watch-fixture","name":"watch-fixture","source":null,"manifest_path":workspace.root.join("crates/watch-fixture/Cargo.toml")},
            {"id":"dependency","name":"dependency","source":null,"manifest_path":external.root.join("Cargo.toml")}],
        "workspace_members":["watch-fixture"],
        "resolve":{"nodes":[{"id":"watch-fixture","dependencies":[]},{"id":"dependency","dependencies":[]}]}
    });
    let mut updated = metadata.clone();
    updated["resolve"]["nodes"][0]["dependencies"] = serde_json::json!(["dependency"]);
    updated["target_directory"] =
        serde_json::json!(workspace.root.join("crates/watch-fixture/new-target"));
    let cargo = workspace.command("dynamic-builder", &format!("if test \"$1\" = metadata; then if test -f .cargo/config.toml || grep -q dependency crates/watch-fixture/Cargo.toml; then printf '%s\\n' '{updated}'; else printf '%s\\n' '{metadata}'; fi; else printf 'built\\n'; fi"));
    let watcher = std::path::Path::new(tooling()).with_file_name("dev_watch");
    let mut child = Command::new(tooling())
        .arg("isolate")
        .arg(watcher)
        .arg(cargo)
        .current_dir(&workspace.root)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines() {
            if send.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    assert_eq!(
        receive
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap(),
        "built"
    );
    if target_change {
        std::fs::write(
            workspace.root.join(".cargo/config.toml"),
            "[build]\ntarget-dir = 'crates/watch-fixture/new-target'\n",
        )
        .unwrap();
    } else {
        let manifest = workspace.root.join("crates/watch-fixture/Cargo.toml");
        let mut text = std::fs::read_to_string(&manifest).unwrap();
        text.push_str(&format!(
            "[dependencies]\ndependency = {{ path = {:?} }}\n",
            external.root
        ));
        std::fs::write(manifest, text).unwrap();
    }
    let refreshed = receive.recv_timeout(std::time::Duration::from_secs(5));
    // Drain duplicate events before observing the later independent edit.
    while receive
        .recv_timeout(std::time::Duration::from_millis(100))
        .is_ok()
    {}
    if target_change {
        std::fs::write(
            workspace
                .root
                .join("crates/watch-fixture/new-target/generated.rs"),
            "artifact",
        )
        .unwrap();
    } else {
        std::fs::write(external.root.join("dependency.rs"), "after").unwrap();
    }
    let later = receive.recv_timeout(std::time::Duration::from_millis(700));
    assert!(
        Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    child.wait().unwrap();
    reader.join().unwrap();
    assert_eq!(refreshed.unwrap(), "built");
    if target_change {
        assert!(
            matches!(later, Err(std::sync::mpsc::RecvTimeoutError::Timeout)),
            "new target artifacts triggered a build: {later:?}"
        );
    } else {
        assert_eq!(
            later.expect("new path dependency was not registered"),
            "built"
        );
    }
}

#[test]
fn watch_refreshes_exclusions_after_changing_the_target_directory() {
    dynamic_watch_configuration(true);
}

#[test]
fn one_case_deadline_leaves_a_concurrent_case_untouched() {
    let workspace = Workspace::new();
    let marker = workspace.root.join("concurrent-completed");
    let executable = compiled_timed_fixture(
        &workspace,
        &format!(
            r#"
#[test] fn a_hangs() {{ std::thread::sleep(std::time::Duration::from_secs(60)); }}
#[test] fn b_warms() {{ std::thread::sleep(std::time::Duration::from_millis(300)); }}
#[test] fn c_completes() {{
    std::thread::sleep(std::time::Duration::from_millis(300));
    std::fs::write({marker:?}, "completed").unwrap();
}}
"#
        ),
    );
    let output = Command::new(short_deadline_tools(&workspace))
        .arg("--root")
        .arg(&workspace.root)
        .arg("cargo-target")
        .arg(executable)
        .arg("--test-threads=2")
        .env("MAESTRO_FIXTURE_DEADLINE_MS", "500")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "completed");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("2 passed; 1 failed; 0 ignored"),
        "{output:?}"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("test timed out: a_hangs"),
        "{output:?}"
    );
}

#[test]
fn watch_recovers_after_a_path_dependency_requires_a_lock_update() {
    use std::io::BufRead;
    let workspace = Workspace::new();
    let external = Workspace::new();
    workspace.member("watch-fixture", "watch-fixture", "");
    external.member("dependency", "dependency", "");
    let lock = fixture_cargo()
        .arg("generate-lockfile")
        .current_dir(&workspace.root)
        .output()
        .unwrap();
    assert!(lock.status.success(), "{lock:?}");
    let real = workspace.cargo();
    let cargo = workspace.command("real-watch-builder", &format!("if test \"$1\" = metadata; then exec {real:?} \"$@\"; fi; {real:?} \"$@\" || exit $?; printf 'built\\n'"));
    let mut child = Command::new(tooling())
        .arg("isolate")
        .arg(std::path::Path::new(tooling()).with_file_name("dev_watch"))
        .arg(cargo)
        .current_dir(&workspace.root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let err_send = send.clone();
    let out_reader = std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines() {
            if send.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    let err_reader = std::thread::spawn(move || {
        for line in std::io::BufReader::new(stderr).lines() {
            if err_send.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    let wait_for = |needle: &str| {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let line = receive
                .recv_timeout(until.saturating_duration_since(std::time::Instant::now()))
                .ok()?;
            if line.contains(needle) {
                return Some(line);
            }
        }
    };
    assert!(wait_for("built").is_some());
    let manifest = workspace.root.join("crates/watch-fixture/Cargo.toml");
    let original = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        &manifest,
        format!(
            "{original}[dependencies]\ndependency = {{ path = {:?} }}\n",
            external.root.join("crates/dependency")
        ),
    )
    .unwrap();
    let reported = wait_for("--locked");
    let lock = fixture_cargo()
        .arg("generate-lockfile")
        .current_dir(&workspace.root)
        .output()
        .unwrap();
    assert!(lock.status.success(), "{lock:?}");
    let recovered = wait_for("built");
    std::fs::write(
        external.root.join("crates/dependency/src/lib.rs"),
        "pub const VALUE: u8 = 2;\n",
    )
    .unwrap();
    let edited = wait_for("built");
    let alive = child.try_wait().unwrap().is_none();
    if alive {
        assert!(
            Command::new("kill")
                .args(["-TERM", &child.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
    }
    child.wait().unwrap();
    out_reader.join().unwrap();
    err_reader.join().unwrap();
    assert!(reported.is_some(), "missing locked metadata error");
    assert!(alive, "watcher exited on a stale lock");
    assert!(
        recovered.is_some(),
        "watcher did not recover after lock update"
    );
    assert!(
        edited.is_some(),
        "new dependency edit did not trigger build"
    );
}
