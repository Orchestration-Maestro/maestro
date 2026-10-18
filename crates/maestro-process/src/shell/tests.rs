//! Command resolution using supplied platform observations.
use super::resolve_windows_executable;
#[cfg(target_os = "linux")]
use crate::pty::foreground::{ForegroundJob, ForegroundProcess};
use std::path::{Path, PathBuf};

#[test]
fn extensionless_commands_follow_directory_and_extension_order() {
    let directories = [PathBuf::from("current"), PathBuf::from("bin")];
    let files = [
        Path::new("current").join("custom.CMD"),
        Path::new("current").join("custom.EXE"),
        Path::new("bin").join("custom.COM"),
    ];
    for (extensions, expected) in [
        (".CMD;.EXE;.COM", "custom.CMD"),
        (".EXE;.CMD;.COM", "custom.EXE"),
        (".COM;.CMD;.EXE", "custom.CMD"),
    ] {
        assert_eq!(
            resolve_windows_executable("custom", &directories, extensions, |p| files
                .contains(&p.to_path_buf())),
            Some(Path::new("current").join(expected))
        );
    }
    assert_eq!(
        resolve_windows_executable("missing", &directories, ".CMD;.EXE", |p| files
            .contains(&p.to_path_buf())),
        None
    );
}

#[test]
fn explicit_script_names_prefer_current_directory_over_path_decoys() {
    let directories = [PathBuf::from("current"), PathBuf::from("bin")];
    let current = Path::new("current").join("wrapper.cmd");
    let decoy = Path::new("bin").join("wrapper.cmd");
    assert_eq!(
        resolve_windows_executable("wrapper.cmd", &directories, ".EXE", |p| {
            p == current || p == decoy
        }),
        Some(current)
    );
}

#[test]
fn explicit_command_directories_do_not_search_other_directories() {
    let command = Path::new("chosen dir").join("custom");
    let script = Path::new("chosen dir").join("custom.CMD");
    assert_eq!(
        resolve_windows_executable(
            command.to_str().unwrap(),
            &[PathBuf::from("other")],
            ".EXE;;.CMD",
            |p| p == script
        ),
        Some(script)
    );
}

#[test]
#[cfg(windows)]
fn verbatim_shell_commands_keep_environment_operands() {
    let output = super::windows_shell_launch("echo \"%MAESTRO_LITERAL_FILE%\"", None)
        .command()
        .unwrap()
        .env(
            "MAESTRO_LITERAL_FILE",
            "directory with spaces & literal.txt",
        )
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        output.stdout,
        b"\"directory with spaces & literal.txt\"\r\n"
    );
}

#[test]
fn shell_literal_operands_share_one_posix_quoter() {
    for (input, expected) in [
        ("", "''"),
        ("a,b", "a,b"),
        ("@%_+=:,./-Az09", "@%_+=:,./-Az09"),
        ("a'b", "'a'\\''b'"),
        ("a\n b", "'a\n b'"),
        ("é;$\"", "'é;$\"'"),
    ] {
        assert_eq!(super::shell_quote(input), expected);
        #[cfg(unix)]
        {
            let output = std::process::Command::new("/bin/sh")
                .args(["-c", &format!("printf %s {}", super::shell_quote(input))])
                .output()
                .unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, input.as_bytes());
        }
    }
}

#[test]
fn interactive_argv_uses_the_selected_shell_quoter() {
    assert_eq!(super::quote_powershell_literal("safe.png"), "'safe.png'");
    assert_eq!(super::quote_powershell_literal("a'b"), "'a''b'");
    assert_eq!(super::quote_powershell_arg("safe.png"), "safe.png");
    let args = ["echo", "", "a,b", "a'b", "a b"].map(str::to_owned);
    assert_eq!(super::interactive_shell_command(&[], "sh"), None);
    assert_eq!(
        super::interactive_shell_command(&args, "sh").as_deref(),
        Some("echo '' a,b 'a'\\''b' 'a b'")
    );
    assert_eq!(
        super::interactive_shell_command(&args, "/bin/-PwSh.exe").as_deref(),
        Some("echo '' 'a,b' 'a''b' 'a b'")
    );
    assert_eq!(super::quote_powershell_arg("_-/.:+=Az09"), "_-/.:+=Az09");
    assert_eq!(super::quote_powershell_arg("é\n\"$"), "'é\n\"$'");
}

