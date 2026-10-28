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
