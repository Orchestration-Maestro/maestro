//! The shipped executable delegates startup to its frontend entry.
use crate::support::Root;

#[test]
fn shipped_root_runs_version_and_help_without_placeholder() {
    let root = Root::new();
    let version = root.output(&["--version"]);
    assert!(version.status.success());
    assert!(version.stdout.is_empty());
    assert_eq!(
        String::from_utf8(version.stderr).unwrap(),
        format!("{}\n", maestro_app::VERSION)
    );
    let help = root.output(&["--help"]);
    assert!(
        help.status.success(),
        "{}",
        String::from_utf8_lossy(&help.stderr)
    );
    assert!(help.stdout.is_empty());
    assert!(String::from_utf8_lossy(&help.stderr).contains("Usage:"));
    let ordinary = root.output(&[]);
    assert_eq!(ordinary.status.code(), Some(1));
    assert!(ordinary.stdout.is_empty());
    assert!(String::from_utf8_lossy(&ordinary.stderr).contains("No models available"));
}

#[test]
fn shipped_root_reports_whole_millisecond_timings() {
    let root = Root::new();
    root.models("http://127.0.0.1:1/v1");
    let output = root
        .command(&[
            "--mode",
            "rpc",
            "--provider",
            "startup",
            "--model",
            "controlled",
        ])
        .env("MAESTRO_TIMING", "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "{stderr}");
    assert!(stderr.contains("--- Startup Timings ---"), "{stderr}");
    let samples: Vec<_> = stderr
        .lines()
        .filter_map(|line| line.strip_suffix("ms"))
        .collect();
    assert!(samples.len() > 1, "{stderr}");
    for sample in samples {
        let (_, milliseconds) = sample.rsplit_once(": ").unwrap();
        assert!(milliseconds.parse::<u64>().is_ok(), "{sample}");
    }
}

#[cfg(unix)]
#[test]
fn shipped_root_forwards_arguments_and_process_setup() {
    use crate::support::rpc::Rpc;
    use serde_json::json;
    let root = Root::new();
    root.models("http://127.0.0.1:1/v1");
    assert_eq!(
        std::path::Path::new(env!("CARGO_BIN_EXE_maestro"))
            .file_stem()
            .unwrap(),
        "maestro"
    );
    let mut rpc = Rpc::start(&root);
    let answer = rpc.request(
        json!({"type":"bash", "command":"printf '%s|literal $HOME' \"$MAESTRO_CODING_AGENT\""}),
    );
    assert_eq!(answer["output"], "true|literal $HOME");
    let removed = rpc.request(json!({"type":"bash", "command":"unset MAESTRO_CODING_AGENT; printf '%s' \"$MAESTRO_CODING_AGENT\""}));
    assert_eq!(removed["output"], "");
    rpc.finish();
    #[cfg(target_os = "linux")]
    {
        let failed_setup = root
            .command(&["--version"])
            .env("MAESTRO_PTY_PROGRAM", "/bin/true")
            .env("MAESTRO_PTY_COUNT", "invalid")
            .output()
            .unwrap();
        assert_eq!(failed_setup.status.code(), Some(1));
        assert!(!String::from_utf8_lossy(&failed_setup.stderr).contains(maestro_app::VERSION));
        assert!(
            String::from_utf8_lossy(&failed_setup.stderr)
                .contains("invalid terminal argument count")
        );
    }
}

#[test]
fn shipped_model_account_uses_its_own_command_identity() {
    let root = Root::new();
    let binary = std::path::Path::new(env!("CARGO_BIN_EXE_maestro")).with_file_name(format!(
        "maestro-model-account{}",
        std::env::consts::EXE_SUFFIX
    ));
    let help = std::process::Command::new(&binary)
        .env_clear()
        .arg("--help")
        .current_dir(root.path("project"))
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(help.stderr.is_empty());
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("maestro-model-account"), "{text}");
    assert!(!text.contains("maestro install"), "{text}");
    let mut login = std::process::Command::new(&binary)
        .env_clear()
        .arg("login")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(login.stdin.as_mut().unwrap(), b"0\n").unwrap();
    let rejected = login.wait_with_output().unwrap();
    assert_eq!(rejected.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&rejected.stdout).contains("Enter number"));
    assert_eq!(
        String::from_utf8(rejected.stderr).unwrap(),
        "Invalid selection\n"
    );
    let unknown = std::process::Command::new(binary)
        .env_clear()
        .arg("unrecognized")
        .output()
        .unwrap();
    assert_eq!(unknown.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(unknown.stderr).unwrap(),
        "Unknown command: unrecognized\nUse 'maestro-model-account --help' for usage\n"
    );
}

