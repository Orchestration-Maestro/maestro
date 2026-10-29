#![cfg(test)]

use crate::recipe_support::{self, write_input};
use crate::toolchain_support;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[cfg(unix)]
use maestro_test_directory::Scratch;
use maestro_test_process::TestProcess;
use maestro_tooling as tooling;

/// Exercise Rustup selection with an incomplete default and an installed pin.
#[test]
fn child_toolchain_uses_declared_pin() {
    if std::env::var_os("MAESTRO_TOOLCHAIN_CONTROL").is_none() {
        let owner = Fixture::new();
        let home = owner.root.path().join("rustup");
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
        fs::read(fixture.root.path().join("rust-toolchain.toml")).unwrap(),
        include_bytes!("../../../rust-toolchain.toml")
    );
    check_declared_children(&fixture);
    let declaration = include_str!("../../../rust-toolchain.toml");
    assert!(
        toolchain_support::declare_toolchain(&fixture.root.path().join("missing"), declaration)
            .is_err()
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
        .current_dir(fixture.root.path())
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
        .env("MAESTRO_RECEIPT", fixture.root.path().join("receipt"))
        .env("HOME", fixture.root.path())
        .status()
        .unwrap();
    assert!(run.success());
    println!("declared compiler and emitted executable: passed");
    check_declared_metadata(fixture);
    fs::write(
        fixture.root.path().join("justfile"),
        "probe:\n    rustc --print sysroot\n",
    )
    .unwrap();
    let recipe = toolchain_support::command("just")
        .current_dir(fixture.root.path())
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
        fixture.root.path().join("Cargo.toml"),
        "[package]\nname = 'toolchain-probe'\nversion = '0.0.0'\nedition = '2024'\n[workspace]\n",
    )
    .unwrap();
    fs::create_dir(fixture.root.path().join("src")).unwrap();
    fs::write(fixture.root.path().join("src/lib.rs"), "").unwrap();
    let metadata = toolchain_support::command("cargo")
        .current_dir(fixture.root.path())
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
        fixture.root.path().join("Cargo.toml"),
        "[package]\nname = 'wasm-child'\nversion = '0.0.0'\nedition = '2024'\n[workspace]\n",
    )
    .unwrap();
    fs::create_dir(fixture.root.path().join("src")).unwrap();
    fs::write(
        fixture.root.path().join("src/lib.rs"),
        "pub fn value() -> u32 { 7 }\n",
    )
    .unwrap();
    let binary = compile_toolchain_probe(&fixture);
    let home = fixture.root.path().join("cargo-home");
    fs::remove_dir_all(&home).unwrap();
    let profile = fixture.root.path().join("parent.profraw");
    let parent = Command::new(&binary)
        .current_dir(fixture.root.path())
        .env("CARGO_HOME", &home)
        .env("MAESTRO_PROBE_CARGO", env!("CARGO"))
        .env("CARGO_TARGET_DIR", fixture.root.path().join("target"))
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
            .path()
            .join("target/wasm32-unknown-unknown/debug/libwasm_child.rlib")
            .is_file()
    );
    assert!(
        home.join(".package-cache").is_file(),
        "the probe's wasm build must use the private Cargo home"
    );
    assert_profile_counters(&profile);
}

/// Generate a fixture's lock file with its Cargo state kept in a private home.
fn generate_private_lock(root: &Path) {
    assert!(
        toolchain_support::fixture_command(env!("CARGO"), root, None)
            .unwrap()
            .arg("generate-lockfile")
            .status()
            .unwrap()
            .success()
    );
    assert!(
        root.join("cargo-home/.package-cache").is_file(),
        "lock generation must use a private Cargo home"
    );
}

