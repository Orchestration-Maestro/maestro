//! Disabled and failed initialization leave both original bash routes available.
use super::*;

/// Starts a supplied project configuration through the production composition.
async fn configured(
    value: serde_json::Value,
    local: Rc<tokio::task::LocalSet>,
) -> (
    super::super::super::support::Root,
    Rc<maestro_app::AgentSession>,
) {
    let root = super::super::super::support::Root::new();
    std::fs::create_dir(root.path("project/.maestro")).unwrap();
    std::fs::write(
        root.path("project/.maestro/sandbox.json"),
        value.to_string(),
    )
    .unwrap();
    let session = super::super::loaded(&root, local).await;
    (root, session)
}

/// Installs a visible UI before the next explicitly selected lifecycle event.
fn visible(runner: &ExtensionRunner<Local>) -> Rc<Ui> {
    let ui = Rc::new(Ui::default());
    runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
    ui
}

/// Checks that the existing native backend, not a partially initialized wrapper, executes.
async fn unwrapped(runner: &Rc<ExtensionRunner<Local>>, ui: &Ui) {
    assert_eq!(
        bash(runner, "printf '%s' \"${SANDBOX_RUNTIME-unwrapped}\"").await,
        "unwrapped"
    );
    assert!(ui.status.borrow().is_none());
    let command = runner.get_command("sandbox").unwrap().command;
    (command.handler)(String::new(), runner.create_command_context())
        .await
        .unwrap();
    assert_eq!(ui.messages.borrow().last().unwrap(), "Sandbox is disabled");
    event(runner, false).await;
}

#[test]
fn sandbox_flag_disables_the_wrapper_before_reading_invalid_configuration() {
    if isolated("sandbox_flag_disables_the_wrapper_before_reading_invalid_configuration") {
        return;
    }
    super::super::run(|local| async move {
        let (_root, session) = configured(json!({"network":{"allowedDomains":3}}), local).await;
        let runner = session.extension_runner();
        runner.set_flag_value("no-sandbox".into(), maestro_app::FlagValue::Boolean(true));
        let ui = visible(&runner);
        event(&runner, true).await;
        assert_eq!(
            &*ui.messages.borrow(),
            &["Sandbox disabled via --no-sandbox"]
        );
        assert!(matches!(
            ui.kinds.borrow()[0],
            Some(NotificationType::Warning)
        ));
        unwrapped(&runner, &ui).await;
    });
}

#[test]
fn sandbox_falsy_configuration_keeps_native_bash_available() {
    if isolated("sandbox_falsy_configuration_keeps_native_bash_available") {
        return;
    }
    super::super::run(|local| async move {
        let (_root, session) = configured(json!({"enabled":0}), local).await;
        let runner = session.extension_runner();
        let ui = visible(&runner);
        event(&runner, true).await;
        assert_eq!(&*ui.messages.borrow(), &["Sandbox disabled via config"]);
        assert!(matches!(ui.kinds.borrow()[0], Some(NotificationType::Info)));
        unwrapped(&runner, &ui).await;
    });
}

#[test]
fn sandbox_initialization_failure_reports_error_without_claiming_protection() {
    if isolated("sandbox_initialization_failure_reports_error_without_claiming_protection") {
        return;
    }
    super::super::run(|local| async move {
        let (_root, session) = configured(json!({"filesystem":{"allowWrite":3}}), local).await;
        let runner = session.extension_runner();
        let ui = visible(&runner);
        event(&runner, true).await;
        assert!(ui.messages.borrow()[0].starts_with("Sandbox initialization failed:"));
        assert!(matches!(
            ui.kinds.borrow()[0],
            Some(NotificationType::Error)
        ));
        unwrapped(&runner, &ui).await;
    });
}
