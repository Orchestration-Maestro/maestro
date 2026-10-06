#![cfg(unix)]

mod support;

use std::process::Command;
use support::Workspace;

fn tooling() -> &'static str {
    env!("CARGO_BIN_EXE_repository_tools")
}

#[test]
fn test_children_drop_credentials_and_keep_build_inputs() {
    let workspace = Workspace::new();
    let probe = workspace.command("environment", "env");
    let parent = std::env::vars_os().collect::<Vec<_>>();
    let poisoned = "ANTHROPIC_API_KEY ANTHROPIC_OAUTH_TOKEN OPENAI_API_KEY GEMINI_API_KEY GROQ_API_KEY CEREBRAS_API_KEY XAI_API_KEY OPENROUTER_API_KEY ZAI_API_KEY MISTRAL_API_KEY MINIMAX_API_KEY MINIMAX_CN_API_KEY KIMI_API_KEY HF_TOKEN AI_GATEWAY_API_KEY OPENCODE_API_KEY COPILOT_GITHUB_TOKEN GH_TOKEN GITHUB_TOKEN GOOGLE_APPLICATION_CREDENTIALS GOOGLE_CLOUD_PROJECT GCLOUD_PROJECT GOOGLE_CLOUD_LOCATION AWS_PROFILE AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_SESSION_TOKEN AWS_REGION AWS_DEFAULT_REGION AWS_BEARER_TOKEN_BEDROCK AWS_CONTAINER_CREDENTIALS_RELATIVE_URI AWS_CONTAINER_CREDENTIALS_FULL_URI AWS_WEB_IDENTITY_TOKEN_FILE BEDROCK_EXTENSIVE_MODEL_TEST FIREWORKS_API_KEY AZURE_OPENAI_API_KEY AZURE_OPENAI_BASE_URL AZURE_OPENAI_RESOURCE_NAME AWS_CONFIG_FILE AWS_SHARED_CREDENTIALS_FILE AWS_DEFAULT_PROFILE CLOUDSDK_CONFIG CLOUDSDK_AUTH_CREDENTIAL_FILE_OVERRIDE AZURE_CONFIG_DIR MAESTRO_CODING_AGENT_DIR HTTP_PROXY HTTPS_PROXY ALL_PROXY UNLISTED_API_KEY UNRELATED_CANARY";
    for value in ["poison", ""] {
        let mut command = Command::new(tooling());
        command
            .arg("isolate")
            .arg(&probe)
            .env("RUST_TEST_THREADS", "3");
        for name in poisoned.split_whitespace() {
            command.env(name, value);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        for name in poisoned.split_whitespace() {
            assert!(
                !text
                    .lines()
                    .any(|line| line.starts_with(&format!("{name}="))),
                "{text}"
            );
        }
        assert!(text.lines().any(|line| line == "RUST_TEST_THREADS=3"));
        assert!(text.lines().any(|line| line == "MAESTRO_NO_LOCAL_LLM=1"));
    }
    assert_eq!(parent, std::env::vars_os().collect::<Vec<_>>());
}

#[test]
fn test_children_have_fresh_home_temp_and_xdg_roots() {
    let workspace = Workspace::new();
    let original = workspace.root.join("original home");
    std::fs::create_dir(&original).unwrap();
    std::fs::write(original.join("auth.json"), b"sentinel").unwrap();
    #[cfg(target_os = "linux")]
    let (auth_events, _watcher) = {
        use notify::Watcher;
        let (send, receive) = std::sync::mpsc::channel();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                send.send(event.unwrap()).unwrap();
            })
            .unwrap();
        watcher
            .watch(
                &original.join("auth.json"),
                notify::RecursiveMode::NonRecursive,
            )
            .unwrap();
        std::fs::read(original.join("auth.json")).unwrap();
        let event = receive
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        assert!(
            matches!(event.kind, notify::EventKind::Access(_)),
            "{event:?}"
        );
        while receive.try_recv().is_ok() {}
        (receive, watcher)
    };

    let probe = workspace.command("roots", "for name in HOME TMPDIR XDG_CONFIG_HOME XDG_CACHE_HOME XDG_DATA_HOME XDG_STATE_HOME XDG_RUNTIME_DIR XDG_CONFIG_DIRS XDG_DATA_DIRS; do eval 'value=$'\"$name\"; test -d \"$value\" || exit 2; test -z \"$(ls -A \"$value\")\" || exit 3; printf '%s=%s\\n' \"$name\" \"$value\"; done; test \"$TMP\" = \"$TMPDIR\"; test \"$TEMP\" = \"$TMPDIR\"");
    let mut homes = Vec::new();
    for _ in 0..2 {
        let output = Command::new(tooling())
            .arg("isolate")
            .arg(&probe)
            .env("HOME", &original)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(!text.contains("Restored auth.json"));
        assert!(!text.contains("Moved auth.json to backup"));
        let roots = text
            .lines()
            .map(|line| line.split_once('=').unwrap().1.to_owned())
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 9);
        assert_eq!(
            roots
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            9
        );
        for root in &roots {
            assert!(!std::path::Path::new(root).exists());
        }
        homes.push(roots[0].clone());
    }
    assert_ne!(homes[0], homes[1]);
    #[cfg(target_os = "linux")]
    {
        assert!(
            auth_events
                .recv_timeout(std::time::Duration::from_millis(100))
                .is_err(),
            "original auth was accessed or mutated"
        );
        drop(_watcher);
    }

    assert_eq!(
        std::fs::read(original.join("auth.json")).unwrap(),
        b"sentinel"
    );
    assert!(!original.join("auth.json.backup").exists());
    let long_temporary = workspace.root.join("nested-temporary-root-".repeat(8));
    std::fs::create_dir(&long_temporary).unwrap();
    let socket = workspace.command("socket-probe", "python3 -c 'import os,socket; s=socket.socket(socket.AF_UNIX); s.bind(os.path.join(os.environ[\"TMPDIR\"], \"socket\"))'");
    let output = Command::new(tooling())
        .args(["isolate", tooling(), "isolate"])
        .arg(socket)
        .env("TMPDIR", long_temporary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "nested temporary paths broke native sockets: {output:?}"
    );
}

