//! Interactive user commands through the shipped component and real extension runner.
#![cfg(test)]
use maestro_extensions::{
    ExtensionRunner, Local, create_extension_runtime, load_extension_from_factory,
};
use std::{rc::Rc, sync::RwLock};

/// Loads the shipped component without a UI or provider.
async fn loaded() -> ExtensionRunner<Local> {
    let path = std::env::var("MAESTRO_INTERACTIVE_SHELL_COMPONENT").unwrap();
    let bus = super::components::TracedBus::new();
    let runtime = create_extension_runtime::<Local>();
    let owner = load_extension_from_factory(
        crate::artifact(path.clone()),
        "/session".into(),
        bus.controller.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session("/session").0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    )
}

/// Dispatches a literal user shell event without executing a model request.
async fn command(
    runner: &ExtensionRunner<Local>,
    command: &str,
) -> Option<maestro_extensions::BashResult> {
    let mut event = maestro_extensions::UserBashEvent {
        command: command.into(),
        cwd: "/event-directory".into(),
        exclude_from_context: false,
    };
    runner.emit_user_bash::<Rc<dyn maestro_extensions::BashOperations<Local>>, maestro_extensions::BashResult>(&mut event)
        .await.unwrap().map(|result| {
            assert!(result.operations.is_none());
            match result.result { maestro_extensions::Presence::Present(value) => value, _ => panic!("missing result") }
        })
}

/// Detects defaults and force prefixes, without intercepting ordinary commands or name prefixes.
#[test]
fn interactive_shell_headless_detection() {
    if isolated("interactive_shell_headless_detection", &[]) {
        return;
    }
    super::wasi_support::run(async {
        let runner = loaded().await;
        for text in [
            "vim file",
            " NVIM\tfile ",
            "git add -p",
            "cat file | less",
            "i echo forced",
            "i\techo forced",
        ] {
            let result = command(&runner, text).await.expect(text);
            assert_eq!(result.output, "(interactive commands require TUI)");
            assert_eq!(result.exit_code, Some(1.0));
            assert!(!result.cancelled && !result.truncated);
        }
        for text in [
            "echo ordinary",
            "vimdiff file",
            "git status",
            "printf less",
            "cat | lessish",
            "i",
        ] {
            assert!(command(&runner, text).await.is_none(), "{text}");
        }
    });
}

/// Isolates environment configuration without changing another test's process state.
fn isolated(name: &str, environment: &[(&str, &str)]) -> bool {
    if std::env::var_os("MAESTRO_INTERACTIVE_CHILD").is_some() {
        return false;
    }
    let component = std::env::var("MAESTRO_INTERACTIVE_SHELL_COMPONENT").unwrap();
    let mut child = maestro_test_process::TestProcess::current(name)
        .unwrap()
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("MAESTRO_INTERACTIVE_CHILD", "1")
        .env("MAESTRO_INTERACTIVE_SHELL_COMPONENT", component);
    for (key, value) in environment {
        child = child.env(key, value);
    }
    let output = child.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    true
}

/// Includes configured prefixes, excludes them case-insensitively and keeps force available.
#[test]
fn interactive_shell_configured_prefixes() {
    if isolated(
        "interactive_shell_configured_prefixes",
        &[
            (
                "INTERACTIVE_COMMANDS",
                " Custom Tool , custom-editor, ,vim ",
            ),
            ("INTERACTIVE_EXCLUDE", " VIM , CUSTOM-EDITOR "),
        ],
    ) {
        return;
    }
    super::wasi_support::run(async {
        let runner = loaded().await;
        for text in [
            "Custom Tool file",
            "cat | custom tool file",
            "i vim file",
            "i\tcustom-editor file",
            "nvim file",
        ] {
            assert!(command(&runner, text).await.is_some(), "{text}");
        }
        for text in [
            "vim file",
            "custom-editor file",
            "custom toolbox",
            "echo ordinary",
        ] {
            assert!(command(&runner, text).await.is_none(), "{text}");
        }
    });
}
