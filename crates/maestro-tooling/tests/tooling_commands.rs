#![cfg(test)]

use crate::recipe_support::write_input;
use crate::toolchain_support;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
use maestro_test_directory::Scratch;
use maestro_test_process::TestProcess;
use maestro_tooling as tooling;

/// Exercise Rustup selection with an incomplete default and an installed pin.
#[test]
fn child_toolchain_uses_declared_pin() {
    if std::env::var_os("MAESTRO_TOOLCHAIN_CONTROL").is_none() {
        let owner = Fixture::new();
        let home = owner.root.join("rustup");
        let sysroot = prepare_incomplete_toolchain(&home);
        for selector in [None, Some("incomplete")] {
            let mut command = TestProcess::current("child_toolchain_uses_declared_pin")
                .unwrap()
                .nocapture()
                .env("MAESTRO_TOOLCHAIN_CONTROL", "1")
                .env("MAESTRO_EXPECTED_SYSROOT", &sysroot)
                .env("RUSTUP_HOME", &home)
                .env_remove("RUSTUP_TOOLCHAIN")
                .env("RUSTUP_AUTO_INSTALL", "0");
            if let Some(selector) = selector {
                command = command.env("RUSTUP_TOOLCHAIN", selector);
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                String::from_utf8_lossy(&output.stdout).contains("parent environment unchanged")
            );
            std::io::Write::write_all(&mut std::io::stdout(), &output.stdout).unwrap();
        }
        return;
    }
    let before: Vec<_> = std::env::vars_os().collect();
    assert_incomplete_default();
    let fixture = Fixture::new();
    assert_eq!(
        fs::read(fixture.root.join("rust-toolchain.toml")).unwrap(),
        include_bytes!("../../../rust-toolchain.toml")
    );
    check_declared_children(&fixture);
    let declaration = include_str!("../../../rust-toolchain.toml");
    assert!(
        toolchain_support::declare_toolchain(&fixture.root.join("missing"), declaration).is_err()
    );
    assert!(toolchain_support::declare_toolchain(&fixture.child, declaration).is_err());
    assert!(
        std::env::vars_os().collect::<Vec<_>>() == before,
        "parent environment changed"
    );
    println!("parent environment unchanged");
}

