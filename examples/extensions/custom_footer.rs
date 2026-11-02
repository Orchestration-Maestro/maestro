//! A command that toggles a footer showing history usage, the model and Git branch.
// maestro-component: MAESTRO_CUSTOM_FOOTER_COMPONENT
#![cfg(any(test, target_arch = "wasm32"))]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::{cell::Cell, rc::Rc};

use maestro_extensions_wasm::utils::TruncateOptions;
use maestro_extensions_wasm::{
    AgentMessage, CallbackEmitter, CommandOptions, Component, ComponentView, ExtensionAPI,
    ExtensionCommandContext, ExtensionComponent, ExtensionFuture, ExtensionResult, FooterData,
    FooterFactory, Message, NotificationType, SessionEntry, Theme, truncate_to_width,
    visible_width,
};

/// The host capabilities the footer reads on each render.
struct Footer {
    /// Host-backed terminal text helpers.
    api: ExtensionAPI,
    /// The command context whose current history and model are displayed.
    context: ExtensionCommandContext,
    /// The live theme lent to the factory.
    theme: Theme,
    /// The current branch metadata.
    data: FooterData,
}

/// Formats token counts with one decimal in thousands from 1,000 upward.
fn tokens(count: f64) -> String {
    if count < 1000.0 {
        ryu_js::Buffer::new().format(count).to_owned()
    } else {
        format!(
            "{}k",
            ryu_js::Buffer::new().format_to_fixed(count / 1000.0, 1)
        )
    }
}

impl ComponentView for Footer {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let (mut input, mut output, mut cost) = (0.0, 0.0, 0.0);
        for entry in self.context.session_manager()?.get_branch(None)? {
            if let SessionEntry::Message(entry) = entry
                && let AgentMessage::Message(message) = entry.message
                && let Message::Assistant(message) = *message
            {
                input += message.usage.input;
                output += message.usage.output;
                cost += message.usage.cost.total;
            }
        }
        let branch = self
            .data
            .get_git_branch()?
            .filter(|branch| !branch.is_empty())
            .map_or_else(String::new, |branch| format!(" ({branch})"));
        let cost = ryu_js::Buffer::new().format_to_fixed(cost, 3).to_owned();
        let left = self.theme.fg(
            "dim",
            &format!("↑{} ↓{} ${cost}", tokens(input), tokens(output)),
        )?;
        let model = self
            .context
            .model()?
            .map(|model| model.id)
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| "no-model".to_owned());
        let right = self.theme.fg("dim", &format!("{model}{branch}"))?;
        let used = visible_width(&self.api, &left)? + visible_width(&self.api, &right)?;
        let padding = " ".repeat((width as usize).saturating_sub(used).max(1));
        Ok(vec![truncate_to_width(
            &self.api,
            &format!("{left}{padding}{right}"),
            width as usize,
            TruncateOptions::default(),
        )?])
    }

    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// Builds a footer whose branch listener requests redraws until disposal unsubscribes it.
fn footer(api: ExtensionAPI, context: ExtensionCommandContext) -> FooterFactory {
    Rc::new(move |tui, theme, data, _| {
        let subscription = data.on_branch_change(Rc::new(move || tui.request_render(false)))?;
        Ok(ExtensionComponent {
            component: Component::new(Footer {
                api: api.clone(),
                context: context.clone(),
                theme,
                data,
            }),
            dispose: Some(Rc::new(move |_| subscription.unsubscribe())),
            set_expanded: None,
            width: None,
        })
    })
}

/// Applies the selected footer state and reports it through the command's UI.
fn set_footer(
    helpers: ExtensionAPI,
    context: ExtensionCommandContext,
    enabled: bool,
) -> ExtensionResult<()> {
    let ui = context.ui()?;
    if enabled {
        ui.set_footer(Some(footer(helpers, context)))?;
        ui.notify("Custom footer enabled", Some(NotificationType::Info))
    } else {
        ui.set_footer(None)?;
        ui.notify("Default footer restored", Some(NotificationType::Info))
    }
}

/// Registers the footer toggle command.
///
/// # Errors
/// Returns the host's failure when it rejects command registration.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    let enabled = Cell::new(false);
    let helpers = api.clone();
    Box::pin(async move {
        api.register_command(
            "footer",
            CommandOptions {
                description: Some("Toggle custom footer".to_owned()),
                get_argument_completions: None,
                handler: Rc::new(move |_, context| {
                    let now = !enabled.get();
                    enabled.set(now);
                    let helpers = helpers.clone();
                    Box::pin(async move { set_footer(helpers, context, now) })
                }),
            },
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
