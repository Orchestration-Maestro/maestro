use super::*;

fn foreground_process(
    pid: u32,
    name: &str,
    argv: &[&str],
) -> maestro_process::pty::foreground::ForegroundProcess {
    maestro_process::pty::foreground::ForegroundProcess {
        pid,
        name: name.to_string(),
        argv0: None,
        argv: Some(argv.iter().map(|arg| (*arg).to_string()).collect()),
        cmdline: Some(argv.join(" ")),
    }
}

fn temp_detection_path(name: &str) -> std::path::PathBuf {
    let unique = format!(
        "maestro-detection-{}-{}-{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos()
    );
    std::env::temp_dir().join(unique)
}

#[test]
fn job_selection_recognizes_a_wrapped_agent() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![
            foreground_process(1, "node", &["node", "/path/to/bin/codex"]),
            foreground_process(2, "bash", &["bash"]),
        ],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Codex, "codex".to_string()))
    );
}

#[test]
fn node_script_and_package_entry_identify_the_agent() {
    for argv in [
        vec!["node", "/home/user/.fnm/bin/qwen"],
        vec![
            "node.exe",
            r"C:\Users\user\AppData\Roaming\npm\node_modules\@qwen-code\qwen-code\dist\index.js",
        ],
    ] {
        let job = maestro_process::pty::foreground::ForegroundJob {
            process_group_id: 123,
            processes: vec![foreground_process(123, "MainThread", &argv)],
        };

        assert_eq!(
            identify_agent_in_job(&job),
            Some((Agent::Qwen, "qwen".to_string()))
        );
    }
}

#[test]
fn recognized_group_leader_beats_a_wrapped_member() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 42,
        processes: vec![
            foreground_process(42, "claude", &["claude"]),
            foreground_process(43, "node", &["node", "/tmp/mcp/bin/codex"]),
        ],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Claude, "claude".to_string()))
    );
}

#[test]
fn unknown_group_leader_allows_member_identification() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 42,
        processes: vec![
            foreground_process(42, "bash", &["bash"]),
            foreground_process(43, "node", &["node", "/tmp/mcp/bin/codex"]),
        ],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Codex, "codex".to_string()))
    );
}

#[test]
fn versioned_python_recognizes_an_agent_script() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "python3.12",
            &[
                "/nix/store/example/bin/python3.12",
                "/nix/store/example/bin/hermes",
                "--resume",
                "session-id",
            ],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Hermes, "hermes".to_string()))
    );
}

#[test]
fn wrapped_program_uses_argv_executable_identity() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            ".codex-wrapped",
            &["/etc/profiles/per-user/user/bin/codex", "--model", "gpt-5"],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Codex, "codex".to_string()))
    );
}

#[test]
fn wrapped_alias_returns_its_canonical_agent_label() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            ".claude-code-wrapped",
            &["/nix/store/example/bin/claude-code"],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Claude, "claude".to_string()))
    );
}

#[test]
fn shell_script_path_identifies_the_catalogue_entry() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            "sh",
            &["/bin/sh", "/tmp/test-bin/pi"],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Pi, "pi".to_string()))
    );
}

#[test]
fn bun_script_path_identifies_the_catalogue_entry() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "bun",
            &["bun", "/home/can/.bun/bin/omp"],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Omp, "omp".to_string()))
    );
}

#[test]
fn scoped_package_cli_identifies_the_catalogue_entry() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "node.exe",
            &[
                "node.exe",
                "C:\\Users\\herdr\\AppData\\Roaming\\npm\\node_modules\\@earendil-works\\pi-coding-agent\\dist\\cli.js",
            ],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Pi, "pi".to_string()))
    );
}

#[test]
fn unscoped_package_cli_identifies_the_catalogue_entry() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "node.exe",
            &[
                "node.exe",
                "C:\\Users\\herdr\\AppData\\Roaming\\npm\\node_modules\\mastracode\\dist\\cli.js",
            ],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Mastracode, "mastracode".to_string()))
    );
}