/// Observe unsanitized default and override failures before using fixture setup.
fn assert_incomplete_default() {
    let default = Command::new("rustup")
        .current_dir(std::env::temp_dir())
        .env_remove("RUSTUP_TOOLCHAIN")
        .args(["show", "active-toolchain"])
        .output()
        .unwrap();
    assert!(default.status.success());
    assert!(String::from_utf8_lossy(&default.stdout).starts_with("incomplete (default)"));
    println!(
        "default control: {}",
        String::from_utf8_lossy(&default.stdout).trim()
    );
    for selector in [None, Some("incomplete")] {
        let mut compiler = Command::new("rustc");
        compiler.current_dir(std::env::temp_dir()).arg("--version");
        compiler.env_remove("RUSTUP_TOOLCHAIN");
        if let Some(selector) = selector {
            compiler.env("RUSTUP_TOOLCHAIN", selector);
        }
        let output = compiler.output().unwrap();
        assert!(!output.status.success());
        println!(
            "rejecting compiler control: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
}

/// Link the already installed compiler without installing or changing its files.
fn prepare_incomplete_toolchain(home: &Path) -> PathBuf {
    let output = toolchain_support::command("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let sysroot = PathBuf::from(std::str::from_utf8(&output.stdout).unwrap().trim());
    fs::create_dir_all(home.join("toolchains/incomplete")).unwrap();
    let target = home.join("toolchains").join(sysroot.file_name().unwrap());
    #[cfg(unix)]
    std::os::unix::fs::symlink(&sysroot, &target).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&sysroot, &target).unwrap();
    fs::write(
        home.join("settings.toml"),
        "version = \"12\"\ndefault_toolchain = \"incomplete\"\nauto_install_toolchains = false\n",
    )
    .unwrap();
    sysroot
}

/// Require compiler output, Cargo metadata and a real recipe below the declaration.
fn check_declared_children(fixture: &Fixture) {
    let sysroot = toolchain_support::command("rustc")
        .current_dir(&fixture.root)
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(
        sysroot.status.success(),
        "{}",
        String::from_utf8_lossy(&sysroot.stderr)
    );
    let expected = std::env::var_os("MAESTRO_EXPECTED_SYSROOT").unwrap();
    assert_eq!(
        fs::canonicalize(std::str::from_utf8(&sysroot.stdout).unwrap().trim()).unwrap(),
        fs::canonicalize(expected).unwrap()
    );
    let run = Command::new(&fixture.child)
        .env("MAESTRO_RECEIPT", fixture.root.join("receipt"))
        .env("HOME", &fixture.root)
        .status()
        .unwrap();
    assert!(run.success());
    println!("declared compiler and emitted executable: passed");
    check_declared_metadata(fixture);
    fs::write(
        fixture.root.join("justfile"),
        "probe:\n    rustc --print sysroot\n",
    )
    .unwrap();
    let recipe = toolchain_support::command("just")
        .current_dir(&fixture.root)
        .arg("probe")
        .output()
        .unwrap();
    assert!(
        recipe.status.success(),
        "{}",
        String::from_utf8_lossy(&recipe.stderr)
    );
    assert_eq!(recipe.stdout, sysroot.stdout);
    println!("declared recipe compiler sysroot: passed");
}

/// Require the real Cargo proxy to identify the local offline package.
fn check_declared_metadata(fixture: &Fixture) {
    fs::write(
        fixture.root.join("Cargo.toml"),
        "[package]\nname = 'toolchain-probe'\nversion = '0.0.0'\nedition = '2024'\n[workspace]\n",
    )
    .unwrap();
    fs::create_dir(fixture.root.join("src")).unwrap();
    fs::write(fixture.root.join("src/lib.rs"), "").unwrap();
    let metadata = toolchain_support::command("cargo")
        .current_dir(&fixture.root)
        .args([
            "metadata",
            "--offline",
            "--no-deps",
            "--format-version",
            "1",
        ])
        .output()
        .unwrap();
    assert!(
        metadata.status.success(),
        "{}",
        String::from_utf8_lossy(&metadata.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&metadata.stdout).unwrap();
    assert_eq!(metadata["packages"][0]["name"], "toolchain-probe");
    println!("declared Cargo proxy metadata: passed");
}

#[cfg(unix)]
#[test]
fn child_toolchain_preserves_parent_profile_destination() {
    const PROBE: &str = "MAESTRO_TOOLCHAIN_ENV_PROBE";
    if std::env::var_os(PROBE).is_some() {
        let before: Vec<_> = std::env::vars_os().collect();
        let mut child = toolchain_support::command("sh");
        let output = child
            .args(["-c", "test -z \"${LLVM_PROFILE_FILE+x}${RUSTFLAGS+x}${CARGO_ENCODED_RUSTFLAGS+x}${__CARGO_LLVM_COV_RUSTC_WRAPPER+x}\" && test \"$RUSTC_WRAPPER\" = original-wrapper"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            std::env::var("LLVM_PROFILE_FILE").unwrap(),
            "parent-%p.profraw"
        );
        assert!(
            std::env::vars_os().collect::<Vec<_>>() == before,
            "parent environment changed"
        );
        return;
    }
    let output = TestProcess::current("child_toolchain_preserves_parent_profile_destination")
        .unwrap()
        .nocapture()
        .current_dir(std::env::temp_dir())
        .env(PROBE, "1")
        .env("LLVM_PROFILE_FILE", "parent-%p.profraw")
        .env("RUSTFLAGS", "-C instrument-coverage")
        .env("CARGO_ENCODED_RUSTFLAGS", "-Cinstrument-coverage")
        .env("__CARGO_LLVM_COV_RUSTC_WRAPPER", "1")
        .env(
            "__CARGO_LLVM_COV_RUSTC_WRAPPER_PRE_EXISTING",
            "original-wrapper",
        )
        .env("RUSTC_WRAPPER", "coverage-wrapper")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("test result: ok. 1 passed;"));
}

#[test]
fn child_toolchain_keeps_parent_counters_while_building_wasm() {
    let fixture = Fixture::new();
    fs::write(
        fixture.root.join("Cargo.toml"),
        "[package]\nname = 'wasm-child'\nversion = '0.0.0'\nedition = '2024'\n[workspace]\n",
    )
    .unwrap();
    fs::create_dir(fixture.root.join("src")).unwrap();
    fs::write(
        fixture.root.join("src/lib.rs"),
        "pub fn value() -> u32 { 7 }\n",
    )
    .unwrap();
    let binary = compile_toolchain_probe(&fixture);
    let profile = fixture.root.join("parent.profraw");
    let parent = Command::new(&binary)
        .current_dir(&fixture.root)
        .env("MAESTRO_PROBE_CARGO", env!("CARGO"))
        .env("CARGO_TARGET_DIR", fixture.root.join("target"))
        .env("RUSTFLAGS", "-C instrument-coverage")
        .env("CARGO_ENCODED_RUSTFLAGS", "-Cinstrument-coverage")
        .env("LLVM_PROFILE_FILE", &profile)
        .output()
        .unwrap();
    assert!(
        parent.status.success(),
        "{}",
        String::from_utf8_lossy(&parent.stderr)
    );
    assert!(
        fixture
            .root
            .join("target/wasm32-unknown-unknown/debug/libwasm_child.rlib")
            .is_file()
    );
    assert_profile_counters(&profile);
}

/// Compile an instrumented parent independently of the integration runner's instrumentation.
fn compile_toolchain_probe(fixture: &Fixture) -> PathBuf {
    let lock = toolchain_support::command(env!("CARGO"))
        .current_dir(&fixture.root)
        .arg("generate-lockfile")
        .output()
        .unwrap();
    assert!(lock.status.success());
    fs::write(
        fixture.root.join("toolchain_support.rs"),
        include_str!("fixtures/toolchain_support.rs"),
    )
    .unwrap();
    let source = fixture.root.join("parent.rs");
    fs::write(&source, include_str!("fixtures/child_toolchain_probe.rs")).unwrap();
    let binary = fixture.root.join("parent");
    let compile = toolchain_support::command("rustc")
        .current_dir(&fixture.root)
        .args(["--cfg", "test", "-C", "instrument-coverage"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    binary
}

/// Inspect actual parent counters after the instrumented process exits normally.
fn assert_profile_counters(profile: &Path) {
    let directory = toolchain_support::command("rustc")
        .current_dir(profile.parent().unwrap())
        .args(["--print", "target-libdir"])
        .output()
        .unwrap();
    assert!(directory.status.success());
    let tools = Path::new(std::str::from_utf8(&directory.stdout).unwrap().trim())
        .parent()
        .unwrap()
        .join("bin");
    let counts = Command::new(tools.join("llvm-profdata"))
        .args(["show", "--all-functions", "--counts"])
        .arg(profile)
        .output()
        .unwrap();
    assert!(
        counts.status.success(),
        "{}",
        String::from_utf8_lossy(&counts.stderr)
    );
    assert!(String::from_utf8_lossy(&counts.stdout).lines().any(|line| {
        line.trim()
            .strip_prefix("Function count: ")
            .is_some_and(|value| value.parse::<u64>().unwrap() > 0)
    }));
}

struct Fixture {
    root: PathBuf,
    child: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "Maestro's tooling fixture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        toolchain_support::declare_toolchain(&root, include_str!("../../../rust-toolchain.toml"))
            .unwrap();
        fs::create_dir(root.join("receipt")).unwrap();
        fs::create_dir_all(root.join(".maestro/agent")).unwrap();
        let source = root.join("child.rs");
        fs::write(&source, r#"
use std::{env, fs, path::PathBuf};
fn main() {
    let root = PathBuf::from(env::var_os("MAESTRO_RECEIPT").unwrap());
    fs::create_dir_all(root.join("args")).unwrap();
    for (i, arg) in env::args_os().skip(1).enumerate() {
        fs::write(root.join("args").join(i.to_string()), arg.as_encoded_bytes()).unwrap();
    }
    fs::create_dir_all(root.join("env")).unwrap();
    for (key, value) in env::vars_os() {
        fs::write(root.join("env").join(key), value.as_encoded_bytes()).unwrap();
    }
    fs::write(root.join("cwd"), env::current_dir().unwrap().as_os_str().as_encoded_bytes()).unwrap();
    let auth = PathBuf::from(env::var_os("HOME").unwrap()).join(".maestro/agent/auth.json");
    fs::write(root.join("auth-present"), if auth.is_file() { "yes" } else { "no" }).unwrap();
    if let Some(phase) = env::args().nth(1) {
        use std::io::Write;
        let mut log = fs::OpenOptions::new().create(true).append(true).open(root.join("phases")).unwrap();
        writeln!(log, "{phase}").unwrap();
        if env::var("MAESTRO_FAIL_PHASE").ok().as_deref() == Some(&phase) { std::process::exit(17); }
    }
    if let Some(root) = env::var_os("MAESTRO_CHECK_INDEX") {
        for name in ["a space.rs", "line\nbreak.rs", "-dash.rs"] {
            let output = std::process::Command::new("git").current_dir(&root).args(["show", &format!(":{name}")]).output().unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, b"formatted");
        }
    }
    if let Some(format) = env::var_os("MAESTRO_FORMAT_ROOT") {
        for name in ["a space.rs", "line\nbreak.rs", "-dash.rs", "unrelated.rs"] {
            fs::write(PathBuf::from(&format).join(name), "formatted").unwrap();
        }
    }
    if let Some(path) = env::var_os("MAESTRO_ARTIFACT") { fs::write(path, "compiled artifact").unwrap(); }
    if let Some(path) = env::var_os("MAESTRO_BLOCK_RESTORE") { fs::create_dir_all(PathBuf::from(path).join("auth.json.bak")).unwrap(); }
    if let Ok(message) = env::var("MAESTRO_DIAGNOSTIC") { eprint!("{message}"); }
    std::process::exit(env::var("MAESTRO_CHILD_EXIT").unwrap_or_default().parse().unwrap_or(0));
}
"#).unwrap();
        let child = root.join(format!("just{}", std::env::consts::EXE_SUFFIX));
        compile_fixture(&root, &source, &child);
        Self { root, child }
    }

    fn invoke(&self, test: &str, operation: &str, args: &[&str]) -> TestProcess {
        let mut command = TestProcess::named(format!("tooling_commands::{test}"))
            .unwrap()
            .nocapture()
            .env_clear()
            .envs(
                ["SYSTEMROOT", "WINDIR", "LLVM_PROFILE_FILE"]
                    .into_iter()
                    .filter_map(|name| std::env::var_os(name).map(|value| (name, value))),
            )
            .env("MAESTRO_DRIVER", operation)
            .env("MAESTRO_FIXTURE_CARGO", &self.child)
            .env("MAESTRO_FIXTURE_ROOT", &self.root)
            .env("MAESTRO_RECEIPT", self.root.join("receipt"))
            .env("HOME", &self.root)
            .env("PATH", &self.root)
            .current_dir(&self.root);
        if let Some(channel) = toolchain_support::channel() {
            command = command.env("RUSTUP_TOOLCHAIN", channel);
        }
        for (index, arg) in args.iter().enumerate() {
            command = command.env(format!("MAESTRO_ARG_{index}"), arg);
        }
        command
    }

    fn auth(&self) -> PathBuf {
        self.root.join(".maestro/agent/auth.json")
    }
    fn receipt(&self, name: &str) -> Vec<u8> {
        fs::read(self.root.join("receipt").join(name)).unwrap()
    }
    fn arguments(&self) -> Vec<Vec<u8>> {
        let root = self.root.join("receipt/args");
        (0..fs::read_dir(&root).unwrap().count())
            .map(|index| fs::read(root.join(index.to_string())).unwrap())
            .collect()
    }
}

/// Compile the fixture program beneath its declared toolchain root.
fn compile_fixture(root: &Path, source: &Path, child: &Path) {
    assert!(
        toolchain_support::command("rustc")
            .current_dir(root)
            .arg(source)
            .arg("-o")
            .arg(child)
            .status()
            .unwrap()
            .success()
    );
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

/// Reject a stale fixture identity before a zero-test child can report success.
#[test]
fn checked_fixture_rejects_missing_identity() {
    let fixture = Fixture::new();
    assert!(
        fixture
            .invoke("missing_fixture_identity", "test-offline", &[])
            .output()
            .is_err(),
        "stale fixture identity returned successful zero-test output"
    );
}

fn drive() {
    let Some(operation) = std::env::var_os("MAESTRO_DRIVER") else {
        return;
    };
    let mut args = vec![operation];
    args.extend((0..).map_while(|index| std::env::var_os(format!("MAESTRO_ARG_{index}"))));
    let cargo = std::env::var_os("MAESTRO_FIXTURE_CARGO").unwrap();
    let root = std::env::var_os("MAESTRO_FIXTURE_ROOT").unwrap();
    let result = if args[0] == "hook" {
        let mut format = Command::new(&cargo);
        format.arg("format");
        let mut check = Command::new(&cargo);
        check.arg("check");
        let mut conventions = Command::new(&cargo);
        conventions.arg("conventions");
        let mut smoke = Command::new(&cargo);
        smoke.arg("smoke");
        match std::env::var("MAESTRO_MISSING_PHASE").as_deref() {
            Ok("format") => format = Command::new("missing-tool"),
            Ok("check") => check = Command::new("missing-tool"),
            Ok("conventions") => conventions = Command::new("missing-tool"),
            Ok("smoke") => smoke = Command::new("missing-tool"),
            _ => {}
        }
        tooling::pre_commit::run(
            Path::new(&root),
            &mut format,
            &mut check,
            &mut conventions,
            (std::env::var("MAESTRO_ACTIVE_SMOKE").as_deref() == Ok("true")).then_some(&mut smoke),
        )
    } else {
        tooling::run(&args, Path::new(&cargo), Path::new(&root))
    };
    match result {
        Ok(code) => std::process::exit(if code == std::process::ExitCode::SUCCESS {
            0
        } else if code == std::process::ExitCode::from(17) {
            17
        } else {
            1
        }),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

const OFFLINE_KEYS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_OAUTH_TOKEN",
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "GROQ_API_KEY",
    "CEREBRAS_API_KEY",
    "XAI_API_KEY",
    "OPENROUTER_API_KEY",
    "ZAI_API_KEY",
    "MISTRAL_API_KEY",
    "MINIMAX_API_KEY",
    "MINIMAX_CN_API_KEY",
    "KIMI_API_KEY",
    "HF_TOKEN",
    "AI_GATEWAY_API_KEY",
    "OPENCODE_API_KEY",
    "COPILOT_GITHUB_TOKEN",
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "GOOGLE_CLOUD_PROJECT",
    "GCLOUD_PROJECT",
    "GOOGLE_CLOUD_LOCATION",
    "AWS_PROFILE",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "AWS_REGION",
    "AWS_DEFAULT_REGION",
    "AWS_BEARER_TOKEN_BEDROCK",
    "AWS_CONTAINER_CREDENTIALS_RELATIVE_URI",
    "AWS_CONTAINER_CREDENTIALS_FULL_URI",
    "AWS_WEB_IDENTITY_TOKEN_FILE",
    "BEDROCK_EXTENSIVE_MODEL_TEST",
    "FIREWORKS_API_KEY",
];

#[test]
fn maestro_offline_removes_only_listed_credentials() {
    drive();
    for value in ["", "fixture"] {
        let fixture = Fixture::new();
        let mut command = fixture.invoke(
            "maestro_offline_removes_only_listed_credentials",
            "test-offline",
            &[],
        );
        for key in OFFLINE_KEYS {
            command = command.env(key, value);
        }
        for key in [
            "AZURE_OPENAI_API_KEY",
            "MAESTRO_CANARY",
            "TMPDIR",
            "XDG_CONFIG_HOME",
            "MAESTRO_AGENT_DIR",
        ] {
            command = command.env(key, "kept");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        for key in OFFLINE_KEYS {
            assert!(
                !fixture.root.join("receipt/env").join(key).exists(),
                "{key}"
            );
        }
        for key in [
            "AZURE_OPENAI_API_KEY",
            "MAESTRO_CANARY",
            "TMPDIR",
            "XDG_CONFIG_HOME",
            "MAESTRO_AGENT_DIR",
        ] {
            assert_eq!(fixture.receipt(&format!("env/{key}")), b"kept");
        }
        assert_eq!(fixture.receipt("env/MAESTRO_NO_LOCAL_LLM"), b"1");
        assert_eq!(
            fixture.receipt("env/HOME"),
            fixture.root.as_os_str().as_encoded_bytes()
        );
        assert!(stdout(&output).contains("Running tests without API keys...\n"));
    }
}

#[test]
fn maestro_source_reports_missing_tool_and_child_failure() {
    drive();
    let fixture = Fixture::new();
    let missing = fixture.root.join("missing-cargo");
    let output = fixture
        .invoke(
            "maestro_source_reports_missing_tool_and_child_failure",
            "run-source",
            &[],
        )
        .env("MAESTRO_FIXTURE_CARGO", &missing)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!(
            "cargo not found at {}. Run just setup from the repo root first.\n",
            missing.display()
        )
    );
    let output = fixture
        .invoke(
            "maestro_source_reports_missing_tool_and_child_failure",
            "run-source",
            &[],
        )
        .env("MAESTRO_CHILD_EXIT", "17")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(17));
}

#[test]
fn maestro_offline_restores_auth_after_child_results() {
    drive();
    for exit in ["0", "17"] {
        let fixture = Fixture::new();
        fs::write(fixture.auth(), b"original auth").unwrap();
        let output = fixture
            .invoke(
                "maestro_offline_restores_auth_after_child_results",
                "test-offline",
                &[],
            )
            .env("MAESTRO_CHILD_EXIT", exit)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(exit.parse().unwrap()));
        assert_eq!(fixture.receipt("auth-present"), b"no");
        assert_eq!(fs::read(fixture.auth()).unwrap(), b"original auth");
        assert!(!fixture.auth().with_extension("json.bak").exists());
        let text = stdout(&output);
        assert!(
            text.find("Moved auth.json to backup\n").unwrap()
                < text.find("Running tests without API keys...\n").unwrap()
        );
        assert!(
            text.find("Running tests without API keys...\n").unwrap()
                < text.find("Restored auth.json\n").unwrap()
        );
    }
    let fixture = Fixture::new();
    fs::write(fixture.auth(), b"spawn failure auth").unwrap();
    fs::remove_file(&fixture.child).unwrap();
    let output = fixture
        .invoke(
            "maestro_offline_restores_auth_after_child_results",
            "test-offline",
            &[],
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(fs::read(fixture.auth()).unwrap(), b"spawn failure auth");
    assert!(stdout(&output).contains("Restored auth.json\n"));
}

#[cfg(unix)]
#[test]
fn maestro_offline_restores_relative_auth_link_from_backup_directory() {
    drive();
    let fixture = Fixture::new();
    fs::write(fixture.auth().with_file_name("credentials"), "auth").unwrap();
    std::os::unix::fs::symlink("credentials", fixture.auth()).unwrap();
    fs::create_dir(fixture.auth().with_extension("json.bak")).unwrap();
    let output = fixture
        .invoke(
            "maestro_offline_restores_relative_auth_link_from_backup_directory",
            "test-offline",
            &[],
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_link(fixture.auth()).unwrap(),
        Path::new("credentials")
    );
    assert_eq!(fs::read(fixture.auth()).unwrap(), b"auth");
    assert!(stdout(&output).contains("Restored auth.json\n"));
}

fn assert_auth_move_errors() {
    let fixture = Fixture::new();
    fs::write(fixture.auth(), "auth").unwrap();
    fs::create_dir_all(fixture.auth().with_extension("json.bak").join("auth.json")).unwrap();
    let output = fixture
        .invoke(
            "maestro_offline_keeps_fixed_backup_move_behavior",
            "test-offline",
            &[],
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(fs::read(fixture.auth()).unwrap(), b"auth");
    assert!(!fixture.root.join("receipt/auth-present").exists());
    let fixture = Fixture::new();
    fs::write(fixture.auth(), "restore bytes").unwrap();
    let output = fixture
        .invoke(
            "maestro_offline_keeps_fixed_backup_move_behavior",
            "test-offline",
            &[],
        )
        .env("MAESTRO_BLOCK_RESTORE", fixture.auth())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        fs::read(fixture.auth().with_extension("json.bak")).unwrap(),
        b"restore bytes"
    );
    assert!(!stdout(&output).contains("Restored auth.json"));
    assert!(!output.stderr.is_empty());
}

#[cfg(unix)]
fn assert_auth_link_restored() {
    let fixture = Fixture::new();
    let target = fixture.root.join("auth-target");
    fs::write(&target, "linked auth").unwrap();
    std::os::unix::fs::symlink(&target, fixture.auth()).unwrap();
    let output = fixture
        .invoke(
            "maestro_offline_keeps_fixed_backup_move_behavior",
            "test-offline",
            &[],
        )
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(fs::read_link(fixture.auth()).unwrap(), target);
    assert_eq!(fs::read(fixture.auth()).unwrap(), b"linked auth");
    assert_eq!(fixture.receipt("auth-present"), b"no");
}

fn assert_auth_preexisting_backup() {
    for original in [None, Some("new auth")] {
        let fixture = Fixture::new();
        fs::write(fixture.auth().with_extension("json.bak"), "old backup").unwrap();
        if let Some(bytes) = original {
            fs::write(fixture.auth(), bytes).unwrap();
        }
        let output = fixture
            .invoke(
                "maestro_offline_keeps_fixed_backup_move_behavior",
                "test-offline",
                &[],
            )
            .output()
            .unwrap();
        assert!(output.status.success());
        if let Some(original) = original {
            assert_eq!(fs::read(fixture.auth()).unwrap(), original.as_bytes());
            assert!(stdout(&output).contains("Restored auth.json\n"));
        } else {
            assert!(!fixture.auth().exists());
            assert_eq!(
                fs::read(fixture.auth().with_extension("json.bak")).unwrap(),
                b"old backup"
            );
            assert!(!stdout(&output).contains("Restored auth.json\n"));
        }
    }
}

#[test]
fn maestro_offline_keeps_fixed_backup_move_behavior() {
    drive();
    assert_auth_preexisting_backup();
    let fixture = Fixture::new();
    let output = fixture
        .invoke(
            "maestro_offline_keeps_fixed_backup_move_behavior",
            "test-offline",
            &[],
        )
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!fixture.auth().exists());
    assert!(!stdout(&output).contains("Restored auth.json"));
    fs::write(fixture.auth(), "auth").unwrap();
    fs::create_dir(fixture.auth().with_extension("json.bak")).unwrap();
    let output = fixture
        .invoke(
            "maestro_offline_keeps_fixed_backup_move_behavior",
            "test-offline",
            &[],
        )
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(fs::read(fixture.auth()).unwrap(), b"auth");
    assert!(
        !fixture
            .auth()
            .with_extension("json.bak")
            .join("auth.json")
            .exists()
    );
    assert!(stdout(&output).contains("Restored auth.json"));
    assert_auth_move_errors();
    #[cfg(unix)]
    assert_auth_link_restored();
}

#[test]
fn maestro_offline_keeps_cwd_and_ignores_arguments() {
    drive();
    let fixture = Fixture::new();
    let output = fixture
        .invoke(
            "maestro_offline_keeps_cwd_and_ignores_arguments",
            "test-offline",
            &["ignored", "--", ""],
        )
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(fixture.arguments(), vec![b"test".to_vec()]);
    assert_eq!(
        fixture.receipt("cwd"),
        fixture.root.as_os_str().as_encoded_bytes()
    );
}

#[test]
fn maestro_source_launchers_clear_the_same_credentials() {
    drive();
    for operation in ["run-source", "run-source-windows"] {
        for value in ["", "fixture"] {
            let fixture = Fixture::new();
            let mut command = fixture.invoke(
                "maestro_source_launchers_clear_the_same_credentials",
                operation,
                &["--no-env"],
            );
            for key in OFFLINE_KEYS {
                command = command.env(key, value);
            }
            for key in [
                "AZURE_OPENAI_API_KEY",
                "AZURE_OPENAI_BASE_URL",
                "AZURE_OPENAI_RESOURCE_NAME",
            ] {
                command = command.env(key, value);
            }
            command = command
                .env("MAESTRO_NO_LOCAL_LLM", "kept")
                .env("MAESTRO_CANARY", "kept");
            let output = command.output().unwrap();
            assert!(output.status.success());
            for key in OFFLINE_KEYS
                .iter()
                .filter(|key| **key != "BEDROCK_EXTENSIVE_MODEL_TEST")
            {
                assert!(
                    !fixture.root.join("receipt/env").join(key).exists(),
                    "{key}"
                );
            }
            for key in [
                "AZURE_OPENAI_API_KEY",
                "AZURE_OPENAI_BASE_URL",
                "AZURE_OPENAI_RESOURCE_NAME",
            ] {
                assert!(!fixture.root.join("receipt/env").join(key).exists());
            }
            assert_eq!(
                fixture.receipt("env/BEDROCK_EXTENSIVE_MODEL_TEST"),
                value.as_bytes()
            );
            assert_eq!(fixture.receipt("env/MAESTRO_NO_LOCAL_LLM"), b"kept");
            assert_eq!(fixture.receipt("env/MAESTRO_CANARY"), b"kept");
            assert_eq!(
                stdout(&output)
                    .matches("Running without API keys...\n")
                    .count(),
                1
            );
        }
    }
}

#[test]
fn maestro_source_keeps_switch_rules_and_literal_arguments() {
    drive();
    for operation in ["run-source", "run-source-windows"] {
        let fixture = Fixture::new();
        let args = [
            "",
            "a b",
            "'quoted'",
            "--no-env",
            "--",
            "--NO-ENV",
            "--no-env",
            "é\u{feff}\u{85}\u{2003}",
        ];
        let output = fixture
            .invoke(
                "maestro_source_keeps_switch_rules_and_literal_arguments",
                operation,
                &args,
            )
            .output()
            .unwrap();
        assert!(output.status.success());
        let mut expected: Vec<_> = ["run", "--quiet", "--manifest-path"]
            .map(|arg| arg.as_bytes().to_vec())
            .into();
        expected.push(
            fixture
                .root
                .join("Cargo.toml")
                .as_os_str()
                .as_encoded_bytes()
                .to_vec(),
        );
        expected.extend(["-p", "maestro", "--"].map(|arg| arg.as_bytes().to_vec()));
        let remaining: &[&str] = if operation == "run-source-windows" {
            &["", "a b", "'quoted'", "--", "é\u{feff}\u{85}\u{2003}"]
        } else {
            &[
                "",
                "a b",
                "'quoted'",
                "--",
                "--NO-ENV",
                "é\u{feff}\u{85}\u{2003}",
            ]
        };
        expected.extend(remaining.iter().map(|arg| arg.as_bytes().to_vec()));
        assert_eq!(fixture.arguments(), expected);
        assert_eq!(
            fixture.receipt("cwd"),
            fixture.root.as_os_str().as_encoded_bytes()
        );
        assert_eq!(
            stdout(&output)
                .matches("Running without API keys...\n")
                .count(),
            1
        );
    }
    assert_unix_switch_case_sensitive();
}

fn assert_unix_switch_case_sensitive() {
    let fixture = Fixture::new();
    let output = fixture
        .invoke(
            "maestro_source_keeps_switch_rules_and_literal_arguments",
            "run-source",
            &["--NO-ENV"],
        )
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!stdout(&output).contains("Running without API keys"));
}

#[test]
fn maestro_source_without_switch_preserves_environment() {
    drive();
    let fixture = Fixture::new();
    fs::write(fixture.auth(), "auth").unwrap();
    let mut command = fixture.invoke(
        "maestro_source_without_switch_preserves_environment",
        "run-source",
        &["literal"],
    );
    for key in OFFLINE_KEYS {
        command = command.env(key, "kept");
    }
    command = command.env("MAESTRO_NO_LOCAL_LLM", "kept");
    let output = command.output().unwrap();
    assert!(output.status.success());
    for key in OFFLINE_KEYS {
        assert_eq!(fixture.receipt(&format!("env/{key}")), b"kept");
    }
    assert_eq!(fixture.receipt("env/MAESTRO_NO_LOCAL_LLM"), b"kept");
    assert_eq!(fixture.receipt("auth-present"), b"yes");
    assert_eq!(fs::read(fixture.auth()).unwrap(), b"auth");
    assert!(!stdout(&output).contains("Running without API keys"));
}

fn asset_inputs(fixture: &Fixture) -> PathBuf {
    let source = fixture.root.join("assets-source");
    for (path, bytes) in [
        ("src/modes/interactive/theme/schema.json", "schema"),
        ("src/modes/interactive/theme/dark.json", "dark"),
        ("src/modes/interactive/assets/mark.png", "image"),
        ("src/core/export-html/template.html", "html"),
        ("src/core/export-html/template.css", "css"),
        ("src/core/export-html/template.js", "runtime"),
        ("src/core/export-html/vendor/highlight.js", "vendor"),
    ] {
        write_input(&source, path, bytes);
    }
    source
}

#[test]
fn maestro_library_assets_keep_selection_and_copy_order() {
    let fixture = Fixture::new();
    let source = asset_inputs(&fixture);
    let themes = source.join("src/modes/interactive/theme");
    fs::write(themes.join(".hidden.json"), "hidden").unwrap();
    fs::write(themes.join("UPPER.JSON"), "uppercase").unwrap();
    fs::write(themes.join("space é.json"), "spaced").unwrap();
    fs::create_dir(themes.join("nested")).unwrap();
    fs::write(themes.join("nested/inside.json"), "excluded").unwrap();
    let destination = fixture.root.join("dist");
    fs::create_dir_all(destination.join("modes/interactive/theme")).unwrap();
    fs::write(destination.join("modes/interactive/theme/dark.json"), "old").unwrap();
    fs::write(destination.join("unrelated"), "kept").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_development"))
        .arg("copy-assets")
        .arg(&source)
        .arg(&destination)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    for (path, bytes) in [
        ("modes/interactive/theme/schema.json", "schema"),
        ("modes/interactive/theme/dark.json", "dark"),
        ("modes/interactive/theme/space é.json", "spaced"),
        ("modes/interactive/assets/mark.png", "image"),
        ("core/export-html/template.html", "html"),
        ("core/export-html/template.css", "css"),
        ("core/export-html/template.js", "runtime"),
        ("core/export-html/vendor/highlight.js", "vendor"),
        ("unrelated", "kept"),
    ] {
        assert_eq!(fs::read(destination.join(path)).unwrap(), bytes.as_bytes());
    }
    assert!(
        !destination
            .join("modes/interactive/theme/.hidden.json")
            .exists()
    );
    assert!(
        !destination
            .join("modes/interactive/theme/UPPER.JSON")
            .exists()
    );
    assert!(!destination.join("modes/interactive/theme/nested").exists());
}

fn standalone_inputs(source: &Path, package: &Path) {
    for (name, bytes) in [
        (
            "Cargo.toml",
            "[package]\nname = 'maestro'\nversion = '0.1.0'\nedition = '2024'\n",
        ),
        ("viewer/index.html", "viewer"),
        ("viewer/style.css", "style"),
        ("viewer/vendor/engine.js", "engine"),
    ] {
        write_input(source, name, bytes);
    }
    package_inputs(package);
}

/// Write the package-level inputs the standalone layout copies.
fn package_inputs(package: &Path) {
    for (name, bytes) in [
        ("README.md", "readme"),
        ("CHANGELOG.md", "history"),
        ("docs/.hidden", "hidden"),
        ("docs/guide.md", "guide"),
        ("examples/nested/demo.rs", "example"),
    ] {
        write_input(package, name, bytes);
    }
}

/// Assert the copy rejects an absent or empty viewer with its own message.
fn assert_viewer_required(source: &Path, destination: &Path, package: &Path) {
    for viewer in [source.join("absent-viewer"), source.join("empty-viewer")] {
        if viewer.ends_with("empty-viewer") {
            fs::create_dir(&viewer).unwrap();
        }
        let error = tooling::copy_assets::copy(
            source,
            destination,
            tooling::copy_assets::Layout::Standalone {
                metadata: &source.join("Cargo.toml"),
                viewer: &viewer,
                package,
            },
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "viewer parameter must name a nonempty directory: {}",
                viewer.display()
            )
        );
    }
}

#[test]
fn maestro_standalone_assets_preserve_recursive_layout() {
    let fixture = Fixture::new();
    let source = asset_inputs(&fixture);
    let package = fixture.root.join("package");
    standalone_inputs(&source, &package);
    let destination = fixture.root.join("standalone");
    fs::create_dir_all(source.join("src")).unwrap();
    fs::write(source.join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(
        fixture.root.join("Cargo.toml"),
        "[workspace]\nmembers = ['assets-source']\nresolver = '3'\n",
    )
    .unwrap();
    assert!(
        toolchain_support::command(env!("CARGO"))
            .arg("generate-lockfile")
            .current_dir(&fixture.root)
            .status()
            .unwrap()
            .success()
    );
    let args = [
        "copy-binary-assets".into(),
        source.clone().into_os_string(),
        destination.clone().into_os_string(),
        source.join("viewer").into_os_string(),
        package.clone().into_os_string(),
    ];
    assert_eq!(
        tooling::run(&args, Path::new(env!("CARGO")), &fixture.root).unwrap(),
        std::process::ExitCode::SUCCESS
    );
    for (name, bytes) in [
        (
            "Cargo.toml",
            "[package]\nname = 'maestro'\nversion = '0.1.0'\nedition = '2024'\n",
        ),
        ("README.md", "readme"),
        ("CHANGELOG.md", "history"),
        ("theme/schema.json", "schema"),
        ("assets/mark.png", "image"),
        ("docs/.hidden", "hidden"),
        ("examples/nested/demo.rs", "example"),
        ("export-html/index.html", "viewer"),
        ("export-html/style.css", "style"),
        ("export-html/vendor/engine.js", "engine"),
    ] {
        assert_eq!(fs::read(destination.join(name)).unwrap(), bytes.as_bytes());
    }
    assert_viewer_required(&source, &destination, &package);
    fs::write(
        source.join("Cargo.toml"),
        "[package]\nname = 'not-the-binary'\nversion = '0.1.0'\n",
    )
    .unwrap();
    assert!(tooling::run(&args, Path::new(env!("CARGO")), &fixture.root).is_err());
}

/// Build a crate-shaped source, a separate package directory and the metadata.
fn split_inputs(fixture: &Fixture) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let source = asset_inputs(fixture);
    for (name, bytes) in [
        ("README.md", "crate-readme"),
        ("CHANGELOG.md", "crate-history"),
        ("docs/crate.md", "crate-docs"),
        ("examples/crate.txt", "crate-example"),
    ] {
        write_input(&source, name, bytes);
    }
    let package = fixture.root.join("package");
    package_inputs(&package);
    let metadata = fixture.root.join("binary/Cargo.toml");
    write_input(&fixture.root.join("binary"), "Cargo.toml", "manifest");
    let viewer = fixture.root.join("viewer");
    write_input(&viewer, "index.html", "viewer");
    (source, package, metadata, viewer)
}

#[test]
fn maestro_package_assets_come_from_the_package_directory() {
    let fixture = Fixture::new();
    let (source, package, metadata, viewer) = split_inputs(&fixture);
    let destination = fixture.root.join("standalone");
    tooling::copy_assets::copy(
        &source,
        &destination,
        tooling::copy_assets::Layout::Standalone {
            metadata: &metadata,
            viewer: &viewer,
            package: &package,
        },
    )
    .unwrap();
    for (name, bytes) in [
        ("Cargo.toml", "manifest"),
        ("README.md", "readme"),
        ("CHANGELOG.md", "history"),
        ("theme/schema.json", "schema"),
        ("assets/mark.png", "image"),
        ("docs/.hidden", "hidden"),
        ("docs/guide.md", "guide"),
        ("examples/nested/demo.rs", "example"),
        ("export-html/index.html", "viewer"),
    ] {
        assert_eq!(fs::read(destination.join(name)).unwrap(), bytes.as_bytes());
    }
    assert!(!destination.join("docs/crate.md").exists());
    assert!(!destination.join("examples/crate.txt").exists());
}

#[test]
fn maestro_missing_package_input_keeps_completed_groups() {
    let fixture = Fixture::new();
    let (source, package, metadata, viewer) = split_inputs(&fixture);
    fs::remove_dir_all(package.join("examples")).unwrap();
    let destination = fixture.root.join("incomplete");
    assert!(
        tooling::copy_assets::copy(
            &source,
            &destination,
            tooling::copy_assets::Layout::Standalone {
                metadata: &metadata,
                viewer: &viewer,
                package: &package,
            },
        )
        .is_err()
    );
    assert_eq!(
        fs::read(destination.join("theme/schema.json")).unwrap(),
        b"schema"
    );
    assert_eq!(fs::read(destination.join("README.md")).unwrap(), b"readme");
    assert!(!destination.join("examples").exists());
}

/// Remove one input file or directory, whichever the path names.
fn remove_input(path: &Path) {
    if path.is_dir() {
        fs::remove_dir_all(path).unwrap();
    } else {
        fs::remove_file(path).unwrap();
    }
}

/// Assert that a failed copy names the input it could not read.
fn assert_names_input(error: &io::Error, input: &Path) {
    assert!(
        error.to_string().contains(&input.display().to_string()),
        "{error} does not name {}",
        input.display()
    );
    assert_eq!(error.kind(), io::ErrorKind::NotFound, "{error}");
}

#[test]
fn maestro_missing_package_input_names_the_absent_path() {
    for name in ["README.md", "CHANGELOG.md", "docs", "examples"] {
        let fixture = Fixture::new();
        let (source, package, metadata, viewer) = split_inputs(&fixture);
        let input = package.join(name);
        remove_input(&input);
        let destination = fixture.root.join("incomplete");
        let error = tooling::copy_assets::copy(
            &source,
            &destination,
            tooling::copy_assets::Layout::Standalone {
                metadata: &metadata,
                viewer: &viewer,
                package: &package,
            },
        )
        .unwrap_err();
        assert_names_input(&error, &input);
        // The whole message is one input clause; a nested wrap would repeat it.
        assert_eq!(
            error.to_string(),
            format!(
                "cannot copy {}: {}",
                input.display(),
                fs::symlink_metadata(&input).unwrap_err()
            )
        );
        assert!(destination.join("Cargo.toml").is_file());
    }
}

#[test]
fn maestro_missing_crate_input_names_the_absent_path() {
    for name in [
        "src/modes/interactive/theme",
        "src/modes/interactive/assets",
        "src/core/export-html/template.css",
    ] {
        let fixture = Fixture::new();
        let source = asset_inputs(&fixture);
        let input = source.join(name);
        remove_input(&input);
        let error = tooling::copy_assets::copy(
            &source,
            &fixture.root.join("dist"),
            tooling::copy_assets::Layout::Library,
        )
        .unwrap_err();
        assert_names_input(&error, &input);
    }
    let fixture = Fixture::new();
    let (source, package, metadata, viewer) = split_inputs(&fixture);
    let input = source.join("src/modes/interactive/theme");
    remove_input(&input);
    let error = tooling::copy_assets::copy(
        &source,
        &fixture.root.join("standalone"),
        tooling::copy_assets::Layout::Standalone {
            metadata: &metadata,
            viewer: &viewer,
            package: &package,
        },
    )
    .unwrap_err();
    assert_names_input(&error, &input);
}

#[test]
fn maestro_missing_metadata_names_the_absent_path() {
    let fixture = Fixture::new();
    let (source, package, _, viewer) = split_inputs(&fixture);
    let metadata = fixture.root.join("absent/Cargo.toml");
    let error = tooling::copy_assets::copy(
        &source,
        &fixture.root.join("standalone"),
        tooling::copy_assets::Layout::Standalone {
            metadata: &metadata,
            viewer: &viewer,
            package: &package,
        },
    )
    .unwrap_err();
    assert_names_input(&error, &metadata);
}

/// Assert that a viewer directory this process cannot list is named.
#[cfg(unix)]
#[test]
fn maestro_unreadable_viewer_names_its_path() {
    let fixture = Fixture::new();
    let (source, package, metadata, viewer) = split_inputs(&fixture);
    let Some(_denied) = maestro_test_directory::deny(&viewer, 0o000).unwrap() else {
        return;
    };
    let error = tooling::copy_assets::copy(
        &source,
        &fixture.root.join("standalone"),
        tooling::copy_assets::Layout::Standalone {
            metadata: &metadata,
            viewer: &viewer,
            package: &package,
        },
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "cannot copy {}: {}",
            viewer.display(),
            fs::read_dir(&viewer).unwrap_err()
        )
    );
}

/// Assert that a requested source file this process cannot read names its own path.
#[cfg(unix)]
#[test]
fn maestro_unreadable_source_file_names_its_path() {
    let fixture = Fixture::new();
    let (source, package, metadata, viewer) = split_inputs(&fixture);
    let input = package.join("README.md");
    let Some(_denied) = maestro_test_directory::deny(&input, 0o000).unwrap() else {
        return;
    };
    let error = tooling::copy_assets::copy(
        &source,
        &fixture.root.join("standalone"),
        tooling::copy_assets::Layout::Standalone {
            metadata: &metadata,
            viewer: &viewer,
            package: &package,
        },
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "cannot copy {}: {}",
            input.display(),
            fs::File::open(&input).unwrap_err()
        )
    );
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}