#[test]
fn pane_shell_names_exclude_replaced_programs() {
    for name in [
        "sh",
        "bash",
        "dash",
        "zsh",
        "fish",
        "ksh",
        "mksh",
        "csh",
        "tcsh",
        "elvish",
        "xonsh",
        "nu",
        "pwsh",
        "powershell",
        "cmd",
        "/bin/--BASH.exe.exe",
        r"C:\bin\-PwSh.exe",
    ] {
        assert!(super::is_pane_shell_process_name(name));
    }
    for name in ["vim", "cargo", "python", "bash.EXE", "bash-", "bash.exe-"] {
        assert!(!super::is_pane_shell_process_name(name));
    }
}

#[test]
#[cfg(unix)]
fn detached_custom_commands_keep_the_login_flag() {
    let command = super::detached_custom_command_process("printf 'a,b'");
    assert_eq!(command.get_program(), "/bin/sh");
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        ["-lc", "printf 'a,b'"]
    );
    assert_eq!(
        super::command("literal program").get_program(),
        "literal program"
    );
}

#[test]
#[cfg(target_os = "linux")]
fn scrollback_editor_keeps_shell_evaluated_editor_fallback() {
    let dir = std::env::temp_dir().join(format!("maestro-editor-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editor.sh");
    let status = std::process::Command::new("/usr/bin/install")
        .args(["-m", "0700"])
        .arg(fixture)
        .arg(dir.join("vi"))
        .status()
        .unwrap();
    assert!(status.success());
    for editor in [None, Some(""), Some("vi --custom")] {
        let path = dir.join("scroll back ' a,b");
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let argv = super::scrollback_editor_argv(&path).unwrap();
        assert_eq!(&argv[..2], ["/bin/sh", "-c"]);
        assert_eq!(
            argv[2],
            format!(
                r#"scrollback_file='{}'; eval "${{EDITOR:-vi}} \"\$scrollback_file\""; status=$?; rm -f "$scrollback_file"; exit $status"#,
                path.display().to_string().replace('\'', "'\\''")
            )
        );
        let mut command = std::process::Command::new(&argv[0]);
        command
            .args(&argv[1..])
            .env("PATH", format!("{}:/usr/bin:/bin", dir.display()))
            .env("MAESTRO_EDITOR_RESULT", dir.join("result"));
        if let Some(editor) = editor {
            command.env("EDITOR", editor);
        } else {
            command.env_remove("EDITOR");
        }
        assert_eq!(command.status().unwrap().code(), Some(7));
        assert!(!path.exists());
        let observed = std::fs::read_to_string(dir.join("result")).unwrap();
        assert_eq!(observed, format!("{}\n", path.display()));
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[cfg(target_os = "linux")]
fn pane_custom_commands_keep_the_plain_command_flag() {
    let command = super::pane_custom_command_pty_builder("printf 'a,b'");
    assert_eq!(command.program, "/bin/sh");
    assert_eq!(command.args, ["-c", "printf 'a,b'"]);
    assert!(command.cwd.is_none());
    assert!(
        matches!(command.environment,crate::pty::PtyEnvironment::Inherit(edits) if edits.is_empty())
    );
    let shell = |pid, name: &str| ForegroundProcess {
        pid,
        name: name.into(),
        argv0: None,
        argv: None,
        cmdline: None,
    };
    assert_eq!(
        super::available_pane_shell_from_job(
            10,
            ForegroundJob {
                process_group_id: 10,
                processes: vec![shell(10, "-BASH")]
            }
        )
        .as_deref(),
        Some("-BASH")
    );
    for job in [
        ForegroundJob {
            process_group_id: 11,
            processes: vec![shell(10, "sh")],
        },
        ForegroundJob {
            process_group_id: 10,
            processes: vec![shell(10, "sh"), shell(11, "sh")],
        },
        ForegroundJob {
            process_group_id: 10,
            processes: vec![shell(10, "vim")],
        },
        ForegroundJob {
            process_group_id: 10,
            processes: vec![],
        },
    ] {
        assert!(super::available_pane_shell_from_job(10, job).is_none());
    }
}