#[test]
fn child_exit_failure_and_abort_clean_only_owned_scratch() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    let workspace = Workspace::new();
    let scratch_parent = workspace.root.join("scratch");
    std::fs::create_dir(&scratch_parent).unwrap();
    std::fs::write(scratch_parent.join("unrelated"), b"keep").unwrap();
    for code in [0, 7] {
        let child = workspace.command(
            "exit-child",
            &format!("printf '%s\\n' \"$HOME\"; exit {code}"),
        );
        let output = Command::new(tooling())
            .arg("isolate")
            .arg(child)
            .env("TMPDIR", &scratch_parent)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(code));
        assert!(!std::path::Path::new(String::from_utf8(output.stdout).unwrap().trim()).exists());
    }
    let output = Command::new(tooling())
        .args(["isolate", "/nonexistent/maestro-fixture"])
        .env("TMPDIR", &scratch_parent)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("repository tools: spawn child:"));
    let child = workspace.command(
        "abort-child",
        "printf '%s\\n%s\\n' \"$HOME\" \"$$\"; exec sleep 60",
    );
    let mut runner = Command::new(tooling())
        .arg("isolate")
        .arg(child)
        .env("TMPDIR", &scratch_parent)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(runner.stdout.take().unwrap());
    let mut home = String::new();
    let mut pid = String::new();
    reader.read_line(&mut home).unwrap();
    reader.read_line(&mut pid).unwrap();
    assert!(
        Command::new("kill")
            .args(["-TERM", &runner.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let status = runner.wait().unwrap();
    let scratch_remains = std::path::Path::new(home.trim()).exists();
    let child_remains = Command::new("kill")
        .args(["-0", pid.trim()])
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success();
    if child_remains {
        let _ = Command::new("kill").args(["-KILL", pid.trim()]).status();
    }
    assert!(!status.success());
    assert!(!scratch_remains, "owned scratch remains after TERM");
    assert!(!child_remains, "child remains after TERM");
    assert_eq!(
        std::fs::read(scratch_parent.join("unrelated")).unwrap(),
        b"keep"
    );
    assert_eq!(std::fs::read_dir(&scratch_parent).unwrap().count(), 1);
}

#[test]
fn source_launch_forwards_cwd_and_literal_arguments() {
    let workspace = Workspace::new();
    let target = workspace.root.join("target directory");
    std::fs::create_dir_all(target.join("debug")).unwrap();
    let app = workspace.command(
        "application",
        "pwd; for arg; do printf '%s\\0' \"$arg\"; done",
    );
    std::fs::copy(app, target.join("debug/maestro")).unwrap();
    let cargo = workspace.command("cargo", "test \"$1\" = build; exit 0");
    let args = [
        "",
        " space ",
        "\t",
        "\n",
        "'\"",
        "*",
        "--",
        "--NO-ENV",
        "--no-env",
        "\u{feff}a\u{85}",
        "🌍",
        "repeat",
        "repeat",
    ];
    for shell in ["bash", "powershell"] {
        let mut command = source_adapter(shell);
        let output = command
            .current_dir(&workspace.root)
            .args(args)
            .env("CARGO", &cargo)
            .env("CARGO_TARGET_DIR", &target)
            .output()
            .unwrap();
        assert!(output.status.success(), "{shell}: {output:?}");
        let first = format!(
            "Running without API keys...\n{}\n",
            workspace.root.display()
        );
        assert!(
            output.stdout.starts_with(first.as_bytes()),
            "{shell}: {output:?}"
        );
        let expected = args
            .iter()
            .filter(|arg| {
                **arg != "--no-env"
                    && !(shell == "powershell" && arg.eq_ignore_ascii_case("--no-env"))
            })
            .flat_map(|arg| arg.as_bytes().iter().copied().chain([0]))
            .collect::<Vec<_>>();
        assert_eq!(&output.stdout[first.len()..], expected, "{shell}");
    }
}

fn source_adapter(shell: &str) -> Command {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts");
    if shell == "bash" {
        Command::new(root.join("run-source.sh"))
    } else {
        let mut command =
            Command::new(std::env::var_os("MAESTRO_TEST_PWSH").unwrap_or_else(|| "pwsh".into()));
        command
            .args(["-NoProfile", "-File"])
            .arg(root.join("run-source.ps1"));
        command
    }
}

#[test]
fn source_no_env_keeps_each_shells_exact_scope() {
    let workspace = Workspace::new();
    let target = workspace.root.join("target");
    std::fs::create_dir_all(target.join("debug")).unwrap();
    let app = workspace.command("environment-source", "env");
    std::fs::copy(app, target.join("debug/maestro")).unwrap();
    let cargo = workspace.command("build-source", "exit 0");
    let removed = "ANTHROPIC_API_KEY ANTHROPIC_OAUTH_TOKEN OPENAI_API_KEY GEMINI_API_KEY GROQ_API_KEY CEREBRAS_API_KEY XAI_API_KEY OPENROUTER_API_KEY ZAI_API_KEY MISTRAL_API_KEY MINIMAX_API_KEY MINIMAX_CN_API_KEY AI_GATEWAY_API_KEY OPENCODE_API_KEY COPILOT_GITHUB_TOKEN GH_TOKEN GITHUB_TOKEN GOOGLE_APPLICATION_CREDENTIALS GOOGLE_CLOUD_PROJECT GCLOUD_PROJECT GOOGLE_CLOUD_LOCATION AWS_PROFILE AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_SESSION_TOKEN AWS_REGION AWS_DEFAULT_REGION AWS_BEARER_TOKEN_BEDROCK AWS_CONTAINER_CREDENTIALS_RELATIVE_URI AWS_CONTAINER_CREDENTIALS_FULL_URI AWS_WEB_IDENTITY_TOKEN_FILE AZURE_OPENAI_API_KEY AZURE_OPENAI_BASE_URL AZURE_OPENAI_RESOURCE_NAME";
    let retained = [
        "KIMI_API_KEY",
        "FIREWORKS_API_KEY",
        "UNKNOWN_API_KEY",
        "MAESTRO_NO_LOCAL_LLM",
        "MAESTRO_CODING_AGENT_DIR",
    ];
    for shell in ["bash", "powershell"] {
        for no_env in [false, true] {
            for value in ["canary", ""] {
                let mut command = source_adapter(shell);
                command
                    .env("CARGO", &cargo)
                    .env("CARGO_TARGET_DIR", &target);
                for name in removed
                    .split_whitespace()
                    .chain(retained)
                    .chain(["HF_TOKEN"])
                {
                    command.env(name, value);
                }
                if no_env {
                    command.arg("--no-env");
                }
                let output = command.output().unwrap();
                assert!(output.status.success(), "{shell}: {output:?}");
                let text = String::from_utf8(output.stdout).unwrap();
                assert_eq!(
                    text.lines()
                        .filter(|line| *line == "Running without API keys...")
                        .count(),
                    usize::from(no_env)
                );
                for name in removed.split_whitespace() {
                    assert_eq!(
                        text.lines().any(|line| line == format!("{name}={value}")),
                        !no_env,
                        "{shell}: {name}"
                    );
                }
                for name in retained {
                    assert!(
                        text.lines().any(|line| line == format!("{name}={value}")),
                        "{shell}: {name}"
                    );
                }
                assert_eq!(
                    text.lines().any(|line| line == format!("HF_TOKEN={value}")),
                    !(no_env && shell == "bash")
                );
                assert!(text.lines().any(|line| line.starts_with("HOME=")));
            }
        }
    }
}

#[test]
fn source_launch_reports_missing_cargo_and_build_failures() {
    let workspace = Workspace::new();
    let missing = workspace.root.join("missing cargo");
    let target = workspace.root.join("target");
    std::fs::create_dir_all(target.join("debug")).unwrap();
    let app = workspace.command("failure-source", "printf 'application ran\\n'; exit 13");
    std::fs::copy(app, target.join("debug/maestro")).unwrap();
    for shell in ["bash", "powershell"] {
        let output = source_adapter(shell)
            .env("CARGO", &missing)
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
        assert!(output.stdout.is_empty());
        for code in [7, 0] {
            let cargo = workspace.command("status-source", &format!("exit {code}"));
            let output = source_adapter(shell)
                .env("CARGO", cargo)
                .env("CARGO_TARGET_DIR", &target)
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(if code == 0 { 13 } else { code })
            );
            assert_eq!(
                output.stdout,
                if code == 0 {
                    b"application ran\n".as_slice()
                } else {
                    b""
                }
            );
        }
    }
}

#[test]
fn parallel_bootstrap_does_not_share_compiler_outputs() {
    let workspace = Workspace::new();
    let original = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = workspace.root.join("checkout with spaces");
    for relative in [
        "scripts/tooling-bootstrap.sh",
        "scripts/tooling-bootstrap.ps1",
        "rust-toolchain.toml",
        "crates/maestro-test-conventions/src/bin/repository_tools.rs",
        "crates/maestro-test-conventions/src/repository_tools/mod.rs",
        "crates/maestro-test-conventions/src/repository_tools/assets.rs",
        "crates/maestro-test-conventions/src/repository_tools/pre_commit.rs",
        "crates/maestro-test-conventions/src/repository_tools/cargo_target.rs",
        "crates/maestro-test-conventions/src/repository_tools/rustdoc.rs",
        "crates/maestro-test-conventions/src/repository_tools/format_staged.rs",
        "crates/maestro-test-conventions/src/repository_tools/isolation.rs",
        "crates/maestro-test-conventions/src/repository_tools/source_launch.rs",
    ] {
        let destination = root.join(relative);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::copy(original.join(relative), destination).unwrap();
    }
    for shell in ["bash", "powershell"] {
        if root.join("target/repository-tools").exists() {
            std::fs::remove_file(root.join("target/repository-tools")).unwrap();
        }
        let children = (0..8)
            .map(|_| {
                let mut command = if shell == "bash" {
                    Command::new(root.join("scripts/tooling-bootstrap.sh"))
                } else {
                    let mut command = Command::new(
                        std::env::var_os("MAESTRO_TEST_PWSH").unwrap_or_else(|| "pwsh".into()),
                    );
                    command
                        .args(["-NoProfile", "-File"])
                        .arg(root.join("scripts/tooling-bootstrap.ps1"));
                    command
                };
                command
                    .args(["isolate", "/bin/true"])
                    .current_dir(&workspace.root)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let outputs = children
            .into_iter()
            .map(|child| child.wait_with_output().unwrap())
            .collect::<Vec<_>>();
        for output in outputs {
            assert!(output.status.success(), "{shell}: {output:?}");
        }
        assert_eq!(std::fs::read_dir(root.join("target")).unwrap().count(), 1);
    }
}

#[test]
fn ordinary_cargo_programs_keep_their_environment() {
    let workspace = Workspace::new();
    let program = workspace.command(
        "contribution-policy",
        "test \"$GITHUB_TOKEN\" = canary; test \"$1\" != --list; printf 'ordinary\\n'",
    );
    let output = Command::new(tooling())
        .arg("cargo-target")
        .arg(&program)
        .env("GITHUB_TOKEN", "canary")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"ordinary\n");
    let deps = workspace.root.join("target/debug/deps");
    std::fs::create_dir_all(&deps).unwrap();
    let unknown = deps.join("unknown-0123456789abcdef");
    std::fs::copy(program, &unknown).unwrap();
    let output = Command::new(tooling())
        .arg("cargo-target")
        .arg(unknown)
        .env("GITHUB_TOKEN", "canary")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .starts_with("repository tools: unknown test artifact:")
    );
}

#[test]
#[ignore = "requires the repository's pinned just/prek; runs in just test until shared CI provisions pinned tools"]
fn supported_routes_enter_the_same_isolation_boundary() {
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
    // Controlled Cargo observes the environment at every recipe subprocess.
    let bin = workspace.root.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let probe = workspace.command("cargo-probe", "test -z \"${GITHUB_TOKEN+x}\" || exit 42; test \"$MAESTRO_NO_LOCAL_LLM\" = 1 || exit 43; printf 'route:%s\\n' \"$*\"");
    std::os::unix::fs::symlink(&probe, bin.join("cargo")).unwrap();
    let path = std::env::join_paths(
        std::iter::once(bin).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let raw = Command::new(&probe)
        .env("GITHUB_TOKEN", "controlled")
        .output()
        .unwrap();
    assert_eq!(raw.status.code(), Some(42));
    let git = Command::new("git")
        .args(["init", "-q"])
        .current_dir(&workspace.root)
        .status()
        .unwrap();
    assert!(git.success());
    for route in ["check", "test", "ci", "pre-commit"] {
        let output = Command::new("just")
            .arg(route)
            .current_dir(&workspace.root)
            .env("PATH", &path)
            .env("GITHUB_TOKEN", "controlled")
            .output()
            .unwrap();
        assert!(output.status.success(), "{route}: {output:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        if ["test", "ci"].contains(&route) {
            assert_eq!(
                stdout.matches("Running tests without API keys...").count(),
                1
            );
        }
        assert!(stdout.contains("route:"));
    }
    let config = std::fs::read_to_string(root.join(".cargo/config.toml")).unwrap();
    assert!(config.contains("cargo-target"));
    assert!(config.contains("run-rustdoc.sh"));
}