/// Compile an instrumented parent independently of the integration runner's instrumentation.
fn compile_toolchain_probe(fixture: &Fixture) -> PathBuf {
    generate_private_lock(fixture.root.path());
    fs::write(
        fixture.root.path().join("toolchain_support.rs"),
        include_str!("fixtures/toolchain_support.rs"),
    )
    .unwrap();
    let source = fixture.root.path().join("parent.rs");
    fs::write(&source, include_str!("fixtures/child_toolchain_probe.rs")).unwrap();
    let binary = fixture.root.path().join("parent");
    let compile = toolchain_support::command("rustc")
        .current_dir(fixture.root.path())
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

/// Controlled tool recording its arguments, environment and phases.
const FIXTURE_CHILD: &str = r#"
use std::{env, fs, io::Write, path::PathBuf};
fn main() {
    let root = PathBuf::from(env::var_os("MAESTRO_RECEIPT").unwrap());
    fs::create_dir_all(root.join("args")).unwrap();
    for (i, arg) in env::args_os().skip(1).enumerate() {
        fs::write(root.join("args").join(i.to_string()), arg.as_encoded_bytes()).unwrap();
    }
    let mut invocations = fs::OpenOptions::new().create(true).append(true).open(root.join("invocations")).unwrap();
    writeln!(invocations, "{}", env::args().skip(1).collect::<Vec<_>>().join("|")).unwrap();
    let env_root = root.join("env");
    fs::create_dir_all(&env_root).unwrap();
    let recorded = env_root.join(fs::read_dir(&env_root).unwrap().count().to_string());
    fs::create_dir(&recorded).unwrap();
    for (key, value) in env::vars_os() {
        fs::write(recorded.join(key), value.as_encoded_bytes()).unwrap();
    }
    if env::args().nth(1).as_deref() == Some("metadata") {
        if let Some(path) = env::var_os("MAESTRO_METADATA") {
            match env::var("MAESTRO_METADATA_MODE").as_deref() {
                Ok("garbage") => println!("not metadata"),
                _ => println!("{}", fs::read_to_string(path).unwrap()),
            }
            if let Ok(code) = env::var("MAESTRO_METADATA_EXIT") {
                std::process::exit(code.parse().unwrap());
            }
            std::process::exit(0);
        }
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
    if env::args().nth(1).as_deref() != Some("format") {
        if let Some(root) = env::var_os("MAESTRO_CHECK_INDEX") {
            for name in ["a space.rs", "line\nbreak.rs", "-dash.rs"].into_iter().filter(|name| !(cfg!(windows) && name.contains('\n'))) {
                let output = std::process::Command::new("git").current_dir(&root).args(["show", &format!(":{name}")]).output().unwrap();
                assert!(output.status.success());
                assert_eq!(output.stdout, b"formatted");
            }
        }
    }
    if let Some(format) = env::var_os("MAESTRO_FORMAT_ROOT") {
        for name in ["a space.rs", "line\nbreak.rs", "-dash.rs", "unrelated.rs"].into_iter().filter(|name| !(cfg!(windows) && name.contains('\n'))) {
            fs::write(PathBuf::from(&format).join(name), "formatted").unwrap();
        }
    }
    if let Some(path) = env::var_os("MAESTRO_ARTIFACT") { fs::write(path, "compiled artifact").unwrap(); }
    if let Some(path) = env::var_os("MAESTRO_BLOCK_RESTORE") { fs::create_dir_all(PathBuf::from(path).join("auth.json.bak")).unwrap(); }
    if let Ok(message) = env::var("MAESTRO_DIAGNOSTIC") { eprint!("{message}"); }
    std::process::exit(env::var("MAESTRO_CHILD_EXIT").unwrap_or_default().parse().unwrap_or(0));
}
"#;

struct Fixture {
    root: maestro_test_directory::Scratch,
    child: PathBuf,
    /// Retains the deliberately spaced and apostrophe-containing parent of `root`.
    _parent: maestro_test_directory::Scratch,
}

impl Fixture {
    fn new() -> Self {
        let parent = maestro_test_directory::Scratch::new().unwrap();
        let spelled = parent.path().join("Maestro's tooling fixture");
        fs::create_dir(&spelled).unwrap();
        let root = maestro_test_directory::Scratch::new_in(&spelled).unwrap();
        toolchain_support::declare_toolchain(
            root.path(),
            include_str!("../../../rust-toolchain.toml"),
        )
        .unwrap();
        fs::create_dir(root.path().join("receipt")).unwrap();
        fs::create_dir_all(root.path().join(".maestro/agent")).unwrap();
        let source = root.path().join("child.rs");
        fs::write(&source, FIXTURE_CHILD).unwrap();
        let child = root
            .path()
            .join(format!("just{}", std::env::consts::EXE_SUFFIX));
        compile_fixture(root.path(), &source, &child);
        Self {
            root,
            child,
            _parent: parent,
        }
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
            .env("MAESTRO_FIXTURE_ROOT", self.root.path())
            .env("MAESTRO_RECEIPT", self.root.path().join("receipt"))
            .env("HOME", self.root.path())
            .env("PATH", self.root.path())
            .current_dir(self.root.path());
        if let Some(channel) = toolchain_support::channel() {
            command = command.env("RUSTUP_TOOLCHAIN", channel);
        }
        for (index, arg) in args.iter().enumerate() {
            command = command.env(format!("MAESTRO_ARG_{index}"), arg);
        }
        command
    }

    fn auth(&self) -> PathBuf {
        self.root.path().join(".maestro/agent/auth.json")
    }
    fn receipt(&self, name: &str) -> Vec<u8> {
        fs::read(self.root.path().join("receipt").join(name)).unwrap()
    }
    fn arguments(&self) -> Vec<Vec<u8>> {
        let root = self.root.path().join("receipt/args");
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

/// Environment the controlled tool recorded for its latest invocation.
fn recorded_environment(fixture: &Fixture) -> PathBuf {
    let root = fixture.root.path().join("receipt/env");
    let latest = fs::read_dir(&root)
        .unwrap()
        .map(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .parse::<usize>()
                .unwrap()
        })
        .max()
        .unwrap();
    root.join(latest.to_string())
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

/// Keep only the fixture's own environment for a controlled tool command.
fn isolated(mut command: Command) -> Command {
    command.env_clear();
    for (key, value) in std::env::vars_os() {
        let keep = ["PATH", "HOME", "TMPDIR", "SYSTEMROOT", "WINDIR"]
            .iter()
            .any(|name| key == *name);
        if keep || key.to_string_lossy().starts_with("MAESTRO_") {
            command.env(key, value);
        }
    }
    command
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
        let mut format = isolated(Command::new(&cargo));
        format.arg("format");
        let mut conventions = isolated(Command::new(&cargo));
        conventions.arg("conventions");
        let mut smoke = isolated(Command::new(&cargo));
        smoke.arg("smoke");
        match std::env::var("MAESTRO_MISSING_PHASE").as_deref() {
            Ok("format") => format = Command::new("missing-tool"),
            Ok("conventions") => conventions = Command::new("missing-tool"),
            Ok("smoke") => smoke = Command::new("missing-tool"),
            _ => {}
        }
        let tool = if std::env::var_os("MAESTRO_MISSING_METADATA_TOOL").is_some() {
            Path::new("missing-tool")
        } else {
            Path::new(&cargo)
        };
        tooling::pre_commit::run(
            Path::new(&root),
            tool,
            &mut format,
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

/// The operating system's own message for the program `program` it cannot start.
fn missing_program_message(program: impl AsRef<std::ffi::OsStr>) -> String {
    Command::new(program).output().unwrap_err().to_string()
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
            assert!(!recorded_environment(&fixture).join(key).exists(), "{key}");
        }
        for key in [
            "AZURE_OPENAI_API_KEY",
            "MAESTRO_CANARY",
            "TMPDIR",
            "XDG_CONFIG_HOME",
            "MAESTRO_AGENT_DIR",
        ] {
            assert_eq!(
                fs::read(recorded_environment(&fixture).join(key)).unwrap(),
                b"kept"
            );
        }
        assert_eq!(
            fs::read(recorded_environment(&fixture).join("MAESTRO_NO_LOCAL_LLM")).unwrap(),
            b"1"
        );
        assert_eq!(
            fs::read(recorded_environment(&fixture).join("HOME")).unwrap(),
            fixture.root.path().as_os_str().as_encoded_bytes()
        );
        assert!(stdout(&output).contains("Running tests without API keys...\n"));
    }
}

#[test]
fn maestro_source_reports_missing_tool_and_child_failure() {
    drive();
    let fixture = Fixture::new();
    let missing = fixture.root.path().join("missing-cargo");
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
    assert!(!fixture.root.path().join("receipt/auth-present").exists());
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
    let target = fixture.root.path().join("auth-target");
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
        fixture.root.path().as_os_str().as_encoded_bytes()
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
                assert!(!recorded_environment(&fixture).join(key).exists(), "{key}");
            }
            for key in [
                "AZURE_OPENAI_API_KEY",
                "AZURE_OPENAI_BASE_URL",
                "AZURE_OPENAI_RESOURCE_NAME",
            ] {
                assert!(!recorded_environment(&fixture).join(key).exists());
            }
            assert_eq!(
                fs::read(recorded_environment(&fixture).join("BEDROCK_EXTENSIVE_MODEL_TEST"))
                    .unwrap(),
                value.as_bytes()
            );
            assert_eq!(
                fs::read(recorded_environment(&fixture).join("MAESTRO_NO_LOCAL_LLM")).unwrap(),
                b"kept"
            );
            assert_eq!(
                fs::read(recorded_environment(&fixture).join("MAESTRO_CANARY")).unwrap(),
                b"kept"
            );
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
                .path()
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
        let cwd = fixture.root.path().as_os_str().as_encoded_bytes();
        assert_eq!(fixture.receipt("cwd").as_slice(), cwd);
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
        assert_eq!(
            fs::read(recorded_environment(&fixture).join(key)).unwrap(),
            b"kept"
        );
    }
    assert_eq!(
        fs::read(recorded_environment(&fixture).join("MAESTRO_NO_LOCAL_LLM")).unwrap(),
        b"kept"
    );
    assert_eq!(fixture.receipt("auth-present"), b"yes");
    assert_eq!(fs::read(fixture.auth()).unwrap(), b"auth");
    assert!(!stdout(&output).contains("Running without API keys"));
}

fn asset_inputs(fixture: &Fixture) -> PathBuf {
    let source = fixture.root.path().join("assets-source");
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
    let destination = fixture.root.path().join("dist");
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
    crate::build_recipes::copy_tree(&repository_examples(), &package.join("examples"));
}

/// Assert that a viewer directory with no entries copies as an empty export directory in a fresh
/// destination, while an existing destination export directory keeps the entries it holds.
#[test]
fn maestro_empty_viewer_copies_an_empty_directory() {
    for (existing, prior) in [(false, None), (true, None), (true, Some("prior"))] {
        let fixture = Fixture::new();
        let (source, package, metadata, viewer) = split_inputs(&fixture);
        fs::remove_file(viewer.join("index.html")).unwrap();
        let destination = fixture.root.path().join("standalone");
        if existing {
            fs::create_dir_all(destination.join("export-html")).unwrap();
            fs::write(destination.join("unrelated"), "kept").unwrap();
        }
        if let Some(bytes) = prior {
            fs::write(destination.join("export-html").join("kept"), bytes).unwrap();
        }
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
        let copied = destination.join("export-html");
        assert!(copied.is_dir());
        assert_eq!(
            fs::read_dir(&copied).unwrap().count(),
            usize::from(prior.is_some())
        );
        if let Some(bytes) = prior {
            assert_eq!(fs::read(copied.join("kept")).unwrap(), bytes.as_bytes());
        }
        assert_eq!(fs::read(destination.join("README.md")).unwrap(), b"readme");
        assert_eq!(
            fs::read(destination.join("docs/guide.md")).unwrap(),
            b"guide"
        );
        assert_eq!(
            fs::read(destination.join("examples/nested/demo.rs")).unwrap(),
            b"example"
        );
        if existing {
            assert_eq!(fs::read(destination.join("unrelated")).unwrap(), b"kept");
        }
    }
}

/// Assert that an absent viewer path fails as an unreadable input, not as a policy.
#[test]
fn maestro_missing_viewer_names_the_absent_path() {
    let fixture = Fixture::new();
    let (source, package, metadata, _) = split_inputs(&fixture);
    let viewer = fixture.root.path().join("absent-viewer");
    let destination = fixture.root.path().join("standalone");
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
    assert_names_input(&error, &viewer);
    assert_eq!(
        error.to_string(),
        format!(
            "cannot copy {}: {}",
            viewer.display(),
            fs::symlink_metadata(&viewer).unwrap_err()
        )
    );
    for name in [
        "Cargo.toml",
        "README.md",
        "CHANGELOG.md",
        "theme/schema.json",
        "assets/mark.png",
    ] {
        assert!(destination.join(name).is_file(), "{name} was not copied");
    }
    for name in ["export-html", "docs", "examples"] {
        assert!(!destination.join(name).exists(), "{name} was created");
    }
}

/// Assert that a viewer of another filesystem kind keeps that kind at the destination.
#[test]
fn maestro_viewer_entries_keep_recursive_copy_semantics() {
    let fixture = Fixture::new();
    let (source, package, metadata, _) = split_inputs(&fixture);
    let viewer = fixture.root.path().join("viewer-file");
    fs::write(&viewer, "viewer-file").unwrap();
    let destination = fixture.root.path().join("file-destination");
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
    let copied = destination.join("export-html");
    assert!(fs::symlink_metadata(&copied).unwrap().is_file());
    assert_eq!(fs::read(copied).unwrap(), b"viewer-file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let empty = fixture.root.path().join("empty-target");
        fs::create_dir(&empty).unwrap();
        let populated = fixture.root.path().join("populated-target");
        write_input(&populated, "index.html", "viewer");
        let file = fixture.root.path().join("file-target");
        fs::write(&file, "target-file").unwrap();
        for (name, target) in [
            ("linked-empty", "empty-target"),
            ("linked-populated", "populated-target"),
            ("linked-file", "file-target"),
            ("dangling", "absent-target"),
        ] {
            let link = fixture.root.path().join(name);
            symlink(target, &link).unwrap();
            let destination = fixture.root.path().join(format!("{name}-destination"));
            tooling::copy_assets::copy(
                &source,
                &destination,
                tooling::copy_assets::Layout::Standalone {
                    metadata: &metadata,
                    viewer: &link,
                    package: &package,
                },
            )
            .unwrap();
            let copied = destination.join("export-html");
            assert!(
                fs::symlink_metadata(&copied).unwrap().is_symlink(),
                "{name}"
            );
            assert_eq!(fs::read_link(&copied).unwrap(), Path::new(target), "{name}");
        }
    }
}

#[test]
fn maestro_standalone_assets_preserve_recursive_layout() {
    let fixture = Fixture::new();
    let source = asset_inputs(&fixture);
    let package = fixture.root.path().join("package");
    standalone_inputs(&source, &package);
    let destination = fixture.root.path().join("standalone");
    fs::create_dir_all(source.join("src")).unwrap();
    fs::write(source.join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(
        fixture.root.path().join("Cargo.toml"),
        "[workspace]\nmembers = ['assets-source']\nresolver = '3'\n",
    )
    .unwrap();
    generate_private_lock(fixture.root.path());
    let args = [
        "copy-binary-assets".into(),
        source.clone().into_os_string(),
        destination.clone().into_os_string(),
        source.join("viewer").into_os_string(),
        package.clone().into_os_string(),
    ];
    assert_eq!(
        tooling::run(&args, Path::new(env!("CARGO")), fixture.root.path()).unwrap(),
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
    assert_example_tree(&repository_examples(), &destination.join("examples"));
    assert_empty_viewer_copy(&fixture, &source, &package);
    fs::write(
        source.join("Cargo.toml"),
        "[package]\nname = 'not-the-binary'\nversion = '0.1.0'\n",
    )
    .unwrap();
    assert!(tooling::run(&args, Path::new(env!("CARGO")), fixture.root.path()).is_err());
}

/// Assert that the command route copies an empty viewer and the later groups.
fn assert_empty_viewer_copy(fixture: &Fixture, source: &Path, package: &Path) {
    let viewer = fixture.root.path().join("empty-viewer");
    fs::create_dir(&viewer).unwrap();
    let destination = fixture.root.path().join("standalone-empty");
    let args = [
        "copy-binary-assets".into(),
        source.to_path_buf().into_os_string(),
        destination.clone().into_os_string(),
        viewer.into_os_string(),
        package.to_path_buf().into_os_string(),
    ];
    assert_eq!(
        tooling::run(&args, Path::new(env!("CARGO")), fixture.root.path()).unwrap(),
        std::process::ExitCode::SUCCESS
    );
    let copied = destination.join("export-html");
    assert!(copied.is_dir());
    assert_eq!(fs::read_dir(copied).unwrap().count(), 0);
    for (name, bytes) in [
        ("README.md", "readme"),
        ("docs/guide.md", "guide"),
        ("examples/nested/demo.rs", "example"),
    ] {
        assert_eq!(fs::read(destination.join(name)).unwrap(), bytes.as_bytes());
    }
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
    let package = fixture.root.path().join("package");
    package_inputs(&package);
    let metadata = fixture.root.path().join("binary/Cargo.toml");
    write_input(
        &fixture.root.path().join("binary"),
        "Cargo.toml",
        "manifest",
    );
    let viewer = fixture.root.path().join("viewer");
    write_input(&viewer, "index.html", "viewer");
    (source, package, metadata, viewer)
}

/// Locate the repository's authored examples from this crate's manifest directory.
fn repository_examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

/// Assert every source file reaches the matching destination path byte for byte.
fn assert_example_tree(source: &Path, destination: &Path) {
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            assert_example_tree(&entry.path(), &destination);
        } else {
            assert_eq!(
                fs::read(&destination).unwrap_or_else(|error| {
                    panic!("cannot read {}: {error}", destination.display())
                }),
                fs::read(entry.path()).unwrap(),
                "{}",
                destination.display()
            );
        }
    }
}

#[test]
fn maestro_package_assets_come_from_the_package_directory() {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let (source, package, metadata, viewer) = split_inputs(&fixture);
    let destination = fixture.root.path().join("standalone");
    #[cfg(unix)]
    fs::set_permissions(
        source.join("src/modes/interactive/theme/schema.json"),
        fs::Permissions::from_mode(0o640),
    )
    .unwrap();
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
    // The copy carries the source's permission bits to the destination.
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(destination.join("theme/schema.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o640
    );
    assert!(!destination.join("docs/crate.md").exists());
    assert!(!destination.join("examples/crate.txt").exists());
    assert_example_tree(&repository_examples(), &destination.join("examples"));
}

#[test]
fn maestro_missing_package_input_keeps_completed_groups() {
    let fixture = Fixture::new();
    let (source, package, metadata, viewer) = split_inputs(&fixture);
    fs::remove_dir_all(package.join("examples")).unwrap();
    let destination = fixture.root.path().join("incomplete");
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
    // Windows reads `/` and `\` as the same separator.
    let spelled = |text: String| {
        if cfg!(windows) {
            text.replace('\\', "/")
        } else {
            text
        }
    };
    assert!(
        spelled(error.to_string()).contains(&spelled(input.display().to_string())),
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
        let destination = fixture.root.path().join("incomplete");
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
            &fixture.root.path().join("dist"),
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
        &fixture.root.path().join("standalone"),
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
    let metadata = fixture.root.path().join("absent/Cargo.toml");
    let error = tooling::copy_assets::copy(
        &source,
        &fixture.root.path().join("standalone"),
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
    let destination = fixture.root.path().join("standalone");
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
    assert_eq!(
        error.to_string(),
        format!(
            "cannot copy {}: {}",
            viewer.display(),
            fs::read_dir(&viewer).unwrap_err()
        )
    );
    assert!(destination.join("export-html").is_dir());
    assert!(!destination.join("docs").exists());
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
        &fixture.root.path().join("standalone"),
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
        &fixture.root.path().join("standalone"),
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

/// Assert that a failed copy leaves no destination more permissive than its source.
#[cfg(target_os = "linux")]
#[test]
fn maestro_failed_copy_keeps_the_source_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let (source, package, _metadata, viewer) = split_inputs(&fixture);
    // This process's memory is a private regular file whose first read fails, so the
    // copy stops after the destination exists and before a single byte is written.
    let private = Path::new("/proc/self/mem");
    assert_eq!(
        fs::metadata(private).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let destination = fixture.root.path().join("standalone");
    let error = tooling::copy_assets::copy(
        &source,
        &destination,
        tooling::copy_assets::Layout::Standalone {
            metadata: private,
            viewer: &viewer,
            package: &package,
        },
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with(&format!("cannot copy {}: ", private.display())),
        "{error}"
    );
    let mode = fs::metadata(destination.join("Cargo.toml"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "a failed copy exposed its destination");
}

/// Assert that a destination-side failure keeps its native error without an input name.
#[test]
fn maestro_destination_failure_keeps_its_native_error() {
    let fixture = Fixture::new();
    let (source, package, metadata, viewer) = split_inputs(&fixture);
    let destination = fixture.root.path().join("standalone");
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
    let destination = fixture.root.path().join("dist");
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
    let destination = fixture.root.path().join("dist");
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
    let image = source.join("src/modes/interactive/assets").join("mark.png");
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
        assert!(tooling::run(&args, &fixture.child, fixture.root.path()).is_err());
    }
    let output = Command::new(env!("CARGO_BIN_EXE_development"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stderr, b"missing development command\n");
    let blocked = fixture.root.path().join("blocked");
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
    let destination = fixture.root.path().join("dist");
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
    let package = fixture.root.path().join("package");
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
    let standalone = fixture.root.path().join("standalone");
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
    let package = fixture.root.path().join("package");
    standalone_inputs(&source, &package);
    fs::rename(package.join("docs"), package.join("real-docs")).unwrap();
    std::os::unix::fs::symlink("real-docs", package.join("docs")).unwrap();
    let destination = fixture.root.path().join("standalone");
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
        .path()
        .join("crates/maestro-models/examples/browser_import_check.rs");
    fs::create_dir_all(entry.parent().unwrap()).unwrap();
    fs::write(entry, "controlled build input").unwrap();
    let log = fixture.root.path().join("maestro-browser-smoke-errors.log");
    let artifact = fixture.root.path().join("artifact.wasm");
    for exit in ["0", "17"] {
        let output = fixture
            .invoke(
                "maestro_browser_build_records_diagnostics_without_noise",
                "check-browser-smoke",
                &[],
            )
            .temp_dir(fixture.root.path())
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

/// The committed metadata document the hook fixture selects from.
fn selection_metadata() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/selection-metadata.json")
}

/// Fixture member names in metadata order.
const MEMBERS: &str = "a, f, b, c, d, e, g, alone, nested, maestro-test-conventions-old, \
external-reader, shared, maestro-models, maestro-web";

/// Stage the supplied checkout paths in the fixture repository.
fn stage_paths(root: &Path, paths: &[&str]) {
    for path in paths {
        let file = root.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "captured").unwrap();
        git(root, &["add", "--", path]);
    }
}

/// Write the fixture metadata in the single-line form Cargo emits.
///
/// The controlled tool serves the file verbatim, so the hook's metadata reader sees a
/// document shaped like the native producer's; parsed equality proves the reformat loses
/// no field.
fn compact_metadata(path: &Path) -> PathBuf {
    let document: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(selection_metadata()).unwrap()).unwrap();
    fs::write(path, serde_json::to_string(&document).unwrap()).unwrap();
    let served: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(served, document, "compacted metadata keeps every field");
    path.to_owned()
}

/// Invoke the hook driver for `test` against the fixture metadata.
///
/// The fixture root leads `PATH`, so `just` resolves to the controlled tool while the
/// hook still finds its native Git.
fn run_hook(fixture: &Fixture, test: &str) -> TestProcess {
    run_driver(fixture, test, "hook")
}

/// Invoke the production `pre-commit` dispatcher for `test` against the fixture metadata.
fn run_pre_commit(fixture: &Fixture, test: &str) -> TestProcess {
    run_driver(fixture, test, "pre-commit")
}

/// Invoke `operation` for `test` with the fixture metadata and controlled tools on `PATH`.
fn run_driver(fixture: &Fixture, test: &str, operation: &str) -> TestProcess {
    let metadata = compact_metadata(&fixture.root.path().join("metadata.json"));
    fixture
        .invoke(test, operation, &[])
        .env("MAESTRO_METADATA", metadata)
        .env("PATH", fixture_path(fixture))
}

/// The fixture's controlled tool directory ahead of this process's `PATH`, joined with the
/// host's list separator.
fn fixture_path(fixture: &Fixture) -> std::ffi::OsString {
    let inherited = std::env::var_os("PATH").unwrap();
    let entries =
        std::iter::once(fixture.root.path().to_path_buf()).chain(std::env::split_paths(&inherited));
    std::env::join_paths(entries).unwrap()
}

/// Resolve an executable through this process's `PATH`.
#[cfg(unix)]
fn resolve(program: &str) -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|directory| directory.join(program))
        .find(|path| path.is_file())
        .unwrap_or_else(|| panic!("{program} is not on PATH"))
}

/// Commands the controlled tools ran, in order.
fn invocations(fixture: &Fixture) -> Vec<String> {
    fs::read_to_string(fixture.root.path().join("receipt/invocations"))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Assert the canary reached no recorded command environment, metadata included.
fn assert_commands_exclude_canary(fixture: &Fixture) {
    let listed = invocations(fixture);
    let recorded: Vec<_> = fs::read_dir(fixture.root.path().join("receipt/env"))
        .unwrap()
        .map(|entry| entry.unwrap())
        .collect();
    assert_eq!(
        recorded.len(),
        listed.len(),
        "every command records its environment: {listed:?}"
    );
    for entry in recorded {
        let index: usize = entry.file_name().to_string_lossy().parse().unwrap();
        let values: Vec<Vec<u8>> = fs::read_dir(entry.path())
            .unwrap()
            .map(|file| fs::read(file.unwrap().path()).unwrap())
            .collect();
        assert!(
            !values.iter().any(|value| value == b"secret-value"),
            "{} received the canary",
            listed[index]
        );
    }
    assert!(
        listed.iter().any(|line| line.starts_with("metadata|")),
        "metadata environment is recorded: {listed:?}"
    );
}

/// The check recipe invocation, or an empty string when the hook never reached it.
fn check_invocation(fixture: &Fixture) -> String {
    invocations(fixture)
        .into_iter()
        .find(|line| line.starts_with("_check|"))
        .unwrap_or_default()
}

/// How many browser smoke commands the hook ran.
fn smoke_runs(fixture: &Fixture) -> usize {
    invocations(fixture)
        .iter()
        .filter(|line| *line == "smoke" || line.starts_with("check-browser-smoke"))
        .count()
}

#[test]
fn maestro_hook_restages_only_captured_paths() {
    drive();
    assert_hook_restages_only_captured_paths("maestro_hook_restages_only_captured_paths");
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
                // The child compiles a fixture, which on Windows needs the system locations
                // its linker discovery and temporary files read.
                [
                    "RUSTUP_HOME",
                    "CARGO_HOME",
                    "RUSTUP_AUTO_INSTALL",
                    "SYSTEMDRIVE",
                    "COMSPEC",
                    "PATHEXT",
                    "PROGRAMDATA",
                    "PROGRAMFILES",
                    "PROGRAMFILES(X86)",
                    "PROGRAMW6432",
                    "COMMONPROGRAMFILES",
                    "COMMONPROGRAMFILES(X86)",
                    "COMMONPROGRAMW6432",
                    "NUMBER_OF_PROCESSORS",
                    "PROCESSOR_ARCHITECTURE",
                    "OS",
                    "USERPROFILE",
                    "APPDATA",
                    "LOCALAPPDATA",
                    "TEMP",
                    "TMP",
                ]
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
    assert_hook_restages_only_captured_paths("maestro_hook_restages_only_captured_paths");
}

/// Stage formatted and deleted paths, then assert the hook restages only the captured ones.
fn assert_hook_restages_only_captured_paths(driver: &str) {
    let fixture = Fixture::new();
    git(fixture.root.path(), &["init", "--quiet"]);
    // Windows refuses a line break in a name, so there only the other spellings are staged.
    let names: Vec<&str> = ["a space.rs", "line\nbreak.rs", "-dash.rs"]
        .into_iter()
        .filter(|name| !(cfg!(windows) && name.contains('\n')))
        .collect();
    let add = [&["add", "--"][..], &names].concat();
    for name in names.iter().chain(&["unrelated.rs"]) {
        fs::write(fixture.root.path().join(name), "old").unwrap();
    }
    git(fixture.root.path(), &add);
    stage_deleted_path(fixture.root.path());
    for name in &names {
        fs::write(fixture.root.path().join(name), "staged").unwrap();
    }
    git(fixture.root.path(), &add);
    let output = run_hook(&fixture, driver)
        .env("MAESTRO_ACTIVE_SMOKE", "true")
        .env("MAESTRO_FORMAT_ROOT", fixture.root.path())
        .env("MAESTRO_CHECK_INDEX", fixture.root.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        stdout(&output),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_commands_exclude_canary(&fixture);
    for name in &names {
        let output = git(fixture.root.path(), &["show", &format!(":{name}")]);
        assert_eq!(output.stdout, b"formatted");
    }
    assert_eq!(
        fs::read(fixture.root.path().join("unrelated.rs")).unwrap(),
        b"formatted"
    );
    assert!(
        !git(fixture.root.path(), &["diff", "--cached", "--name-only"])
            .stdout
            .windows(12)
            .any(|part| part == b"unrelated.rs")
    );
    assert_eq!(
        git(
            fixture.root.path(),
            &["diff", "--cached", "--diff-filter=D", "--name-only"]
        )
        .stdout,
        b"deleted.rs\n"
    );
}

fn assert_hook_phase(phase: &str) {
    let fixture = Fixture::new();
    git(fixture.root.path(), &["init", "--quiet"]);
    stage_paths(
        fixture.root.path(),
        &["Cargo.toml", "crates/maestro-test-conventions/src/lib.rs"],
    );
    let output = run_hook(&fixture, "maestro_hook_messages_follow_phase_results")
        .env("MAESTRO_FAIL_PHASE", phase)
        .env("MAESTRO_ACTIVE_SMOKE", "true")
        .output()
        .unwrap();
    let text = stdout(&output);
    assert!(text.contains("Running formatting, linting, and type checking...\n"));
    let selected = format!("Selected crates: {MEMBERS}\n");
    assert_eq!(
        text.contains(&selected),
        phase != "format",
        "{phase}: {text}"
    );
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
    assert_eq!(output.status.success(), phase == "success");
    assert_hook_failure_lines(phase, &text);
    assert_hook_phases_reached(&fixture, phase);
    if phase != "format" {
        assert!(
            text.find(&selected).unwrap() < text.find("✅").unwrap_or(text.len()),
            "{phase}: {text}"
        );
    }
}

/// Assert the announcement and failure line a failing phase prints.
fn assert_hook_failure_lines(phase: &str, text: &str) {
    let (announcement, failure) = match phase {
        "format" | "_check" => (
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
        _ => return,
    };
    assert!(text.ends_with(failure), "{text}");
    assert!(text.find(announcement).unwrap() < text.find(failure).unwrap());
}

/// Assert the hook stopped after the failing phase.
fn assert_hook_phases_reached(fixture: &Fixture, phase: &str) {
    let reached: &[&str] = match phase {
        "format" => &["format"],
        "_check" => &["format", "metadata", "_check"],
        "conventions" => &["format", "metadata", "_check", "conventions"],
        _ => &["format", "metadata", "_check", "conventions", "smoke"],
    };
    let ran = invocations(fixture);
    assert_eq!(ran.len(), reached.len(), "{phase}: {ran:?}");
    for (line, expected) in ran.iter().zip(reached) {
        assert!(line.starts_with(expected), "{phase}: {line}");
    }
}

#[test]
fn maestro_hook_messages_follow_phase_results() {
    drive();
    for phase in ["format", "_check", "conventions", "smoke", "success"] {
        assert_hook_phase(phase);
    }

    let fixture = Fixture::new();
    git(fixture.root.path(), &["init", "--quiet"]);
    let output = run_pre_commit(&fixture, "maestro_hook_messages_follow_phase_results")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout(&output).contains("✅ All pre-commit checks passed!\n"));
    assert_eq!(fixture.receipt("phases"), b"fmt\n_check\n");
}

#[test]
fn maestro_hook_selects_extra_steps_from_original_paths() {
    drive();
    for (name, active, expected) in [
        ("crates/maestro-models/src/lib.rs", true, "smoke\n"),
        ("crates/shared/src/lib.rs", true, "smoke\n"),
        ("Cargo.toml", true, "smoke\n"),
        ("crates/alone/src/lib.rs", true, ""),
        ("crates/maestro-models/src/lib.rs", false, ""),
        (
            "crates/maestro-test-conventions/src/lib.rs",
            true,
            "conventions\nsmoke\n",
        ),
        (
            "crates/maestro-test-conventions/tests/integration.rs",
            false,
            "conventions\n",
        ),
        ("crates/maestro-test-conventions-old/src/lib.rs", true, ""),
    ] {
        let fixture = Fixture::new();
        git(fixture.root.path(), &["init", "--quiet"]);
        let path = fixture.root.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "captured").unwrap();
        git(fixture.root.path(), &["add", "--", name]);
        fs::remove_file(&path).unwrap();
        let output = run_hook(
            &fixture,
            "maestro_hook_selects_extra_steps_from_original_paths",
        )
        .env("MAESTRO_ACTIVE_SMOKE", active.to_string())
        .output()
        .unwrap();
        assert!(output.status.success(), "{name}");
        let text = stdout(&output);
        assert_eq!(
            text.contains("Running the full conventions suite...\n"),
            expected.contains("conventions"),
            "{name}"
        );
        assert_eq!(
            smoke_runs(&fixture),
            usize::from(expected.contains("smoke")),
            "{name}"
        );
    }
}

#[cfg(unix)]
#[test]
fn consumer_workspace_cargo_state_stays_beside_the_checkout() {
    let scratch = Scratch::new().unwrap();
    let workspace = consumer_workspace(scratch.path());
    let home = scratch.path().join("cargo-home");
    assert!(
        home.join(".package-cache").is_file(),
        "lock generation must use a private Cargo home"
    );
    fs::remove_dir_all(&home).unwrap();
    let metadata = consumer_cargo(
        &workspace,
        &["metadata", "--offline", "--format-version", "1"],
    );
    assert!(metadata.status.success());
    assert!(
        home.join(".package-cache").is_file(),
        "consumer Cargo runs must use the private Cargo home"
    );
}

#[cfg(unix)]
#[test]
fn consumer_workspace_never_reaches_the_shared_cargo_home() {
    if !recipe_support::without_shared_cargo_home() {
        recipe_support::rerun_without_shared_cargo_home(
            "consumer_workspace_never_reaches_the_shared_cargo_home",
        );
        return;
    }
    let scratch = Scratch::new().unwrap();
    let workspace = consumer_workspace(scratch.path());
    let metadata = consumer_cargo(
        &workspace,
        &["metadata", "--offline", "--format-version", "1"],
    );
    assert!(metadata.status.success());
}

#[cfg(unix)]
#[test]
fn maestro_hook_checks_only_selected_packages() {
    drive();
    let scratch = Scratch::new().unwrap();
    let workspace = consumer_workspace(scratch.path());
    let leaf = workspace.join("leaf/src/lib.rs");
    fs::write(&leaf, LEAF_SOURCE.replace("    7", "    8")).unwrap();
    git(&workspace, &["add", "--", "leaf/src/lib.rs"]);
    let home = scratch.path().join("cargo-home");
    // The hook's own Cargo runs must recreate the private home the workspace setup left.
    let _ = fs::remove_dir_all(&home);
    let output = run_consumer_hook("maestro_hook_checks_only_selected_packages", &workspace);
    assert!(
        output.status.success(),
        "{}\n{}",
        stdout(&output),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        home.join(".package-cache").is_file(),
        "the hook's Cargo runs must use the private Cargo home"
    );
    let text = stdout(&output);
    assert!(text.contains("Selected crates: leaf, caller\n"), "{text}");
    assert!(text.ends_with("✅ All pre-commit checks passed!\n"));
    let log = fs::read_to_string(workspace.join("cargo-log")).unwrap();
    let commands: Vec<&str> = log.lines().collect();
    let ran = |prefix: &str| {
        commands
            .iter()
            .filter(|line| line.starts_with(prefix))
            .count()
    };
    assert_eq!(ran("fmt "), 1, "{log}");
    assert_eq!(ran("hakari "), 1, "{log}");
    assert_eq!(
        commands
            .iter()
            .find(|line| line.starts_with("clippy "))
            .copied(),
        Some("clippy -p leaf -p caller --all-targets --locked -- -D warnings"),
        "{log}"
    );
    assert_eq!(
        commands
            .iter()
            .find(|line| line.starts_with("doc "))
            .copied(),
        Some("doc -p leaf -p caller --no-deps --locked"),
        "{log}"
    );
    assert_eq!(
        commands
            .iter()
            .filter(|line| line.starts_with("test "))
            .copied()
            .collect::<Vec<_>>(),
        [
            "test -p maestro-test-conventions --locked --test integration -- --exact architecture::real_workspace_obeys_crate_rules dependency_locks::maestro_locked_resolution_keeps_checkout_clean"
        ],
        "{log}"
    );
    assert!(!log.contains("--workspace"), "{log}");
}

#[test]
fn maestro_hook_workspace_inputs_keep_the_full_gate() {
    drive();
    for path in [
        "Cargo.toml",
        "Cargo.lock",
        ".cargo/config.toml",
        "rust-toolchain.toml",
        "justfile",
        "clippy.toml",
        ".pre-commit-config.yaml",
        "docs/development.md",
        "crates/maestro-test-conventions/src/lib.rs",
    ] {
        let fixture = Fixture::new();
        git(fixture.root.path(), &["init", "--quiet"]);
        stage_paths(fixture.root.path(), &[path]);
        let output = run_hook(&fixture, "maestro_hook_workspace_inputs_keep_the_full_gate")
            .env("MAESTRO_ACTIVE_SMOKE", "true")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{path}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            stdout(&output).contains(&format!("Selected crates: {MEMBERS}\n")),
            "{path}"
        );
        assert_eq!(check_invocation(&fixture), "_check|--workspace", "{path}");
        assert_eq!(smoke_runs(&fixture), 1, "{path}");
        let conventions = path.starts_with("crates/maestro-test-conventions/");
        assert_eq!(
            invocations(&fixture)
                .iter()
                .any(|line| line == "conventions"),
            conventions,
            "{path}"
        );
    }

    let fixture = Fixture::new();
    git(fixture.root.path(), &["init", "--quiet"]);
    stage_paths(fixture.root.path(), &["Cargo.toml", "crates/a/src/lib.rs"]);
    let output = run_hook(&fixture, "maestro_hook_workspace_inputs_keep_the_full_gate")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(check_invocation(&fixture), "_check|--workspace");
}

#[test]
fn maestro_hook_renames_check_both_previous_and_new_owners() {
    drive();
    let fixture = Fixture::new();
    git(fixture.root.path(), &["init", "--quiet"]);
    stage_paths(fixture.root.path(), &["crates/b/src/moved.rs"]);
    git(
        fixture.root.path(),
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
    fs::create_dir_all(fixture.root.path().join("crates/alone/src")).unwrap();
    git(
        fixture.root.path(),
        &["mv", "crates/b/src/moved.rs", "crates/alone/src/moved.rs"],
    );
    let output = run_pre_commit(
        &fixture,
        "maestro_hook_renames_check_both_previous_and_new_owners",
    )
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // `b` reaches only `b` and `d`, `alone` only itself, so each owner adds its own members.
    assert_eq!(check_invocation(&fixture), "_check|-p|b|-p|d|-p|alone");
    assert!(!fixture.root.path().join("crates/b/src/moved.rs").exists());
    let listed = String::from_utf8(git(fixture.root.path(), &["ls-files"]).stdout).unwrap();
    assert!(!listed.contains("crates/b/src/moved.rs"), "{listed}");
    assert!(listed.contains("crates/alone/src/moved.rs"), "{listed}");
}

#[test]
fn maestro_hook_reports_metadata_errors_before_running_checks() {
    drive();
    for (case, environment) in [
        ("garbage", vec![("MAESTRO_METADATA_MODE", "garbage")]),
        ("status", vec![("MAESTRO_METADATA_EXIT", "17")]),
        ("launch", vec![("MAESTRO_MISSING_METADATA_TOOL", "1")]),
    ] {
        let fixture = Fixture::new();
        git(fixture.root.path(), &["init", "--quiet"]);
        stage_paths(fixture.root.path(), &["crates/a/src/lib.rs"]);
        let mut command = run_hook(
            &fixture,
            "maestro_hook_reports_metadata_errors_before_running_checks",
        );
        for (key, value) in environment {
            command = command.env(key, value);
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(1), "{case}");
        assert!(
            stdout(&output)
                .ends_with("❌ Checks failed. Please fix the errors before committing.\n"),
            "{case}: {}",
            stdout(&output)
        );
        assert!(!stdout(&output).contains("All pre-commit checks passed"));
        assert!(!output.stderr.is_empty(), "{case}");
        let ran = invocations(&fixture);
        assert!(
            !ran.iter().any(|line| line.starts_with("_check|")
                || line == "conventions"
                || line == "smoke"),
            "{case}: {ran:?}"
        );
    }

    let fixture = Fixture::new();
    git(fixture.root.path(), &["init", "--quiet"]);
    stage_paths(fixture.root.path(), &["crates/a/src/lib.rs"]);
    let output = run_hook(
        &fixture,
        "maestro_hook_reports_metadata_errors_before_running_checks",
    )
    .env("MAESTRO_FAIL_PHASE", "format")
    .output()
    .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(invocations(&fixture), ["format"]);
}

#[cfg(unix)]
#[test]
fn maestro_hook_missing_check_tool_reports_failure() {
    use std::os::unix::fs::symlink;

    drive();
    let fixture = Fixture::new();
    git(fixture.root.path(), &["init", "--quiet"]);
    stage_paths(fixture.root.path(), &["crates/a/src/lib.rs"]);
    let tools = fixture.root.path().join("tools");
    fs::create_dir_all(&tools).unwrap();
    symlink(resolve("git"), tools.join("git")).unwrap();
    let output = run_hook(&fixture, "maestro_hook_missing_check_tool_reports_failure")
        .env("PATH", &tools)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let text = stdout(&output);
    assert!(
        text.ends_with("❌ Checks failed. Please fix the errors before committing.\n"),
        "{text}"
    );
    assert!(!text.contains("All pre-commit checks passed"));
    let unavailable = Command::new("maestro-missing-check-tool-probe")
        .output()
        .unwrap_err()
        .to_string();
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(&unavailable),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Fail one recipe phase and assert the later phases never run, after the formatted
/// source was re-staged.
#[cfg(unix)]
fn assert_recipe_phase_failure(phase: &str, expected: &[&str]) {
    let scratch = Scratch::new().unwrap();
    let workspace = consumer_workspace(scratch.path());
    let formatted = LEAF_SOURCE.replace("    7", "    8");
    fs::write(
        workspace.join("leaf/src/lib.rs"),
        formatted.replace("    8", "8"),
    )
    .unwrap();
    git(&workspace, &["add", "--", "leaf/src/lib.rs"]);
    let output =
        consumer_hook_command("maestro_hook_scoped_failures_stop_later_phases", &workspace)
            .env("MAESTRO_FAIL_PHASE", phase)
            .output()
            .unwrap();
    assert_eq!(output.status.code(), Some(1), "{phase}");
    let text = stdout(&output);
    assert!(
        text.ends_with("❌ Checks failed. Please fix the errors before committing.\n"),
        "{phase}: {text}"
    );
    assert!(!text.contains("All pre-commit checks passed"), "{phase}");
    assert!(
        !text.contains("Running the full conventions suite...\n"),
        "{phase}: {text}"
    );
    let log = fs::read_to_string(workspace.join("cargo-log")).unwrap();
    let ran: Vec<&str> = log
        .lines()
        .map(|line| line.split(' ').next().unwrap_or_default())
        .filter(|name| *name != "metadata")
        .collect();
    assert_eq!(ran, expected, "{phase}: {log}");
    assert_eq!(
        git(&workspace, &["show", ":leaf/src/lib.rs"]).stdout,
        formatted.as_bytes(),
        "{phase}"
    );
}

#[test]
fn maestro_hook_scoped_failures_stop_later_phases() {
    drive();
    for (phase, reached) in [("format", 1usize), ("conventions", 4), ("smoke", 5)] {
        let fixture = Fixture::new();
        git(fixture.root.path(), &["init", "--quiet"]);
        stage_paths(
            fixture.root.path(),
            &[
                "Cargo.toml",
                "a space.rs",
                "crates/maestro-test-conventions/src/lib.rs",
            ],
        );
        let output = run_hook(&fixture, "maestro_hook_scoped_failures_stop_later_phases")
            .env("MAESTRO_FAIL_PHASE", phase)
            .env("MAESTRO_ACTIVE_SMOKE", "true")
            .env("MAESTRO_FORMAT_ROOT", fixture.root.path())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{phase}");
        assert!(
            !stdout(&output).contains("All pre-commit checks passed"),
            "{phase}"
        );
        assert_eq!(invocations(&fixture).len(), reached, "{phase}");
    }

    #[cfg(unix)]
    for (phase, expected) in [
        ("hakari", &["fmt", "hakari"][..]),
        ("clippy", &["fmt", "hakari", "clippy"][..]),
        ("doc", &["fmt", "hakari", "clippy", "doc"][..]),
        ("test", &["fmt", "hakari", "clippy", "doc", "test"][..]),
    ] {
        assert_recipe_phase_failure(phase, expected);
    }

    let fixture = Fixture::new();
    let output = run_hook(&fixture, "maestro_hook_scoped_failures_stop_later_phases")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!output.stderr.is_empty());
    assert!(invocations(&fixture).is_empty());

    let fixture = Fixture::new();
    git(fixture.root.path(), &["init", "--quiet"]);
    stage_paths(fixture.root.path(), &["crates/a/src/lib.rs"]);
    fs::write(fixture.root.path().join(".git/index.lock"), "").unwrap();
    let output = run_hook(&fixture, "maestro_hook_scoped_failures_stop_later_phases")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(invocations(&fixture), ["format"]);
}

#[test]
fn maestro_hook_browser_smoke_runs_once_for_reached_inputs() {
    drive();
    for (paths, active, expected) in [
        (vec!["crates/maestro-models/src/lib.rs"], true, 1usize),
        (vec!["crates/shared/src/lib.rs"], true, 1),
        (vec!["Cargo.toml"], true, 1),
        (vec!["crates/alone/src/lib.rs"], true, 0),
        (vec!["crates/maestro-models/src/lib.rs"], false, 0),
    ] {
        let fixture = Fixture::new();
        git(fixture.root.path(), &["init", "--quiet"]);
        stage_paths(fixture.root.path(), &paths);
        let output = run_hook(
            &fixture,
            "maestro_hook_browser_smoke_runs_once_for_reached_inputs",
        )
        .env("MAESTRO_ACTIVE_SMOKE", active.to_string())
        .output()
        .unwrap();
        assert!(output.status.success(), "{paths:?}");
        assert_eq!(smoke_runs(&fixture), expected, "{paths:?}");
    }

    assert_dispatcher_routes_delivered_smoke();

    let fixture = Fixture::new();
    git(fixture.root.path(), &["init", "--quiet"]);
    stage_paths(fixture.root.path(), &["crates/maestro-models/src/lib.rs"]);
    let output = run_hook(
        &fixture,
        "maestro_hook_browser_smoke_runs_once_for_reached_inputs",
    )
    .env("MAESTRO_ACTIVE_SMOKE", "true")
    .env("MAESTRO_FAIL_PHASE", "smoke")
    .output()
    .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).ends_with("❌ Browser smoke check failed.\n"));
    assert_eq!(smoke_runs(&fixture), 1);
}

/// Assert the dispatcher schedules smoke only for a delivered browser entry.
fn assert_dispatcher_routes_delivered_smoke() {
    let entry = "crates/maestro-models/examples/browser_import_check.rs";
    for delivered in [false, true] {
        let fixture = Fixture::new();
        git(fixture.root.path(), &["init", "--quiet"]);
        stage_paths(fixture.root.path(), &["crates/maestro-models/src/lib.rs"]);
        if delivered {
            stage_paths(fixture.root.path(), &[entry]);
        }
        let output = run_pre_commit(
            &fixture,
            "maestro_hook_browser_smoke_runs_once_for_reached_inputs",
        )
        .output()
        .unwrap();
        assert!(output.status.success(), "{delivered}");
        assert_eq!(
            stdout(&output).contains("Running browser smoke check...\n"),
            delivered,
            "the dispatcher routes smoke only for a delivered entry"
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
        .path()
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
        .temp_dir(fixture.root.path())
        .output()
        .unwrap();
    let log = fixture.root.path().join("maestro-browser-smoke-errors.log");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        output.stderr,
        format!("Browser smoke check failed. See {}\n", log.display()).as_bytes()
    );
    let logged = fs::read_to_string(&log).unwrap();
    let expected = missing_program_message(&fixture.child);
    assert!(
        logged.contains(&expected),
        "log {logged:?} does not hold {expected:?}"
    );
}

#[test]
fn maestro_hook_missing_tool_announces_failed_step() {
    drive();
    for (phase, message) in [
        (
            "format",
            "❌ Checks failed. Please fix the errors before committing.\n",
        ),
        ("conventions", "❌ Conventions suite failed.\n"),
        ("smoke", "❌ Browser smoke check failed.\n"),
    ] {
        let fixture = Fixture::new();
        git(fixture.root.path(), &["init", "--quiet"]);
        stage_paths(
            fixture.root.path(),
            &["Cargo.toml", "crates/maestro-test-conventions/src/lib.rs"],
        );
        let output = run_hook(&fixture, "maestro_hook_missing_tool_announces_failed_step")
            .env("MAESTRO_MISSING_PHASE", phase)
            .env("MAESTRO_ACTIVE_SMOKE", "true")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{phase}");
        assert!(stdout(&output).ends_with(message), "{}", stdout(&output));
        assert!(!stdout(&output).contains("All pre-commit checks passed"));
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains(&missing_program_message("missing-tool")),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Create a disposable native workspace whose caller compiles the leaf crate.
#[cfg(unix)]
fn consumer_workspace(root: &Path) -> PathBuf {
    let workspace = root.join("checkout");
    write_consumer_tool(&workspace);
    write_consumer_crates(&workspace);
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../justfile"),
        workspace.join("justfile"),
    )
    .unwrap();
    let lock = consumer_toolchain(&workspace)
        .args(["generate-lockfile", "--offline"])
        .output()
        .unwrap();
    assert!(
        lock.status.success(),
        "{}",
        String::from_utf8_lossy(&lock.stderr)
    );
    commit_fixture(&workspace);
    workspace
}

/// Write the Cargo shim that records calls and forwards to the real compiler.
#[cfg(unix)]
fn write_consumer_tool(workspace: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::create_dir_all(workspace.join("bin")).unwrap();
    let cargo = env!("CARGO");
    fs::write(
        workspace.join("bin/cargo"),
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$(dirname \"$0\")/../cargo-log\"\nif [ -n \"${{MAESTRO_FAIL_PHASE:-}}\" ] && [ \"$1\" = \"$MAESTRO_FAIL_PHASE\" ]; then exit 17; fi\ncase \"$1\" in\n  hakari|test) exit 0 ;;\nesac\nexec '{cargo}' \"$@\"\n"
        ),
    )
    .unwrap();
    fs::set_permissions(
        workspace.join("bin/cargo"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
}

/// Write the leaf and caller crates of the consumer workspace.
#[cfg(unix)]
fn write_consumer_crates(workspace: &Path) {
    fs::write(
        workspace.join("Cargo.toml"),
        "[workspace]\nmembers = [\"leaf\", \"caller\"]\nresolver = \"3\"\n",
    )
    .unwrap();
    for (name, dependency) in [("leaf", ""), ("caller", "leaf = { path = \"../leaf\" }\n")] {
        fs::create_dir_all(workspace.join(name).join("src")).unwrap();
        fs::write(
            workspace.join(name).join("Cargo.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[dependencies]\n{dependency}"
            ),
        )
        .unwrap();
    }
    fs::write(workspace.join("leaf/src/lib.rs"), LEAF_SOURCE).unwrap();
    fs::write(workspace.join("caller/src/lib.rs"), CALLER_SOURCE).unwrap();
}

/// Commit the fixture inputs so only a test's own edit is staged later.
#[cfg(unix)]
fn commit_fixture(workspace: &Path) {
    git(workspace, &["init", "--quiet"]);
    git(
        workspace,
        &[
            "add",
            "--",
            "Cargo.toml",
            "Cargo.lock",
            "justfile",
            "leaf",
            "caller",
        ],
    );
    git(
        workspace,
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
}

/// Documented leaf crate the consumer workspace starts from.
#[cfg(unix)]
const LEAF_SOURCE: &str =
    "//! Leaf crate.\n\n/// Returns the fixture value.\npub fn value() -> u32 {\n    7\n}\n";

/// Initial caller source in the consumer workspace.
#[cfg(unix)]
const CALLER_SOURCE: &str = "//! Caller crate.\n\n/// Doubles the leaf value.\npub fn doubled() -> u32 {\n    leaf::value() * 2\n}\n";

/// Run the hook against a native workspace with its recorded tool shim.
#[cfg(unix)]
fn run_consumer_hook(test: &str, workspace: &Path) -> Output {
    consumer_hook_command(test, workspace).output().unwrap()
}

/// Prepare the production `pre-commit` dispatcher for a native workspace with its
/// recorded tool shim.
#[cfg(unix)]
fn consumer_hook_command(test: &str, workspace: &Path) -> TestProcess {
    let mut command = TestProcess::named(format!("tooling_commands::{test}"))
        .unwrap()
        .nocapture()
        .current_dir(workspace)
        .env("MAESTRO_DRIVER", "pre-commit")
        .env("MAESTRO_FIXTURE_ROOT", workspace)
        .env("MAESTRO_FIXTURE_CARGO", workspace.join("bin/cargo"))
        .env("MAESTRO_RECEIPT", workspace.join("receipt"))
        .env("CARGO_TARGET_DIR", workspace.join("target"))
        .env(
            "CARGO_HOME",
            toolchain_support::home(workspace.parent().unwrap()),
        )
        .env(
            "PATH",
            format!(
                "{}:{}",
                workspace.join("bin").display(),
                std::env::var("PATH").unwrap()
            ),
        );
    if let Some(channel) = toolchain_support::channel() {
        command = command.env("RUSTUP_TOOLCHAIN", channel);
    }
    command
}

/// Start a native toolchain command in the consumer workspace, with Cargo state kept beside the
/// checkout and outside its repository.
#[cfg(unix)]
fn consumer_toolchain(workspace: &Path) -> Command {
    let mut command =
        toolchain_support::fixture_command("cargo", workspace.parent().unwrap(), None).unwrap();
    command.current_dir(workspace);
    command
}

/// Run one native Cargo command inside the consumer workspace.
#[cfg(unix)]
fn consumer_cargo(workspace: &Path, args: &[&str]) -> Output {
    consumer_toolchain(workspace)
        .env("CARGO_TARGET_DIR", workspace.join("target"))
        .args(args)
        .output()
        .unwrap()
}

#[cfg(unix)]
#[test]
fn maestro_hook_reverse_dependent_lint_is_not_skipped() {
    drive();
    let scratch = Scratch::new().unwrap();
    let workspace = consumer_workspace(scratch.path());
    let leaf = workspace.join("leaf/src/lib.rs");
    fs::write(
        &leaf,
        "//! Leaf crate.\n\n/// Returns the fixture value.\n#[deprecated(note = \"use the next value\")]\npub fn value() -> u32 {\n    7\n}\n",
    )
    .unwrap();
    git(&workspace, &["add", "--", "leaf/src/lib.rs"]);
    let control = consumer_cargo(
        &workspace,
        &[
            "clippy",
            "-p",
            "leaf",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings",
        ],
    );
    assert!(
        control.status.success(),
        "leaf-only control: {}",
        String::from_utf8_lossy(&control.stderr)
    );
    let output = run_consumer_hook(
        "maestro_hook_reverse_dependent_lint_is_not_skipped",
        &workspace,
    );
    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    let text = format!(
        "{}{}",
        stdout(&output),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains("caller/src/lib.rs"), "{text}");
    assert!(text.contains("deprecated"), "{text}");
    assert_reverse_dependent_arguments(&workspace);

    fs::write(&leaf, LEAF_SOURCE).unwrap();
    git(&workspace, &["add", "--", "leaf/src/lib.rs"]);
    let output = run_consumer_hook(
        "maestro_hook_reverse_dependent_lint_is_not_skipped",
        &workspace,
    );
    assert!(
        output.status.success(),
        "{}\n{}",
        stdout(&output),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Assert the hook's Clippy invocation names the leaf and its caller.
#[cfg(unix)]
fn assert_reverse_dependent_arguments(workspace: &Path) {
    let log = fs::read_to_string(workspace.join("cargo-log")).unwrap();
    let clippy = log
        .lines()
        .rfind(|line| line.starts_with("clippy "))
        .unwrap_or_default();
    assert!(
        clippy.contains("-p leaf") && clippy.contains("-p caller"),
        "{log}"
    );
}

#[cfg(unix)]
#[test]
fn maestro_hook_scoped_docs_still_deny_missing_documentation() {
    drive();
    let scratch = Scratch::new().unwrap();
    let workspace = consumer_workspace(scratch.path());
    let caller = workspace.join("caller/src/lib.rs");
    fs::write(
        &caller,
        "pub fn doubled() -> u32 {\n    leaf::value() * 2\n}\n",
    )
    .unwrap();
    git(&workspace, &["add", "--", "caller/src/lib.rs"]);
    let control = consumer_toolchain(&workspace)
        .env("CARGO_TARGET_DIR", workspace.join("target"))
        .env("RUSTDOCFLAGS", "-D warnings -D missing_docs")
        .args(["doc", "-p", "leaf", "--no-deps", "--locked"])
        .output()
        .unwrap();
    assert!(
        control.status.success(),
        "leaf-only control: {}",
        String::from_utf8_lossy(&control.stderr)
    );
    let output = run_consumer_hook(
        "maestro_hook_scoped_docs_still_deny_missing_documentation",
        &workspace,
    );
    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    let text = format!(
        "{}{}",
        stdout(&output),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains("missing documentation"), "{text}");
    assert!(text.contains("caller"), "{text}");
    let log = fs::read_to_string(workspace.join("cargo-log")).unwrap();
    let doc = log
        .lines()
        .find(|line| line.starts_with("doc "))
        .unwrap_or_default();
    assert!(doc.contains("-p caller"), "{log}");

    fs::write(&caller, CALLER_SOURCE).unwrap();
    git(&workspace, &["add", "--", "caller/src/lib.rs"]);
    let output = run_consumer_hook(
        "maestro_hook_scoped_docs_still_deny_missing_documentation",
        &workspace,
    );
    assert!(
        output.status.success(),
        "{}\n{}",
        stdout(&output),
        String::from_utf8_lossy(&output.stderr)
    );
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
        "crates/maestro-tooling/src/example_files.rs",
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

/// Independent source files supply their own index rows without a second inventory.
#[test]
fn example_index_discovers_independent_additions() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let folder = root.path().join("examples/extensions");
    example_checkout(root.path());
    fs::create_dir_all(folder.join("nested")).unwrap();
    let readme = folder.join("README.md");
    fs::write(
        folder.join("zebra.rs"),
        "//! Zebra | tool.\nfn factory() {}\n",
    )
    .unwrap();
    fs::write(
        folder.join("alpha.rs"),
        "//! Alpha command\n//! with a continuation.\n\nfn factory() {}\n",
    )
    .unwrap();
    fs::write(folder.join("nested/helper.rs"), "//! A helper.\n").unwrap();
    fs::write(folder.join("nested.md"), "# Nested usage\n").unwrap();
    fs::write(folder.join("alpha.md"), "# Alpha usage\n\nDetails.\n").unwrap();
    let cargo = Path::new("unused-cargo");
    assert_eq!(
        tooling::run(&["example-index".into()], cargo, root.path()).unwrap(),
        std::process::ExitCode::SUCCESS
    );
    let rendered = fs::read_to_string(&readme).unwrap();
    assert_eq!(
        rendered,
        "Before\n<!-- example-index:start -->\n| File | Provides |\n| --- | --- |\n| `alpha.md` | Alpha usage |\n| `alpha.rs` | Alpha command with a continuation. |\n| `nested.md` | Nested usage |\n| `nested/helper.rs` | A helper. |\n| `zebra.rs` | Zebra \\| tool. |\n<!-- example-index:end -->\nAfter\n"
    );
    assert!(
        tooling::run(
            &["example-index".into(), "--check".into()],
            cargo,
            root.path()
        )
        .is_ok()
    );
    fs::write(folder.join("beta.rs"), "//! Beta command.\n").unwrap();
    assert!(
        tooling::run(
            &["example-index".into(), "--check".into()],
            cargo,
            root.path()
        )
        .is_err()
    );
    assert_eq!(fs::read_to_string(&readme).unwrap(), rendered);
}

/// Write a checkout with stale generated regions and an untouched preparation program.
fn example_checkout(root: &Path) {
    let folder = root.join("examples/extensions");
    let local = root.join("crates/maestro-extensions-wasm/examples");
    fs::create_dir_all(&folder).unwrap();
    fs::create_dir_all(&local).unwrap();
    fs::write(
        folder.join("README.md"),
        "Before\n<!-- example-index:start -->\nold\n<!-- example-index:end -->\nAfter\n",
    )
    .unwrap();
    fs::write(
        root.join("crates/maestro-extensions-wasm/Cargo.toml"),
        "[package]\n# example-index:start\nold\n# example-index:end\n\n[dependencies]\n",
    )
    .unwrap();
    fs::write(
        local.join("prepare_author_component.rs"),
        "//! Prepare.\n// example-index:start\nold\n// example-index:end\nfn main() {}\n",
    )
    .unwrap();
}

/// Run the generator, in check mode when `check` is set.
fn example_index(root: &Path, check: bool) -> std::io::Result<std::process::ExitCode> {
    let mut args = vec!["example-index".into()];
    if check {
        args.push("--check".into());
    }
    tooling::run(&args, Path::new("unused-cargo"), root)
}

/// Example sources supply manifest targets without rewriting runtime preparation.
#[test]
fn example_index_renders_targets() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let folder = root.path().join("examples/extensions");
    let local = root.path().join("crates/maestro-extensions-wasm/examples");
    example_checkout(root.path());
    fs::create_dir_all(folder.join("with-deps")).unwrap();
    fs::create_dir_all(folder.join("zebra")).unwrap();
    let marker = "// maestro-component:";
    fs::write(
        folder.join("zebra.rs"),
        format!("//! Zebra.\n{marker} MAESTRO_ZEBRA_COMPONENT\n"),
    )
    .unwrap();
    fs::write(folder.join("zebra/view.rs"), "//! Zebra view.\n").unwrap();
    fs::write(folder.join("alpha.rs"), "//! Alpha.\n").unwrap();
    fs::write(folder.join("alpha.md"), "# Alpha usage\n").unwrap();
    fs::write(
        folder.join("with-deps/main.rs"),
        format!("//! Dependencies.\n{marker} MAESTRO_DEPS_COMPONENT\n"),
    )
    .unwrap();
    fs::write(
        local.join("bridge.rs"),
        format!("//! Bridge.\n{marker} MAESTRO_CONTEXT_COMPONENT\nfn f() {{}}\n"),
    )
    .unwrap();
    fs::write(local.join("plain.rs"), "//! Plain.\n").unwrap();
    example_index(root.path(), false).unwrap();
    let manifest = root
        .path()
        .join("crates/maestro-extensions-wasm/Cargo.toml");
    let prepare = local.join("prepare_author_component.rs");
    let rendered = (
        fs::read_to_string(&manifest).unwrap(),
        fs::read_to_string(&prepare).unwrap(),
    );
    assert_eq!(
        rendered.0,
        "[package]\n# example-index:start\n[[example]]\nname = \"alpha\"\npath = \"../../examples/extensions/alpha.rs\"\ncrate-type = [\"cdylib\"]\n\n[[example]]\nname = \"bridge\"\ncrate-type = [\"cdylib\"]\n\n[[example]]\nname = \"plain\"\ncrate-type = [\"cdylib\"]\n\n[[example]]\nname = \"with_deps\"\npath = \"../../examples/extensions/with-deps/main.rs\"\ncrate-type = [\"cdylib\"]\n\n[[example]]\nname = \"zebra\"\npath = \"../../examples/extensions/zebra.rs\"\ncrate-type = [\"cdylib\"]\n# example-index:end\n\n[dependencies]\n"
    );
    assert_eq!(
        rendered.1,
        "//! Prepare.\n// example-index:start\nold\n// example-index:end\nfn main() {}\n"
    );
    example_index(root.path(), true).unwrap();
    fs::write(local.join("plain.rs"), "//! Plain.\n").unwrap();
    fs::write(local.join("added.rs"), "//! Added.\n").unwrap();
    assert!(example_index(root.path(), true).is_err());
    assert_eq!(
        (
            fs::read_to_string(&manifest).unwrap(),
            fs::read_to_string(&prepare).unwrap()
        ),
        rendered
    );
}

/// The tracked repository index is exactly the table rendered from its files.
#[test]
fn shipped_example_index_matches_documentation() {
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    tooling::run(
        &["example-index".into(), "--check".into()],
        Path::new("unused-cargo"),
        &checkout,
    )
    .unwrap();
}

/// Two new example test files enter the same generated module inventory independently.
#[test]
fn example_test_modules_discover_independent_additions() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    for name in ["zebra_example.rs", "alpha_example.rs", "helper.rs"] {
        fs::write(root.path().join(name), "// test fixture\n").unwrap();
    }
    let declarations = crate::example_test_modules::generate(root.path()).unwrap();
    assert!(declarations.contains("mod alpha_example;"));
    assert!(declarations.contains("mod zebra_example;"));
    assert!(!declarations.contains("mod helper;"));
    assert!(
        declarations.find("alpha_example").unwrap() < declarations.find("zebra_example").unwrap()
    );
    assert!(!declarations.contains("#[path"));
}

/// Discovered modules retain ordinary nested-module lookup when compiled and run.
#[test]
fn example_test_modules_resolve_nested_modules() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let tests = root.path().join("tests");
    fs::create_dir_all(tests.join("nested_example")).unwrap();
    fs::create_dir(root.path().join("generated")).unwrap();
    fs::write(
        tests.join("nested_example.rs"),
        "mod child; pub(super) fn value() -> u8 { child::value() }",
    )
    .unwrap();
    fs::write(
        tests.join("nested_example/child.rs"),
        "pub(super) fn value() -> u8 { 42 }",
    )
    .unwrap();
    let generated = root.path().join("generated/modules.rs");
    fs::write(
        &generated,
        crate::example_test_modules::generate(&tests).unwrap(),
    )
    .unwrap();
    fs::write(
        root.path().join("tests.rs"),
        format!(
            "include!({generated:?}); example_tests!(); pub fn value() -> u8 {{ nested_example::value() }}"
        ),
    )
    .unwrap();
    let main = root.path().join("main.rs");
    fs::write(
        &main,
        "mod tests; fn main() { assert_eq!(tests::value(), 42); }",
    )
    .unwrap();
    let executable = root
        .path()
        .join(format!("nested{}", std::env::consts::EXE_SUFFIX));
    let output = toolchain_support::command("rustc")
        .arg(&main)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(Command::new(executable).status().unwrap().success());
}

/// A source-only addition supplies its build target, environment and setup selection.
#[test]
fn example_components_discover_source_only_additions() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    fs::create_dir(root.path().join("helpers")).unwrap();
    fs::write(root.path().join("helpers/unmarked.rs"), "//! Helper.\n").unwrap();
    let prepare = || {
        let mut builds = Vec::new();
        let entries =
            tooling::example_files::prepare(&[root.path().to_path_buf()], |name, environment| {
                builds.push((name.to_owned(), environment.to_owned()));
                Ok(PathBuf::from(format!("/components/{name}.wasm")))
            })
            .unwrap();
        (builds, entries)
    };
    assert_eq!(prepare(), (Vec::new(), String::new()));
    fs::write(
        root.path().join("fixture_example.rs"),
        "//! Fixture.\n// maestro-component: MAESTRO_FIXTURE_COMPONENT\n",
    )
    .unwrap();
    assert_eq!(
        prepare(),
        (
            vec![(
                "fixture_example".to_owned(),
                "MAESTRO_FIXTURE_COMPONENT".to_owned()
            )],
            "MAESTRO_FIXTURE_COMPONENT=/components/fixture_example.wasm\n".to_owned()
        )
    );
    let config: toml::Value =
        toml::from_str(include_str!("../../../.config/nextest.toml")).unwrap();
    let filter = config["profile"]["default"]["scripts"][0]["filter"]
        .as_str()
        .unwrap();
    assert!(filter.contains("test(/_example::/)"), "{filter}");
    assert!(!filter.contains("qna_example::"), "{filter}");
}

/// Invalid environment markers never reach the component builder.
#[test]
fn example_components_reject_invalid_markers() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    for variable in [
        "OTHER_FIXTURE_COMPONENT",
        "MAESTRO_FIXTURE",
        "MAESTRO_fixture_COMPONENT",
        "MAESTRO_FIXTURE=VALUE_COMPONENT",
        "MAESTRO_FIXTURE COMPONENT",
    ] {
        fs::write(
            root.path().join("fixture.rs"),
            format!("// maestro-component: {variable}\n"),
        )
        .unwrap();
        let mut builds = Vec::new();
        let error =
            tooling::example_files::prepare(&[root.path().to_path_buf()], |name, environment| {
                builds.push((name.to_owned(), environment.to_owned()));
                Ok(PathBuf::from("/components/fixture.wasm"))
            })
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("invalid component variable: {variable}")
        );
        assert!(builds.is_empty(), "{builds:?}");
    }
}

/// A repeated variable is rejected before its second build, even across files.
#[test]
fn example_components_reject_duplicate_markers() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    for name in ["first", "second"] {
        fs::write(
            root.path().join(format!("{name}.rs")),
            "// maestro-component: MAESTRO_FIXTURE_COMPONENT\n",
        )
        .unwrap();
    }
    let mut builds = Vec::new();
    let error =
        tooling::example_files::prepare(&[root.path().to_path_buf()], |name, environment| {
            builds.push((name.to_owned(), environment.to_owned()));
            Ok(PathBuf::from(format!("/components/{name}.wasm")))
        })
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "duplicate component variable: MAESTRO_FIXTURE_COMPONENT"
    );
    assert_eq!(
        builds,
        [("first".to_owned(), "MAESTRO_FIXTURE_COMPONENT".to_owned())]
    );
}

/// Builder failures retain their error and stop preparation before later targets.
#[test]
fn example_components_propagate_builder_failure() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    for (name, variable) in [
        ("first", "MAESTRO_FIRST_COMPONENT"),
        ("second", "MAESTRO_SECOND_COMPONENT"),
        ("third", "MAESTRO_THIRD_COMPONENT"),
    ] {
        fs::write(
            root.path().join(format!("{name}.rs")),
            format!("// maestro-component: {variable}\n"),
        )
        .unwrap();
    }
    let mut builds = Vec::new();
    let error =
        tooling::example_files::prepare(&[root.path().to_path_buf()], |name, environment| {
            builds.push((name.to_owned(), environment.to_owned()));
            if name == "second" {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "fixture build refused",
                ))
            } else {
                Ok(PathBuf::from(format!("/components/{name}.wasm")))
            }
        })
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(error.to_string(), "fixture build refused");
    assert_eq!(
        builds,
        [
            ("first".to_owned(), "MAESTRO_FIRST_COMPONENT".to_owned()),
            ("second".to_owned(), "MAESTRO_SECOND_COMPONENT".to_owned()),
        ]
    );
}