#[test]
fn package_build_script_is_not_an_agent_cli() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "node.exe",
            &[
                "node.exe",
                "C:\\Users\\herdr\\AppData\\Roaming\\npm\\node_modules\\@earendil-works\\pi-coding-agent\\scripts\\build.js",
            ],
        )],
    };

    assert_eq!(identify_agent_in_job(&job), None);
}

#[test]
fn command_interpreter_payload_identifies_an_agent() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            "cmd.exe",
            &[
                "cmd.exe",
                "/D",
                "/S",
                "/C",
                "C:\\Users\\herdr\\AppData\\Roaming\\npm\\codex.cmd --model gpt-5",
            ],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Codex, "codex".to_string()))
    );
}

#[test]
fn powershell_file_operand_identifies_an_agent() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            "powershell.exe",
            &[
                "powershell.exe",
                "-NoProfile",
                "-File",
                "C:\\Users\\herdr\\Documents\\PowerShell\\Scripts\\claude.ps1",
            ],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Claude, "claude".to_string()))
    );
}

#[test]
fn powershell_prompt_integration_stays_a_plain_shell() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            "powershell.exe",
            &[
                "powershell.exe",
                "-NoExit",
                "-Command",
                r"if ($null -eq $global:__HerdrOriginalPrompt) { $global:__HerdrOriginalPrompt = $function:prompt; function global:prompt { $out = @(& $global:__HerdrOriginalPrompt) -join ' '; $loc = $ExecutionContext.SessionState.Path.CurrentLocation; if ($loc.Provider.Name -eq 'FileSystem') { try { [Environment]::CurrentDirectory = $loc.ProviderPath } catch {}; $esc = [string][char]27; $out += $esc + ']9;9;' + $loc.ProviderPath + $esc + '\' }; $out } }",
            ],
        )],
    };

    assert_eq!(identify_agent_in_job(&job), None);
}

#[test]
fn alternate_executable_keeps_its_observed_process_name() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "opencode2",
            &["opencode2", "--standalone"],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::OpenCode, "opencode2".to_string()))
    );
}

#[test]
fn recognized_native_package_process_keeps_its_name() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "opencode.exe",
            &["/home/user/.local/share/pnpm/global/node_modules/opencode-ai/bin/opencode.exe"],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::OpenCode, "opencode.exe".to_string()))
    );
}

#[test]
fn argv_package_basename_returns_the_canonical_label() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "MainThread",
            &["/home/user/.local/share/pnpm/global/node_modules/opencode-ai/bin/opencode.exe"],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::OpenCode, "opencode".to_string()))
    );
}

#[test]
fn plain_shell_flags_do_not_supply_an_agent_script() {
    assert_eq!(
        wrapped_agent_name_from_runtime_argv("bash", Some(&["bash".into(), "-lc".into()])),
        None
    );
}

#[test]
fn python_inline_code_does_not_consume_later_agent_names() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            "python3",
            &["python3", "-c", "import time; time.sleep(60)", "/tmp/codex"],
        )],
    };

    assert_eq!(identify_agent_in_job(&job), None);
}

#[test]
fn node_eval_does_not_consume_later_agent_names() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            "node",
            &["node", "-e", "setTimeout(() => {}, 60000)", "/tmp/codex"],
        )],
    };

    assert_eq!(identify_agent_in_job(&job), None);
}

#[test]
fn shell_inline_command_does_not_consume_later_agent_names() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            "bash",
            &["bash", "-c", "sleep 60", "/tmp/codex"],
        )],
    };

    assert_eq!(identify_agent_in_job(&job), None);
}

#[test]
fn python_file_operand_can_identify_an_agent() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            "python3",
            &["python3", "/tmp/codex", "--model", "gpt-5"],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Codex, "codex".to_string()))
    );
}

#[test]
fn command_line_first_word_canonicalizes_an_alias() {
    assert_eq!(
        cmdline_argv0_agent_name("/nix/store/example/bin/ghcs"),
        Some("copilot".to_string())
    );
}

#[test]
fn command_line_helper_name_does_not_match_a_substring() {
    assert_eq!(cmdline_argv0_agent_name("/tmp/my-codex-helper"), None);
}