/// Assert that an entry this process cannot read inside a copied tree is named.
#[cfg(unix)]
#[test]
fn maestro_unreadable_tree_entry_names_that_entry() {
    let fixture = Fixture::new();
    let (source, package, metadata, viewer) = split_inputs(&fixture);
    let entry = package.join("docs/guide.md");
    let Some(_denied) = maestro_test_directory::deny(&entry, 0o000).unwrap() else {
        return;
    };
    let error = tooling::copy_assets::copy(
        &source,
        &fixture.root.join("standalone"),
        tooling::copy_assets::Layout::Standalone {
            metadata: &metadata,
            viewer: &viewer,
            package: &package,
        },
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "cannot copy {}: {}",
            entry.display(),
            fs::File::open(&entry).unwrap_err()
        )
    );
}

/// Assert that a destination-side failure keeps its native error without an input name.
#[test]
fn maestro_destination_failure_keeps_its_native_error() {
    let fixture = Fixture::new();
    let (source, package, metadata, viewer) = split_inputs(&fixture);
    let destination = fixture.root.join("standalone");
    fs::create_dir_all(destination.join("README.md")).unwrap();
    let error = tooling::copy_assets::copy(
        &source,
        &destination,
        tooling::copy_assets::Layout::Standalone {
            metadata: &metadata,
            viewer: &viewer,
            package: &package,
        },
    )
    .unwrap_err();
    let message = error.to_string();
    assert!(
        !message.contains("cannot copy") && !message.contains(&package.display().to_string()),
        "{message}"
    );
}