/// Fixed exports cannot be replaced by a source marker or reach the builder.
#[test]
fn example_components_reject_reserved_variables() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    for variable in ["MAESTRO_AUTHOR_COMPONENT", "MAESTRO_CONTEXT_COMPONENT"] {
        fs::write(
            root.path().join("fixture.rs"),
            format!("// maestro-component: {variable}\n"),
        )
        .unwrap();
        let error = tooling::example_files::prepare(&[root.path().to_path_buf()], |_, _| {
            panic!("reserved variable reached the builder")
        })
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("duplicate component variable: {variable}")
        );
    }
}

/// Component exports remain unique across the local and shipped roots.
#[test]
fn example_components_reject_cross_root_duplicates() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let folders = [root.path().join("local"), root.path().join("shipped")];
    for folder in &folders {
        fs::create_dir(folder).unwrap();
        fs::write(
            folder.join("fixture.rs"),
            "// maestro-component: MAESTRO_FIXTURE_COMPONENT\n",
        )
        .unwrap();
    }
    let mut builds = 0;
    let error = tooling::example_files::prepare(&folders, |_, _| {
        builds += 1;
        Ok(PathBuf::from("/components/fixture.wasm"))
    })
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "duplicate component variable: MAESTRO_FIXTURE_COMPONENT"
    );
    assert_eq!(builds, 1);
}

