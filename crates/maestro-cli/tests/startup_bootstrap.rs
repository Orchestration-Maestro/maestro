//! Startup mode selection, diagnostics and session-directory precedence.

use crate::support::startup;
use maestro_app::{
    AgentSessionRuntimeDiagnostic, AgentSessionRuntimeDiagnosticType, SettingsManager,
};
use maestro_cli::input::args::{Args, Diagnostic, DiagnosticType, Mode};
use maestro_cli::startup::main::bootstrap::{
    AppMode, collect_settings_diagnostics, report_diagnostics, report_parser_diagnostics,
    resolve_app_mode, resolve_session_dir, takes_over_stdout, to_print_output_mode,
    validate_fork_flags,
};
use maestro_test_directory::Scratch;
use maestro_theme::Theme;
use serde_json::json;

/// The status startup ends with for a fatal report.
const FAILURE: Option<i32> = Some(1);

/// The mode each combination of mode, print request and stdin kind selects.
const MODE_CASES: [(Option<Mode>, Option<bool>, bool, AppMode); 16] = [
    (None, None, true, AppMode::Interactive),
    (None, None, false, AppMode::Print),
    (None, Some(true), true, AppMode::Print),
    (None, Some(true), false, AppMode::Print),
    (Some(Mode::Text), None, true, AppMode::Interactive),
    (Some(Mode::Text), None, false, AppMode::Print),
    (Some(Mode::Text), Some(true), true, AppMode::Print),
    (Some(Mode::Text), Some(true), false, AppMode::Print),
    (Some(Mode::Json), None, true, AppMode::Json),
    (Some(Mode::Json), None, false, AppMode::Json),
    (Some(Mode::Json), Some(true), true, AppMode::Json),
    (Some(Mode::Json), Some(true), false, AppMode::Json),
    (Some(Mode::Rpc), None, true, AppMode::Rpc),
    (Some(Mode::Rpc), None, false, AppMode::Rpc),
    (Some(Mode::Rpc), Some(true), true, AppMode::Rpc),
    (Some(Mode::Rpc), Some(true), false, AppMode::Rpc),
];

#[test]
fn mode_selection_reads_stdin_not_stdout() {
    for (mode, print, stdin_is_tty, expected) in MODE_CASES {
        let parsed = Args {
            mode,
            print,
            ..Args::default()
        };
        let selected = resolve_app_mode(&parsed, stdin_is_tty);
        assert_eq!(
            selected, expected,
            "mode {mode:?}, print {print:?}, tty {stdin_is_tty}"
        );
        assert_eq!(
            takes_over_stdout(selected),
            selected != AppMode::Interactive,
            "takeover for {selected:?}"
        );
        assert_eq!(
            to_print_output_mode(selected),
            if selected == AppMode::Json {
                Mode::Json
            } else {
                Mode::Text
            },
            "print output mode for {selected:?}"
        );
    }
}

#[test]
fn diagnostics_keep_context_styles_and_drain() {
    let scratch = Scratch::new().unwrap();
    let agent_dir = scratch.path().join("agent");
    std::fs::create_dir_all(&agent_dir).unwrap();
    std::fs::write(agent_dir.join("settings.json"), "{").unwrap();
    let cwd = scratch.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let mut settings = SettingsManager::create(
        cwd.to_str().unwrap(),
        agent_dir.to_str().unwrap(),
        ".maestro",
    );

    let drained = collect_settings_diagnostics(&mut settings, "startup session lookup");
    assert_eq!(drained.len(), 1);
    assert_eq!(
        drained[0].r#type,
        AgentSessionRuntimeDiagnosticType::Warning
    );
    assert!(
        drained[0]
            .message
            .starts_with("(startup session lookup, global settings) "),
        "{}",
        drained[0].message
    );
    assert!(
        collect_settings_diagnostics(&mut settings, "startup session lookup").is_empty(),
        "a second drain reports nothing"
    );

    let severity = [
        AgentSessionRuntimeDiagnostic {
            r#type: AgentSessionRuntimeDiagnosticType::Info,
            message: "note".to_owned(),
        },
        AgentSessionRuntimeDiagnostic {
            r#type: AgentSessionRuntimeDiagnosticType::Warning,
            message: "careful".to_owned(),
        },
        AgentSessionRuntimeDiagnostic {
            r#type: AgentSessionRuntimeDiagnosticType::Error,
            message: "broken".to_owned(),
        },
    ];
    let mut plain = Vec::new();
    report_diagnostics(&severity, &mut plain, false).unwrap();
    assert_eq!(
        String::from_utf8(plain).unwrap(),
        "note\nWarning: careful\nError: broken\n"
    );
    let mut styled = Vec::new();
    report_diagnostics(&severity, &mut styled, true).unwrap();
    assert_eq!(
        String::from_utf8(styled).unwrap(),
        format!(
            "{}\n{}\n{}\n",
            Theme::dim("note", true),
            Theme::yellow("Warning: careful", true),
            Theme::red("Error: broken", true)
        )
    );
}