fn assert_asset_match_order() {
    let fixture = Fixture::new();
    let source = asset_inputs(&fixture);
    let themes = source.join("src/modes/interactive/theme");
    fs::write(themes.join("a.json"), "first").unwrap();
    fs::create_dir(themes.join("b.json")).unwrap();
    fs::write(themes.join("c.json"), "later").unwrap();
    let destination = fixture.root.join("dist");
    let error =
        tooling::copy_assets::copy(&source, &destination, tooling::copy_assets::Layout::Library)
            .unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "cannot copy {}: not a regular file",
            themes.join("b.json").display()
        )
    );
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(
        fs::read(destination.join("modes/interactive/theme/a.json")).unwrap(),
        b"first"
    );
    assert!(!destination.join("modes/interactive/theme/c.json").exists());
    assert!(
        !destination
            .join("modes/interactive/assets/mark.png")
            .exists()
    );
}

#[test]
fn maestro_asset_errors_keep_prior_copies() {
    let fixture = Fixture::new();
    let source = asset_inputs(&fixture);
    let destination = fixture.root.join("dist");
    fs::remove_file(source.join("src/core/export-html/template.css")).unwrap();
    assert!(
        tooling::copy_assets::copy(&source, &destination, tooling::copy_assets::Layout::Library)
            .is_err()
    );
    assert_eq!(
        fs::read(destination.join("modes/interactive/theme/schema.json")).unwrap(),
        b"schema"
    );
    assert_eq!(
        fs::read(destination.join("core/export-html/template.html")).unwrap(),
        b"html"
    );
    assert!(!destination.join("core/export-html/template.js").exists());
    fs::write(source.join("src/core/export-html/template.css"), "css").unwrap();
    let image = source.join("src/modes/interactive/assets/mark.png");
    fs::remove_file(&image).unwrap();
    assert!(
        tooling::copy_assets::copy(&source, &destination, tooling::copy_assets::Layout::Library)
            .is_err()
    );
    fs::create_dir(&image).unwrap();
    let error =
        tooling::copy_assets::copy(&source, &destination, tooling::copy_assets::Layout::Library)
            .unwrap_err();
    assert_eq!(
        error.to_string(),
        format!("cannot copy {}: not a regular file", image.display())
    );
    for args in [vec![], vec!["unknown".into()], vec!["copy-assets".into()]] {
        assert!(tooling::run(&args, &fixture.child, &fixture.root).is_err());
    }
    let output = Command::new(env!("CARGO_BIN_EXE_development"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stderr, b"missing development command\n");
    let blocked = fixture.root.join("blocked");
    fs::write(&blocked, "not a directory").unwrap();
    assert!(
        tooling::copy_assets::copy(&source, &blocked, tooling::copy_assets::Layout::Library)
            .is_err()
    );
    assert_asset_match_order();
}

#[cfg(unix)]
#[test]
fn maestro_asset_links_keep_layout_semantics() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let source = asset_inputs(&fixture);
    let themes = source.join("src/modes/interactive/theme");
    fs::write(source.join("real-theme"), "linked bytes").unwrap();
    symlink(source.join("real-theme"), themes.join("linked.json")).unwrap();
    let destination = fixture.root.join("dist");
    tooling::copy_assets::copy(&source, &destination, tooling::copy_assets::Layout::Library)
        .unwrap();
    let copied = destination.join("modes/interactive/theme/linked.json");
    assert!(!fs::symlink_metadata(&copied).unwrap().is_symlink());
    assert_eq!(fs::read(copied).unwrap(), b"linked bytes");
    symlink(source.join("absent"), themes.join("dangling.json")).unwrap();
    assert!(
        tooling::copy_assets::copy(&source, &destination, tooling::copy_assets::Layout::Library)
            .is_err()
    );
    fs::remove_file(themes.join("dangling.json")).unwrap();
    let package = fixture.root.join("package");
    for name in ["Cargo.toml", "viewer/index.html"] {
        write_input(&source, name, name);
    }
    for name in [
        "README.md",
        "CHANGELOG.md",
        "docs/content",
        "examples/content",
    ] {
        write_input(&package, name, name);
    }
    symlink("../examples", package.join("docs/linked-dir")).unwrap();
    symlink("missing", package.join("docs/dangling")).unwrap();
    let standalone = fixture.root.join("standalone");
    tooling::copy_assets::copy(
        &source,
        &standalone,
        tooling::copy_assets::Layout::Standalone {
            metadata: &source.join("Cargo.toml"),
            viewer: &source.join("viewer"),
            package: &package,
        },
    )
    .unwrap();
    assert_eq!(
        fs::read_link(standalone.join("docs/linked-dir")).unwrap(),
        Path::new("../examples")
    );
    assert_eq!(
        fs::read_link(standalone.join("docs/dangling")).unwrap(),
        Path::new("missing")
    );
}