#[test]
fn package_relative_source_uses_global_settings_base() {
    let root = Root::new();
    let package = root.path("project/packages/local-package");
    std::fs::create_dir_all(&package).unwrap();
    let output = root.output(&["install", "./packages/local-package"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let settings: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.path("agent/settings.json")).unwrap()).unwrap();
    let sources = settings["packages"].as_array().unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(
        root.path("agent")
            .join(sources[0].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        package.canonicalize().unwrap()
    );
}

#[test]
fn package_remove_accepts_trailing_separator() {
    let root = Root::new();
    let path = format!("{}/", root.path("local-package").display());
    std::fs::create_dir_all(&path).unwrap();
    let installed = root.output(&["install", &path]);
    assert!(
        installed.status.success(),
        "{}",
        String::from_utf8_lossy(&installed.stderr)
    );
    let settings = || {
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(root.path("agent/settings.json")).unwrap(),
        )
        .unwrap()
    };
    assert_eq!(settings()["packages"].as_array().unwrap().len(), 1);
    let removed = root.output(&["remove", &path]);
    assert!(
        removed.status.success(),
        "{}",
        String::from_utf8_lossy(&removed.stderr)
    );
    assert_eq!(settings()["packages"].as_array().unwrap().len(), 0);
}

#[test]
fn package_install_help_precedes_runtime() {
    let root = Root::new();
    let output = root.output(&["install", "--help"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("Usage:") && text.contains("maestro install <source> [-l]"),
        "{text}"
    );
    assert!(!root.path("agent/settings.json").exists());
}

#[test]
fn package_unknown_install_option_is_friendly() {
    let output = Root::new().output(&["install", "--unknown"]);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(
        text.contains("Unknown option --unknown for \"install\"."),
        "{text}"
    );
    assert!(
        text.contains("Use \"maestro --help\" or \"maestro install <source> [-l]\"."),
        "{text}"
    );
}

#[test]
fn package_missing_install_source_is_friendly() {
    let output = Root::new().output(&["install"]);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(text.contains("Missing install source."), "{text}");
    assert!(
        text.contains("Usage: maestro install <source> [-l]"),
        "{text}"
    );
    assert!(!text.contains("at "), "{text}");
}

#[test]
fn package_update_hint_retains_configured_prefix() {
    let root = Root::new();
    std::fs::write(
        root.path("agent/settings.json"),
        r#"{"packages":["npm:maestro-formatter"]}"#,
    )
    .unwrap();
    let output = root.output(&["update", "maestro-formatter"]);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(
        text.contains("Did you mean npm:maestro-formatter?"),
        "{text}"
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Updated"));
    let stored: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.path("agent/settings.json")).unwrap()).unwrap();
    assert_eq!(
        stored["packages"],
        serde_json::json!(["npm:maestro-formatter"])
    );
}