#[test]
fn settings_failures_name_their_scope_in_loading_order() {
    let scratch = Scratch::new().unwrap();
    let agent_dir = scratch.path().join("agent");
    std::fs::create_dir_all(&agent_dir).unwrap();
    std::fs::write(agent_dir.join("settings.json"), "{").unwrap();
    let cwd = scratch.path().join("project");
    std::fs::create_dir_all(cwd.join(".maestro")).unwrap();
    std::fs::write(cwd.join(".maestro").join("settings.json"), "[").unwrap();
    let mut settings = SettingsManager::create(
        cwd.to_str().unwrap(),
        agent_dir.to_str().unwrap(),
        ".maestro",
    );

    let drained = collect_settings_diagnostics(&mut settings, "startup");
    assert_eq!(
        drained
            .iter()
            .map(|diagnostic| (
                diagnostic.r#type,
                diagnostic.message.split(')').next().unwrap()
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                AgentSessionRuntimeDiagnosticType::Warning,
                "(startup, global settings"
            ),
            (
                AgentSessionRuntimeDiagnosticType::Warning,
                "(startup, project settings"
            ),
        ],
        "each failure names its own scope, global before project"
    );
    assert!(collect_settings_diagnostics(&mut settings, "startup").is_empty());
}

#[test]
fn parser_errors_end_before_runtime_creation() {
    let warning = || Diagnostic {
        r#type: DiagnosticType::Warning,
        message: "unknown flag".to_owned(),
    };
    let error = || Diagnostic {
        r#type: DiagnosticType::Error,
        message: "bad value".to_owned(),
    };

    let mut ordered = Vec::new();
    assert_eq!(
        report_parser_diagnostics(&[warning(), error()], &mut ordered, false).unwrap(),
        FAILURE
    );
    assert_eq!(
        String::from_utf8(ordered).unwrap(),
        "Warning: unknown flag\nError: bad value\n"
    );

    let mut accepted = Vec::new();
    assert_eq!(
        report_parser_diagnostics(&[warning()], &mut accepted, false).unwrap(),
        None
    );
    assert_eq!(
        String::from_utf8(accepted).unwrap(),
        "Warning: unknown flag\n"
    );

    let mut styled = Vec::new();
    report_parser_diagnostics(&[warning(), error()], &mut styled, true).unwrap();
    assert_eq!(
        String::from_utf8(styled).unwrap(),
        format!(
            "{}\n{}\n",
            Theme::yellow("Warning: unknown flag", true),
            Theme::red("Error: bad value", true)
        )
    );
}

#[test]
fn fork_conflicts_preserve_flag_order() {
    let flags = [
        ("--session", Some("session")),
        ("--continue", None),
        ("--resume", None),
        ("--no-session", None),
    ];
    let mut observed = 0;
    for subset in 1_u8..16 {
        let mut tokens = vec!["--fork", "source"];
        let mut expected: Vec<&str> = Vec::new();
        for (index, (flag, value)) in flags.iter().enumerate() {
            if subset & (1 << index) == 0 {
                continue;
            }
            tokens.push(flag);
            if let Some(value) = value {
                tokens.push(value);
            }
            expected.push(flag);
        }
        let parsed = startup::args(&tokens);
        let mut stderr = Vec::new();
        assert_eq!(
            validate_fork_flags(&parsed, &mut stderr, false).unwrap(),
            FAILURE,
            "{tokens:?}"
        );
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            format!(
                "Error: --fork cannot be combined with {}\n",
                expected.join(", ")
            ),
            "{tokens:?}"
        );
        observed += 1;
    }
    assert_eq!(observed, 15);

    for tokens in [
        vec!["--session", "session"],
        vec!["--fork", "source"],
        vec!["--fork", ""],
    ] {
        let parsed = startup::args(&tokens);
        let mut stderr = Vec::new();
        assert_eq!(
            validate_fork_flags(&parsed, &mut stderr, false).unwrap(),
            None,
            "{tokens:?}"
        );
        assert!(stderr.is_empty(), "{tokens:?}");
    }
}

#[test]
fn session_directory_keeps_empty_cli_and_precedence() {
    let settings = startup::settings(&[("sessionDir", json!("/stored/sessions"))]);
    let home = || Ok("/home/tester".to_owned());

    let explicit = Args {
        session_dir: Some(String::new()),
        ..Args::default()
    };
    assert_eq!(
        resolve_session_dir(&explicit, Some("/env/sessions"), &settings, home).unwrap(),
        Some(String::new()),
        "an authored argument wins even when empty"
    );

    let authored = Args {
        session_dir: Some("/cli/sessions".to_owned()),
        ..Args::default()
    };
    assert_eq!(
        resolve_session_dir(&authored, Some("/env/sessions"), &settings, home).unwrap(),
        Some("/cli/sessions".to_owned())
    );

    assert_eq!(
        resolve_session_dir(&Args::default(), Some("~/sessions"), &settings, home).unwrap(),
        Some("/home/tester/sessions".to_owned()),
        "the environment value is expanded"
    );
    assert_eq!(
        resolve_session_dir(&Args::default(), Some(""), &settings, home).unwrap(),
        Some("/stored/sessions".to_owned()),
        "an empty environment value falls through"
    );
    assert_eq!(
        resolve_session_dir(&Args::default(), None, &settings, home).unwrap(),
        Some("/stored/sessions".to_owned())
    );
    let unset = startup::settings(&[]);
    assert_eq!(
        resolve_session_dir(&Args::default(), None, &unset, home).unwrap(),
        None
    );
}