#[cfg(unix)]
#[test]
fn maestro_standalone_assets_preserve_symlinked_directory_roots() {
    let fixture = Fixture::new();
    let source = asset_inputs(&fixture);
    let package = fixture.root.join("package");
    standalone_inputs(&source, &package);
    fs::rename(package.join("docs"), package.join("real-docs")).unwrap();
    std::os::unix::fs::symlink("real-docs", package.join("docs")).unwrap();
    let destination = fixture.root.join("standalone");
    tooling::copy_assets::copy(
        &source,
        &destination,
        tooling::copy_assets::Layout::Standalone {
            metadata: &source.join("Cargo.toml"),
            viewer: &source.join("viewer"),
            package: &package,
        },
    )
    .unwrap();
    assert!(
        fs::symlink_metadata(destination.join("docs"))
            .unwrap()
            .is_symlink()
    );
    assert_eq!(
        fs::read_link(destination.join("docs")).unwrap(),
        Path::new("real-docs")
    );
}

const BROWSER_BUILD_ARGS: [&str; 8] = [
    "build",
    "--locked",
    "-p",
    "maestro-models",
    "--example",
    "browser_import_check",
    "--target",
    "wasm32-unknown-unknown",
];

#[test]
fn maestro_browser_build_records_diagnostics_without_noise() {
    drive();
    let fixture = Fixture::new();
    let output = fixture
        .invoke(
            "maestro_browser_build_records_diagnostics_without_noise",
            "check-browser-smoke",
            &[],
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        output.stderr,
        b"browser_import_check entry is not delivered\n"
    );
    let entry = fixture
        .root
        .join("crates/maestro-models/examples/browser_import_check.rs");
    fs::create_dir_all(entry.parent().unwrap()).unwrap();
    fs::write(entry, "controlled build input").unwrap();
    let log = fixture.root.join("maestro-browser-smoke-errors.log");
    let artifact = fixture.root.join("artifact.wasm");
    for exit in ["0", "17"] {
        let output = fixture
            .invoke(
                "maestro_browser_build_records_diagnostics_without_noise",
                "check-browser-smoke",
                &[],
            )
            .temp_dir(&fixture.root)
            .env("MAESTRO_CHILD_EXIT", exit)
            .env("MAESTRO_ARTIFACT", &artifact)
            .env("MAESTRO_DIAGNOSTIC", "fixture.rs:4: compilation failed\n")
            .output()
            .unwrap();
        assert_eq!(output.status.success(), exit == "0");
        assert_eq!(
            fixture.arguments(),
            BROWSER_BUILD_ARGS.map(|arg| arg.as_bytes().to_vec())
        );
        if exit == "0" {
            assert!(output.stderr.is_empty());
            assert!(!log.exists());
            assert_eq!(fs::read(&artifact).unwrap(), b"compiled artifact");
        } else {
            assert_eq!(
                output.stderr,
                format!("Browser smoke check failed. See {}\n", log.display()).as_bytes()
            );
            assert_eq!(
                fs::read(&log).unwrap(),
                b"fixture.rs:4: compilation failed\n"
            );
        }
        assert!(!stdout(&output).contains("compilation failed"));
    }
}