#[test]
fn symlinked_executable_resolves_agent_identity() {
    let dir = temp_detection_path("cursor-agent-symlink");
    std::fs::create_dir_all(&dir).expect("test directory should be created");
    let target = dir.join("cursor-agent");
    let link = dir.join("agent");
    std::fs::File::create_new(&target).expect("target should be created");
    std::os::unix::fs::symlink(&target, &link).expect("symlink should be created");

    let argv0 = link.to_string_lossy().into_owned();
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 42,
        processes: vec![foreground_process(
            42,
            "MainThread",
            &[&argv0, "--use-system-ca", "/tmp/index.js"],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Cursor, "cursor".to_string()))
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn job_priority_keeps_first_ties_and_prefers_wrapped_identities() {
    let mut job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 99,
        processes: vec![
            foreground_process(1, "claude", &["claude"]),
            foreground_process(2, "codex", &["codex"]),
        ],
    };
    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Claude, "claude".into()))
    );
    job.processes
        .push(foreground_process(3, "node", &["node", "/bin/qwen"]));
    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Qwen, "qwen".into()))
    );
    job.processes
        .push(foreground_process(4, "node", &["node", "/bin/codex"]));
    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Qwen, "qwen".into()))
    );
    job.processes.clear();
    assert_eq!(identify_agent_in_job(&job), None);
}

#[test]
fn script_operands_keep_option_and_inline_code_boundaries() {
    for (runtime, words, expected) in [
        (
            "node",
            vec!["node", "--require", "codex", "--", "/bin/qwen"],
            Some("qwen"),
        ),
        ("node", vec!["node", "--eval=codex", "/bin/qwen"], None),
        ("node", vec!["node", "-pcodex", "/bin/qwen"], None),
        ("python3", vec!["python3", "-m", "codex"], None),
        ("python3", vec!["python3", "-mcodex"], None),
        ("node", vec!["node", "--require"], None),
        ("node", vec!["node", "--"], None),
        ("tmux", vec!["tmux", "codex"], None),
    ] {
        let argv: Vec<_> = words.into_iter().map(str::to_owned).collect();
        assert_eq!(
            wrapped_agent_name_from_runtime_argv(runtime, Some(&argv)).as_deref(),
            expected
        );
    }
    assert_eq!(wrapped_agent_name_from_runtime_argv("node", None), None);
}

#[test]
fn command_introducers_and_encoded_commands_keep_their_boundaries() {
    for (runtime, words, expected) in [
        (
            "cmd",
            vec!["cmd", "/d", "/s", "/k", "call 'codex.cmd' --help"],
            Some("codex"),
        ),
        (
            "pwsh",
            vec!["pwsh", "-ExecutionPolicy", "codex", "-c", "& . 'qwen.ps1'"],
            Some("qwen"),
        ),
        ("pwsh", vec!["pwsh", "-enc", "codex"], None),
        ("pwsh", vec!["pwsh", "-NoExit", "codex.ps1"], Some("codex")),
        ("pwsh", vec!["pwsh", "-File"], None),
        ("cmd", vec!["cmd", "/c"], None),
    ] {
        let argv: Vec<_> = words.into_iter().map(str::to_owned).collect();
        assert_eq!(
            wrapped_agent_name_from_runtime_argv(runtime, Some(&argv)).as_deref(),
            expected
        );
    }
    assert_eq!(command_text_agent_name("'codex"), Some("codex".into()));
    assert_eq!(command_text_agent_name("  &  "), None);
}

#[test]
fn supplied_executable_identity_precedes_kernel_name_and_absent_arguments() {
    let mut process = foreground_process(1, "codex", &[]);
    process.argv0 = Some("claude-code".into());
    process.argv = None;
    process.cmdline = None;
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 1,
        processes: vec![process],
    };
    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Claude, "claude-code".into()))
    );
}

#[test]
fn package_near_misses_and_missing_paths_stay_unrecognized() {
    for path in [
        "",
        "--codex",
        "/missing/maestro-detection-executable",
        "/node_modules/@qwen-code/other/dist/index.js",
        "/node_modules/mastracode/build/cli.js",
    ] {
        assert_eq!(agent_name_from_path_token(path), None);
    }
    assert_eq!(
        agent_name_from_path_token(r"C:\bin\codex.exe"),
        Some("codex".into())
    );
}
