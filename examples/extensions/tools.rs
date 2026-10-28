//! Interactive tool selection retained in the current session branch.
// Tests receive this component's path in MAESTRO_TOOLS_COMPONENT.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use indexmap::IndexSet;
use maestro_extensions_wasm::{
    CallbackEmitter, CommandOptions, Component, ComponentView, CustomFactoryOutcome, ExtensionAPI,
    ExtensionCommandContext, ExtensionComponent, ExtensionContext, ExtensionFuture,
    ExtensionResult, SessionEntry, SettingItem, SettingsList, SettingsListOptions, TUI, Theme,
    get_settings_list_theme,
};
use std::{cell::RefCell, rc::Rc};

/// Ordered selection stored in a custom session entry.
#[derive(Default)]
struct ToolsState {
    /// Tools enabled in their selection order.
    enabled_tools: IndexSet<String>,
}

/// Restores the newest configuration in the current branch, excluding removed tools.
fn restore(
    api: &ExtensionAPI,
    ctx: &ExtensionContext,
    state: &RefCell<ToolsState>,
) -> ExtensionResult<()> {
    let inventory = api.get_all_tools()?;
    let mut saved = None;
    for entry in ctx.session_manager()?.get_branch(None)? {
        if let SessionEntry::Custom(entry) = entry
            && entry.custom_type == "tools-config"
            && let Some(data) = entry.data.present()
        {
            let data: serde_json::Value =
                serde_json::from_str(data).map_err(|error| error.to_string())?;
            if let Some(enabled) = data.get("enabledTools") {
                saved = Some(
                    serde_json::from_value::<IndexSet<String>>(enabled.clone())
                        .map_err(|error| error.to_string())?,
                );
            }
        }
    }
    state.borrow_mut().enabled_tools = if let Some(mut names) = saved {
        names.retain(|name| inventory.iter().any(|tool| tool.name == *name));
        api.set_active_tools(&names.iter().cloned().collect::<Vec<_>>())?;
        names
    } else {
        api.get_active_tools()?.into_iter().collect()
    };
    Ok(())
}

/// The host-owned settings list beneath this example's heading.
struct ToolSelector {
    /// Settings and input handling retained by the host.
    settings: SettingsList,
    /// Theme supplied by the custom-view factory.
    theme: Theme,
    /// Frame writer to redraw after each input.
    tui: TUI,
}
impl ComponentView for ToolSelector {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let title = self
            .theme
            .fg("accent", &self.theme.bold("Tool Configuration")?)?;
        let mut rows = vec![title, String::new()];
        rows.extend(self.settings.render(width)?);
        Ok(rows)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.settings.invalidate()
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.settings.handle_input(data)?;
        self.tui.request_render(false)
    }
}

/// Changes the ordered selection, applies it immediately and then persists it.
fn change(
    api: &ExtensionAPI,
    state: &RefCell<ToolsState>,
    id: &str,
    value: &str,
) -> ExtensionResult<()> {
    let names: Vec<_> = {
        let mut state = state.borrow_mut();
        if value == "enabled" {
            state.enabled_tools.insert(id.to_owned());
        } else {
            state.enabled_tools.shift_remove(id);
        }
        state.enabled_tools.iter().cloned().collect()
    };
    api.set_active_tools(&names)?;
    api.append_entry(
        "tools-config",
        Some(serde_json::json!({"enabledTools":names})),
    )
}

/// Refreshes the inventory and opens a settings view until its cancellation completes it.
async fn select_tools(
    api: ExtensionAPI,
    state: Rc<RefCell<ToolsState>>,
    ctx: ExtensionCommandContext,
) -> ExtensionResult<()> {
    let items: Vec<_> = api
        .get_all_tools()?
        .into_iter()
        .map(|tool| SettingItem {
            current_value: if state.borrow().enabled_tools.contains(&tool.name) {
                "enabled"
            } else {
                "disabled"
            }
            .into(),
            id: tool.name.clone(),
            label: tool.name,
            description: None,
            values: Some(vec!["enabled".into(), "disabled".into()]),
            submenu: None,
        })
        .collect();
    ctx.ui()?
        .custom(
            Rc::new(move |tui, theme, _, done, _| {
                let api = api.clone();
                let state = state.clone();
                let settings = SettingsList::new(
                    items.clone(),
                    u32::try_from(items.len().saturating_add(2).min(15))
                        .map_err(|error| error.to_string())?,
                    &get_settings_list_theme()?,
                    (
                        Rc::new(move |id, value, _| change(&api, &state, id, value)),
                        Rc::new(move |_| done(())),
                    ),
                    SettingsListOptions::default(),
                )?;
                Ok(CustomFactoryOutcome::Ready(ExtensionComponent {
                    component: Component::new(ToolSelector {
                        settings,
                        theme,
                        tui,
                    }),
                    dispose: None,
                    set_expanded: None,
                    width: None,
                }))
            }),
            None,
        )
        .await?;
    Ok(())
}

/// Registers the tool selector and branch restoration handlers.
///
/// # Errors
/// Returns a host rejection unchanged.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let state = Rc::new(RefCell::new(ToolsState::default()));
        let selected = state.clone();
        let registered = api.clone();
        api.register_command(
            "tools",
            CommandOptions {
                description: Some("Enable/disable tools".into()),
                get_argument_completions: None,
                handler: Rc::new(move |_, ctx| {
                    Box::pin(select_tools(registered.clone(), selected.clone(), ctx))
                }),
            },
        )?;
        for event in ["session_start", "session_tree"] {
            let api_for_restore = api.clone();
            let state = state.clone();
            api.on(
                event,
                Rc::new(move |_, ctx| {
                    let result = restore(&api_for_restore, &ctx, &state);
                    Box::pin(std::future::ready(result.map(|()| None)))
                }),
            )?;
        }
        Ok(())
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