fn git(root: &Path, args: &[&str]) -> Output {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn stage_deleted_path(root: &Path) {
    fs::write(root.join("deleted.rs"), "deleted").unwrap();
    git(root, &["add", "deleted.rs"]);
    git(
        root,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        ],
    );
    fs::remove_file(root.join("deleted.rs")).unwrap();
    git(root, &["add", "--", "deleted.rs"]);
}

#[test]
fn maestro_hook_restages_only_captured_paths() {
    assert_hook_restages_only_captured_paths();
}

#[test]
fn maestro_hook_fixtures_exclude_ambient_credentials() {
    if std::env::var_os("MAESTRO_DRIVER").is_none() {
        let fixture = Fixture::new();
        let output = fixture
            .invoke(
                "maestro_hook_fixtures_exclude_ambient_credentials",
                "fixture-hook",
                &[],
            )
            .envs(
                ["RUSTUP_HOME", "CARGO_HOME", "RUSTUP_AUTO_INSTALL"]
                    .into_iter()
                    .filter_map(|key| std::env::var_os(key).map(|value| (key, value))),
            )
            .env("SYNTHETIC_TEST_API_KEY", "secret-value")
            .env("PATH", std::env::var_os("PATH").unwrap())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    assert_eq!(
        std::env::var("SYNTHETIC_TEST_API_KEY").unwrap(),
        "secret-value"
    );
    assert_hook_restages_only_captured_paths();
}

fn assert_hook_restages_only_captured_paths() {
    let fixture = Fixture::new();
    git(&fixture.root, &["init", "--quiet"]);
    for name in ["a space.rs", "line\nbreak.rs", "-dash.rs", "unrelated.rs"] {
        fs::write(fixture.root.join(name), "old").unwrap();
    }
    git(
        &fixture.root,
        &["add", "--", "a space.rs", "line\nbreak.rs", "-dash.rs"],
    );
    stage_deleted_path(&fixture.root);
    for name in ["a space.rs", "line\nbreak.rs", "-dash.rs"] {
        fs::write(fixture.root.join(name), "staged").unwrap();
    }
    git(
        &fixture.root,
        &["add", "--", "a space.rs", "line\nbreak.rs", "-dash.rs"],
    );
    let mut format = Command::new(&fixture.child);
    format
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("TMPDIR", &fixture.root)
        .env("MAESTRO_RECEIPT", fixture.root.join("receipt"))
        .env("HOME", &fixture.root)
        .env("MAESTRO_FORMAT_ROOT", &fixture.root);
    let mut check = Command::new(&fixture.child);
    check
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("TMPDIR", &fixture.root)
        .env("MAESTRO_RECEIPT", fixture.root.join("receipt"))
        .env("HOME", &fixture.root);
    check.env("MAESTRO_CHECK_INDEX", &fixture.root);
    let mut unused = Command::new("missing-tool");
    tooling::pre_commit::run(&fixture.root, &mut format, &mut check, &mut unused, None).unwrap();
    for entry in fs::read_dir(fixture.root.join("receipt/env")).unwrap() {
        assert_ne!(fs::read(entry.unwrap().path()).unwrap(), b"secret-value");
    }
    for name in ["a space.rs", "line\nbreak.rs", "-dash.rs"] {
        let output = git(&fixture.root, &["show", &format!(":{name}")]);
        assert_eq!(output.stdout, b"formatted");
    }
    assert_eq!(
        fs::read(fixture.root.join("unrelated.rs")).unwrap(),
        b"formatted"
    );
    assert!(
        !git(&fixture.root, &["diff", "--cached", "--name-only"])
            .stdout
            .windows(12)
            .any(|part| part == b"unrelated.rs")
    );
    assert_eq!(
        git(
            &fixture.root,
            &["diff", "--cached", "--diff-filter=D", "--name-only"]
        )
        .stdout,
        b"deleted.rs\n"
    );
}

fn assert_hook_phase(phase: &str) {
    let fixture = Fixture::new();
    git(&fixture.root, &["init", "--quiet"]);
    fs::write(fixture.root.join("Cargo.toml"), "manifest").unwrap();
    let checker = fixture
        .root
        .join("crates/maestro-test-conventions/src/lib.rs");
    fs::create_dir_all(checker.parent().unwrap()).unwrap();
    fs::write(checker, "checker").unwrap();
    git(&fixture.root, &["add", "Cargo.toml", "crates"]);
    let output = fixture
        .invoke("maestro_hook_messages_follow_phase_results", "hook", &[])
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("MAESTRO_FAIL_PHASE", phase)
        .env("MAESTRO_ACTIVE_SMOKE", "true")
        .output()
        .unwrap();
    let text = stdout(&output);
    assert!(text.contains("Running formatting, linting, and type checking...\n"));
    assert_eq!(
        text.contains("Running the full conventions suite...\n"),
        matches!(phase, "conventions" | "smoke" | "success")
    );
    assert_eq!(
        text.contains("Running browser smoke check...\n"),
        matches!(phase, "smoke" | "success")
    );
    assert_eq!(
        text.contains("✅ All pre-commit checks passed!\n"),
        phase == "success"
    );
    assert!(output.stderr.is_empty());
    let (announcement, failure) = match phase {
        "format" | "check" => (
            "Running formatting, linting, and type checking...\n",
            "❌ Checks failed. Please fix the errors before committing.\n",
        ),
        "conventions" => (
            "Running the full conventions suite...\n",
            "❌ Conventions suite failed.\n",
        ),
        "smoke" => (
            "Running browser smoke check...\n",
            "❌ Browser smoke check failed.\n",
        ),
        _ => ("", ""),
    };
    if !failure.is_empty() {
        assert!(text.ends_with(failure), "{text}");
        assert!(text.find(announcement).unwrap() < text.find(failure).unwrap());
    }
    assert_eq!(output.status.success(), phase == "success");
    assert_eq!(
        fixture.receipt("phases"),
        match phase {
            "format" => b"format\n".as_slice(),
            "check" => b"format\ncheck\n",
            "conventions" => b"format\ncheck\nconventions\n",
            _ => b"format\ncheck\nconventions\nsmoke\n",
        }
    );
}

#[test]
fn maestro_hook_messages_follow_phase_results() {
    drive();
    for phase in ["format", "check", "conventions", "smoke", "success"] {
        assert_hook_phase(phase);
    }

    let fixture = Fixture::new();
    git(&fixture.root, &["init", "--quiet"]);
    let output = fixture
        .invoke(
            "maestro_hook_messages_follow_phase_results",
            "pre-commit",
            &[],
        )
        .env(
            "PATH",
            format!(
                "{}:{}",
                fixture.root.display(),
                std::env::var("PATH").unwrap()
            ),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout(&output).contains("✅ All pre-commit checks passed!\n"));
    assert_eq!(fixture.receipt("phases"), b"fmt\ncheck\n");
}

#[test]
fn maestro_hook_selects_extra_steps_from_original_paths() {
    drive();
    for (name, active, expected) in [
        ("crates/maestro-models/src/lib.rs", true, "smoke\n"),
        ("crates/maestro-web/src/lib.rs", true, "smoke\n"),
        ("Cargo.toml", true, "smoke\n"),
        ("Cargo.lock", true, "smoke\n"),
        ("crates/maestro-agent/src/lib.rs", true, ""),
        ("crates/maestro-models-old/src/lib.rs", true, ""),
        ("Cargo.toml", false, ""),
        (
            "crates/maestro-test-conventions/src/lib.rs",
            true,
            "conventions\n",
        ),
        (
            "crates/maestro-test-conventions/tests/integration.rs",
            false,
            "conventions\n",
        ),
        ("crates/maestro-test-conventions-old/src/lib.rs", true, ""),
    ] {
        let fixture = Fixture::new();
        git(&fixture.root, &["init", "--quiet"]);
        let path = fixture.root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "captured").unwrap();
        git(&fixture.root, &["add", "--", name]);
        fs::remove_file(&path).unwrap();
        let output = fixture
            .invoke(
                "maestro_hook_selects_extra_steps_from_original_paths",
                "hook",
                &[],
            )
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("MAESTRO_ACTIVE_SMOKE", active.to_string())
            .output()
            .unwrap();
        assert!(output.status.success());
        let text = stdout(&output);
        assert_eq!(
            text.contains("Running the full conventions suite...\n"),
            expected.contains("conventions"),
            "{name}"
        );
        assert_eq!(
            text.contains("Running browser smoke check...\n"),
            expected.contains("smoke"),
            "{name}"
        );
        assert_eq!(
            fixture.receipt("phases"),
            format!("format\ncheck\n{expected}").as_bytes(),
            "{name}"
        );
    }
}