#[cfg(unix)]
#[test]
fn forced_update_uses_configured_manager_without_release_fetch() {
    use serde_json::json;
    let root = Root::new();
    let prefix = root.path("global-prefix");
    let package = prefix.join("lib/node_modules/maestro");
    std::fs::create_dir_all(package.join("bin")).unwrap();
    let binary = package.join("bin/maestro");
    std::fs::copy(env!("CARGO_BIN_EXE_maestro"), &binary).unwrap();
    let script = root.path("fake-npm.sh");
    std::fs::write(&script, "if [ \"$3\" = root ]; then printf '%s/lib/node_modules\\n' \"$2\"; else printf '%s\\n' \"$@\" > \"$RECORD\"; fi\n").unwrap();
    let settings =
        |base: &std::path::Path| json!({"npmCommand":["/bin/sh",script,"--prefix",base]});
    std::fs::write(
        root.path("agent/settings.json"),
        settings(&prefix).to_string(),
    )
    .unwrap();
    std::fs::create_dir(root.path("project/.maestro")).unwrap();
    std::fs::write(
        root.path("project/.maestro/settings.json"),
        settings(&root.path("project-prefix")).to_string(),
    )
    .unwrap();
    let environment = root.command(&[]);
    let output = std::process::Command::new(binary)
        .env_clear()
        .envs(
            environment
                .get_envs()
                .filter_map(|(key, value)| value.map(|value| (key, value))),
        )
        .env("MAESTRO_PACKAGE_DIR", &package)
        .env("RECORD", root.path("record"))
        .current_dir(root.path("project"))
        .args(["update", "--self", "--force"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let recorded = std::fs::read_to_string(root.path("record")).unwrap();
    assert!(recorded.contains(prefix.to_str().unwrap()), "{recorded}");
    assert!(recorded.lines().any(|arg| arg == "maestro"), "{recorded}");
    assert!(!recorded.contains("project-prefix"), "{recorded}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("Updated maestro"));
}

#[test]
fn rpc_root_reports_state() {
    crate::support::connected(|_, rpc, _| {
        let state = rpc.request(serde_json::json!({"type":"get_state"}));
        assert_eq!(state["model"]["provider"], "startup");
        assert_eq!(state["model"]["id"], "controlled");
        assert_eq!(state["isStreaming"], false);
        assert_eq!(state["messageCount"], 0);
    });
}

#[test]
fn rpc_root_persists_prompt_history() {
    crate::support::connected(|_, rpc, provider| {
        rpc.prompt("hello");
        assert!(
            provider.requests.lock().unwrap()[0]["messages"]
                .to_string()
                .contains("hello")
        );
        assert!(
            rpc.events
                .iter()
                .filter(|event| event["type"] == "message_end")
                .count()
                >= 2
        );
        let history = crate::support::history(rpc);
        assert_eq!(history[0]["type"], "session");
        for role in ["user", "assistant"] {
            assert!(
                history
                    .iter()
                    .any(|entry| entry["type"] == "message" && entry["message"]["role"] == role),
                "{history:?}"
            );
        }
    });
}

#[test]
fn rpc_root_compacts_history() {
    crate::support::connected(|_, rpc, _| {
        rpc.prompt("hello");
        let result = rpc.request(serde_json::json!({"type":"compact"}));
        assert!(result["summary"].is_string(), "{result}");
        assert!(result["tokensBefore"].as_f64().unwrap() > 0.0);
        let history = crate::support::history(rpc);
        let summaries: Vec<_> = history
            .iter()
            .filter(|entry| entry["type"] == "compaction")
            .collect();
        assert_eq!(summaries.len(), 1);
        assert!(summaries[0]["summary"].is_string());
    });
}

#[cfg(unix)]
#[test]
fn rpc_root_executes_bash() {
    crate::support::connected(|_, rpc, _| {
        let result = rpc.request(serde_json::json!({"type":"bash", "command":"echo hello"}));
        assert_eq!(result["output"].as_str().unwrap().trim(), "hello");
        assert_eq!(result["exitCode"], 0);
        assert_eq!(result["cancelled"], false);
    });
}

#[cfg(unix)]
#[test]
fn rpc_root_keeps_bash_context_entry() {
    crate::support::connected(|_, rpc, _| {
        rpc.prompt("initialize");
        rpc.request(serde_json::json!({"type":"bash", "command":"echo root-context-Ω"}));
        let history = crate::support::history(rpc);
        let entries: Vec<_> = history
            .iter()
            .filter(|entry| entry["message"]["role"] == "bashExecution")
            .collect();
        assert_eq!(entries.len(), 1);
        assert!(
            entries[0]["message"]["output"]
                .as_str()
                .unwrap()
                .contains("root-context-Ω")
        );
    });
}

#[cfg(unix)]
#[test]
fn rpc_root_sends_bash_context_to_provider() {
    crate::support::connected(|_, rpc, provider| {
        rpc.request(serde_json::json!({"type":"bash", "command":"echo root-context-Ω"}));
        rpc.prompt("what was the output");
        assert!(
            provider.requests.lock().unwrap()[0]["messages"]
                .to_string()
                .contains("root-context-Ω")
        );
        let answer = rpc
            .events
            .iter()
            .find(|event| event["type"] == "message_end" && event["message"]["role"] == "assistant")
            .unwrap();
        assert!(
            answer["message"]["content"]
                .to_string()
                .contains("root-context-Ω")
        );
    });
}

#[test]
fn rpc_root_sets_thinking() {
    crate::support::connected(|_, rpc, _| {
        rpc.request(serde_json::json!({"type":"set_thinking_level", "level":"high"}));
        let state = rpc.request(serde_json::json!({"type":"get_state"}));
        assert_eq!(state["thinkingLevel"], "high");
    });
}

#[test]
fn rpc_root_cycles_thinking() {
    crate::support::connected(|_, rpc, _| {
        let before = rpc.request(serde_json::json!({"type":"get_state"}));
        let cycle = rpc.request(serde_json::json!({"type":"cycle_thinking_level"}));
        assert_ne!(cycle["level"], before["thinkingLevel"]);
        let after = rpc.request(serde_json::json!({"type":"get_state"}));
        assert_eq!(after["thinkingLevel"], cycle["level"]);
    });
}

#[test]
fn rpc_root_lists_available_models() {
    crate::support::connected(|_, rpc, _| {
        let result = rpc.request(serde_json::json!({"type":"get_available_models"}));
        let models = result["models"].as_array().unwrap();
        assert!(!models.is_empty());
        for model in models {
            assert!(model["provider"].is_string() && model["id"].is_string());
            assert!(model["contextWindow"].as_f64().unwrap() > 0.0);
            assert!(model["reasoning"].is_boolean());
        }
    });
}

#[test]
fn rpc_root_reports_session_stats() {
    crate::support::connected(|_, rpc, _| {
        rpc.prompt("hello");
        let stats = rpc.request(serde_json::json!({"type":"get_session_stats"}));
        assert!(stats["sessionFile"].is_string() && stats["sessionId"].is_string());
        assert!(stats["userMessages"].as_u64().unwrap() >= 1);
        assert!(stats["assistantMessages"].as_u64().unwrap() >= 1);
    });
}

#[test]
fn rpc_root_creates_new_session() {
    crate::support::connected(|_, rpc, _| {
        rpc.prompt("hello");
        let before = rpc.request(serde_json::json!({"type":"get_state"}));
        assert!(before["messageCount"].as_u64().unwrap() > 0);
        rpc.request(serde_json::json!({"type":"new_session"}));
        let after = rpc.request(serde_json::json!({"type":"get_state"}));
        assert_eq!(after["messageCount"], 0);
    });
}

#[test]
fn rpc_root_returns_last_assistant_text() {
    crate::support::connected(|_, rpc, _| {
        let before = rpc.request(serde_json::json!({"type":"get_last_assistant_text"}));
        assert!(before.get("text").is_none());
        rpc.prompt("reply test123");
        let after = rpc.request(serde_json::json!({"type":"get_last_assistant_text"}));
        assert!(after["text"].as_str().unwrap().contains("test123"));
    });
}

#[test]
fn rpc_root_retains_session_name() {
    crate::support::connected(|_, rpc, _| {
        let before = rpc.request(serde_json::json!({"type":"get_state"}));
        assert!(before.get("sessionName").is_none());
        rpc.prompt("initialize");
        rpc.request(serde_json::json!({"type":"set_session_name", "name":"my-test-session"}));
        let state = rpc.request(serde_json::json!({"type":"get_state"}));
        assert_eq!(state["sessionName"], "my-test-session");
        let history = crate::support::history(rpc);
        let names: Vec<_> = history
            .iter()
            .filter(|entry| entry["type"] == "session_info")
            .collect();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0]["name"], "my-test-session");
    });
}

#[test]
fn rpc_root_exports_html() {
    let root = Root::new();
    let provider = crate::support::provider::Provider::new();
    root.models(&provider.base);
    let mut command = root.command(&[
        "--mode",
        "rpc",
        "--provider",
        "startup",
        "--model",
        "controlled",
    ]);
    command.env("MAESTRO_PACKAGE_DIR", root.bundle());
    let mut rpc = crate::support::rpc::Rpc::command(&mut command);
    rpc.prompt("hello");
    let result = rpc.request(serde_json::json!({"type":"export_html"}));
    let path = result["path"].as_str().unwrap();
    assert_eq!(
        std::path::Path::new(path).extension(),
        Some(std::ffi::OsStr::new("html"))
    );
    let written = root.path("project").join(path);
    assert!(written.exists());
    let document = std::fs::read_to_string(written).unwrap();
    assert!(document.contains("id=\"session-data\""), "{document}");
    assert!(!document.contains("{{SESSION_DATA}}"));
    let state = rpc.request(serde_json::json!({"type":"get_state"}));
    let exported = root
        .command(&[
            "--export",
            state["sessionFile"].as_str().unwrap(),
            "explicit.html",
        ])
        .env("MAESTRO_PACKAGE_DIR", root.bundle())
        .output()
        .unwrap();
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    assert_eq!(
        String::from_utf8(exported.stderr).unwrap(),
        "Exported to: explicit.html\n"
    );
    assert!(root.path("project/explicit.html").exists());
    rpc.finish();
}

#[test]
fn shipped_root_uses_native_color_policy() {
    let root = Root::new();
    for (force, styled) in [("1", true), ("0", false), ("false", false)] {
        let output = root
            .command(&["install", "--help"])
            .env("FORCE_COLOR", force)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains("\x1b[1mUsage:\x1b[22m"),
            styled,
            "FORCE_COLOR={force}"
        );
    }
}
