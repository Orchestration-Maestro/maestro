//! Live domain bindings shared by component and typed-request witnesses.
#![cfg(test)]
use maestro_extensions::{
    ExtensionContextActions, ExtensionExecution, ExtensionRunner, Local, types::ProviderActions,
};
use std::rc::Rc;

/// Binds a prompt reader without replacing command-specific operations.
pub fn bind_prompt(runner: &ExtensionRunner<Local>, prompt: &'static str) {
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_system_prompt: Rc::new(move || Ok(prompt.into())),
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
}

/// The user bindings a test manager with a file reloads.
pub fn reloaded_bindings() -> maestro_tui::KeybindingsConfig {
    vec![(
        "tui.select.up".to_owned(),
        Some(maestro_tui::KeybindingKeys::Single("f2".to_owned())),
    )]
}

/// Toolkit widgets over `theme`, whose managers with a file reload [`reloaded_bindings`].
pub fn widgets_over(
    theme: maestro_theme::LiveTheme,
) -> Rc<dyn maestro_extensions::ExtensionWidgets> {
    let widgets = maestro_extensions::test_support::ToolkitWidgets::new(theme).unwrap();
    Rc::new(widgets.reloading(reloaded_bindings()))
}

/// Toolkit widgets over an unpublished live theme.
pub fn widgets() -> Rc<dyn maestro_extensions::ExtensionWidgets> {
    widgets_over(maestro_theme::LiveTheme::default())
}