#[cfg(unix)]
#[test]
fn maestro_source_announces_non_executable_cargo() {
    use std::os::unix::fs::PermissionsExt;
    drive();
    let fixture = Fixture::new();
    fs::set_permissions(&fixture.child, fs::Permissions::from_mode(0o644)).unwrap();
    let output = fixture
        .invoke(
            "maestro_source_announces_non_executable_cargo",
            "run-source",
            &[],
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        output.stderr,
        format!(
            "cargo not found at {}. Run just setup from the repo root first.\n",
            fixture.child.display()
        )
        .as_bytes()
    );
}

#[test]
fn maestro_browser_missing_tool_logs_smoke_failure() {
    drive();
    let fixture = Fixture::new();
    let entry = fixture
        .root
        .join("crates/maestro-models/examples/browser_import_check.rs");
    fs::create_dir_all(entry.parent().unwrap()).unwrap();
    fs::write(entry, "fixture").unwrap();
    fs::remove_file(&fixture.child).unwrap();
    let output = fixture
        .invoke(
            "maestro_browser_missing_tool_logs_smoke_failure",
            "check-browser-smoke",
            &[],
        )
        .temp_dir(&fixture.root)
        .output()
        .unwrap();
    let log = fixture.root.join("maestro-browser-smoke-errors.log");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        output.stderr,
        format!("Browser smoke check failed. See {}\n", log.display()).as_bytes()
    );
    assert!(
        fs::read_to_string(log)
            .unwrap()
            .contains("No such file or directory")
    );
}

