//! The shipped tool selector changes the real application's active tools and session entries.
use maestro_app::{
    CustomComponentFactory, CustomFactoryOutcome, CustomUIOptions, EventError, ExtensionBindings,
    ExtensionUIContext, ExtensionUIContextPort, ExtensionUIFuture, NavigateTreeOptions,
    PromptOptions, ReadonlySessionManager,
};
use std::{cell::Cell, rc::Rc};

#[path = "../../maestro-test-terminal/src/manual_runtime.rs"]
pub mod manual_runtime;
#[path = "../../maestro-test-terminal/src/recording_terminal.rs"]
pub mod recording_terminal;

/// An interactive host that delivers one toggle followed by Escape to the actual view.
struct Ui {
    /// Terminal supplied to the custom factory.
    tui: maestro_tui::TUI,
    /// Prepared theme used by the heading.
    theme: maestro_theme::LiveTheme,
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        self.theme.clone()
    }
    fn custom_component(
        &self,
        factory: CustomComponentFactory,
        _: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        let closed = Rc::new(Cell::new(false));
        let done = closed.clone();
        let keys = maestro_app::ExtensionKeybindings {
            manager: maestro_tui::KeybindingsManager::new(Vec::new(), Vec::new()),
            reload: None,
        };
        let built = factory.call((
            self.tui.clone(),
            self.theme(),
            keys,
            Rc::new(move || Ok(!done.replace(true))),
        ));
        Box::pin(async move {
            let built = match built? {
                CustomFactoryOutcome::Ready(built) => built,
                CustomFactoryOutcome::Async(built) => built.await?,
            };
            assert!(
                built
                    .component
                    .try_render(80)
                    .unwrap()
                    .iter()
                    .any(|row| row.contains("Tool Configuration"))
            );
            let input = built.component.input_handler().unwrap();
            for data in ["\r", "\x1b"] {
                input
                    .try_handle_input(data)
                    .map_err(|error| EventError::from(error.to_string()))?;
            }
            assert!(closed.get());
            Ok(())
        })
    }
}

/// Binds the production frame writer to the existing controlled terminal and clock.
fn ui_binding() -> ExtensionBindings {
    let terminal = recording_terminal::RecordingTerminal::new(80, 24);
    let tui = maestro_tui::TUI::new(
        terminal.handle(),
        manual_runtime::ManualRuntime::new().handle(),
        maestro_tui::TerminalImage::default(),
        Some(false),
    );
    let theme = maestro_theme::load_theme_from_path(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../maestro-theme/assets/theme/dark.json"
        ),
        Some(maestro_theme::ColorMode::Truecolor),
    )
    .unwrap();
    ExtensionBindings {
        ui_context: Some(
            ExtensionUIContext::new(Rc::new(Ui {
                tui,
                theme: maestro_theme::LiveTheme::from_theme(Rc::new(theme)),
            }))
            .into(),
        ),
        ..ExtensionBindings::default()
    }
}

#[test]
fn tools_component_changes_native_session_and_restores_branch() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let root = super::support::Root::new();
        let session = super::support::session::component_session(
            &root,
            local.clone(),
            std::env::var("MAESTRO_TOOLS_COMPONENT").expect("nextest author-component setup"),
            None,
            super::support::widgets(),
        )
        .await;
        exercise(session).await;
    });
}

/// Toggles through the registered command and reconstructs the real selected branch.
async fn exercise(session: Rc<maestro_app::AgentSession>) {
    session.bind_extensions(ui_binding()).await.unwrap();
    assert_eq!(
        session.get_active_tool_names(),
        ["read", "bash", "edit", "write"]
    );
    assert!(
        session.extension_runner().get_command("tools").is_some(),
        "the shipped selector is registered"
    );
    assert_eq!(session.get_all_tools()[0].name, "read");
    session
        .prompt("/tools", PromptOptions::default())
        .await
        .unwrap();
    assert_eq!(session.get_active_tool_names(), ["bash", "edit", "write"]);
    let first = session
        .session_manager()
        .borrow()
        .get_leaf_id()
        .unwrap()
        .to_owned();
    let entries = serde_json::to_value(session.session_manager().borrow().get_entries()).unwrap();
    assert_eq!(
        entries.as_array().unwrap().last().unwrap()["data"],
        serde_json::json!({"enabledTools":["bash", "edit", "write"]})
    );
    session
        .prompt("/tools", PromptOptions::default())
        .await
        .unwrap();
    assert_eq!(
        session.get_active_tool_names(),
        ["bash", "edit", "write", "read"]
    );
    session
        .navigate_tree(
            &first,
            NavigateTreeOptions {
                summarize: Some(false),
                ..NavigateTreeOptions::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(session.get_active_tool_names(), ["bash", "edit", "write"]);
    session.reload().await.unwrap();
    assert_eq!(session.get_active_tool_names(), ["bash", "edit", "write"]);
    session.dispose();
}