/// Marked directory entry points use the same target as the generated manifest.
#[test]
fn example_components_prepare_directory_targets() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    fs::create_dir(root.path().join("with-deps")).unwrap();
    fs::write(
        root.path().join("with-deps/main.rs"),
        "// maestro-component: MAESTRO_DIRECTORY_COMPONENT\n",
    )
    .unwrap();
    let entries =
        tooling::example_files::prepare(&[root.path().to_path_buf()], |name, variable| {
            assert_eq!(name, "with_deps");
            assert_eq!(variable, "MAESTRO_DIRECTORY_COMPONENT");
            Ok(PathBuf::from(format!("/components/{name}.wasm")))
        })
        .unwrap();
    assert_eq!(
        entries,
        "MAESTRO_DIRECTORY_COMPONENT=/components/with_deps.wasm\n"
    );
}

/// A top-level main source retains its file-stem target rather than its root name.
#[test]
fn example_components_prepare_top_level_main() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    fs::write(
        root.path().join("main.rs"),
        "// maestro-component: MAESTRO_MAIN_COMPONENT\n",
    )
    .unwrap();
    let entries = tooling::example_files::prepare(&[root.path().to_path_buf()], |name, _| {
        assert_eq!(name, "main");
        Ok(PathBuf::from("/components/main.wasm"))
    })
    .unwrap();
    assert_eq!(entries, "MAESTRO_MAIN_COMPONENT=/components/main.wasm\n");
}