#[test]
fn maestro_hook_missing_tool_announces_failed_step() {
    drive();
    for phase in ["format", "check", "conventions", "smoke"] {
        let fixture = Fixture::new();
        git(&fixture.root, &["init", "--quiet"]);
        fs::write(fixture.root.join("Cargo.toml"), "manifest").unwrap();
        let checker = fixture
            .root
            .join("crates/maestro-test-conventions/src/lib.rs");
        fs::create_dir_all(checker.parent().unwrap()).unwrap();
        fs::write(checker, "checker").unwrap();
        git(&fixture.root, &["add", "Cargo.toml", "crates"]);
        let output = fixture
            .invoke(
                "maestro_hook_missing_tool_announces_failed_step",
                "hook",
                &[],
            )
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("MAESTRO_MISSING_PHASE", phase)
            .env("MAESTRO_ACTIVE_SMOKE", "true")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        let message = match phase {
            "conventions" => "❌ Conventions suite failed.\n",
            "smoke" => "❌ Browser smoke check failed.\n",
            _ => "❌ Checks failed. Please fix the errors before committing.\n",
        };
        assert!(stdout(&output).ends_with(message), "{}", stdout(&output));
        assert!(!stdout(&output).contains("All pre-commit checks passed"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("No such file or directory"));
    }
}

/// Run the test build's author preparation only while it is newer than its sources.
#[cfg(unix)]
#[test]
fn author_setup_runs_the_built_example_only_while_fresh() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, SystemTime};
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    let profile = root.join("target/debug");
    fs::create_dir_all(profile.join("deps")).unwrap();
    fs::create_dir_all(profile.join("examples")).unwrap();
    let sources = [
        "crates/maestro-extensions-wasm/examples/prepare_author_component.rs",
        "crates/maestro-extensions-wasm/src/tests/build.rs",
        "crates/maestro-extensions-wasm/Cargo.toml",
        "Cargo.lock",
    ];
    let earlier = SystemTime::now() - Duration::from_secs(3600);
    for source in sources {
        let path = root.join(source);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::File::create(path)
            .unwrap()
            .set_modified(earlier)
            .unwrap();
    }
    let example = profile.join("examples/prepare_author_component");
    for (path, name) in [(&example, "example"), (&root.join("cargo"), "cargo")] {
        fs::write(path, format!("#!/bin/sh\necho {name} \"$@\"\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let run = || run_author_setup(root, &profile);
    assert_eq!(run(), "example\n");
    for source in sources {
        let file = fs::File::options()
            .write(true)
            .open(root.join(source))
            .unwrap();
        file.set_modified(SystemTime::now() + Duration::from_secs(60))
            .unwrap();
        assert!(run().starts_with("cargo run "), "{source}");
        file.set_modified(earlier).unwrap();
    }
    fs::remove_file(&example).unwrap();
    assert!(run().starts_with("cargo run "));
}

/// Run the author setup script from `root` with `profile` as nextest's artifact directory.
#[cfg(unix)]
fn run_author_setup(root: &Path, profile: &Path) -> String {
    let output = Command::new("sh")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.config/prepare-author-component.sh"),
        )
        .current_dir(root)
        .env("NEXTEST_LD_LIBRARY_PATH", profile)
        .env("NEXTEST_DYLD_FALLBACK_LIBRARY_PATH", profile)
        .env("CARGO", root.join("cargo"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout(&output)
}
