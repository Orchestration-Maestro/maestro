//! Lists the host's slash commands through an opt-in `/commands` selector.
// maestro-component: MAESTRO_COMMANDS_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    AutocompleteItem, CommandOptions, ExtensionAPI, ExtensionCommandContext, ExtensionFuture,
    ExtensionResult, ExtensionUIContext, NotificationType, SlashCommandInfo,
};

/// Source labels in selector and completion order.
const SOURCES: [(&str, &str); 3] = [
    ("extension", "Extensions"),
    ("prompt", "Prompts"),
    ("skill", "Skills"),
];

/// Shows the current inventory, optionally restricted to one literal source.
///
/// # Errors
/// Returns the host's inventory failure, a UI lookup, notification or selection
/// failure, or the failure of `show_path`.
async fn list(
    api: ExtensionAPI,
    args: String,
    ctx: ExtensionCommandContext,
) -> ExtensionResult<()> {
    let commands = api.get_commands()?;
    let filter = args.trim();
    let filtered: Vec<_> = commands
        .iter()
        .filter(|command| filter.is_empty() || command.source.0 == filter)
        .collect();
    let ui = ctx.ui()?;
    if filtered.is_empty() {
        let message = if filter.is_empty() {
            "No commands found".into()
        } else {
            format!("No {filter} commands found")
        };
        return ui.notify(&message, Some(NotificationType::Info));
    }
    let mut items = Vec::new();
    for (source, label) in SOURCES {
        let group: Vec<_> = filtered
            .iter()
            .filter(|command| command.source.0 == source)
            .collect();
        if group.is_empty() {
            continue;
        }
        items.push(format!("--- {label} ---"));
        for command in group {
            let description = command
                .description
                .as_deref()
                .filter(|text| !text.is_empty());
            items.push(match description {
                Some(description) => format!("/{} - {description}", command.name),
                None => format!("/{}", command.name),
            });
        }
    }
    let selected = ui.select("Available Commands".into(), items, None).await?;
    show_path(&commands, selected, &ui).await
}

/// Offers the selected command's authored path without opening the source file.
///
/// # Errors
/// Returns the confirmation or notification failure.
async fn show_path(
    commands: &[SlashCommandInfo],
    selected: Option<String>,
    ui: &ExtensionUIContext,
) -> ExtensionResult<()> {
    let Some(selected) = selected.filter(|text| !text.is_empty() && !text.starts_with("---"))
    else {
        return Ok(());
    };
    let name = selected.split(" - ").next().unwrap_or_default();
    let name = name.strip_prefix('/').unwrap_or_default();
    let Some(command) = commands
        .iter()
        .find(|command| command.name == name)
        .filter(|command| !command.source_info.path.is_empty())
    else {
        return Ok(());
    };
    let path = &command.source_info.path;
    if ui
        .confirm(
            command.name.clone(),
            format!("View source path?\n{path}"),
            None,
        )
        .await?
    {
        ui.notify(path, Some(NotificationType::Info))?;
    }
    Ok(())
}

/// Registers the command inventory selector.
///
/// # Errors
/// The returned future returns the host's rejection when it refuses the command
/// registration.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let registered = api.clone();
        api.register_command(
            "commands",
            CommandOptions {
                description: Some("List available slash commands".into()),
                get_argument_completions: Some(Rc::new(|prefix| {
                    Box::pin(async move {
                        let items: Vec<_> = SOURCES
                            .into_iter()
                            .filter(|(source, _)| source.starts_with(&prefix))
                            .map(|(source, _)| AutocompleteItem {
                                value: source.into(),
                                label: source.into(),
                                description: None,
                            })
                            .collect();
                        Ok((!items.is_empty()).then_some(items))
                    })
                })),
                handler: Rc::new(move |args, ctx| Box::pin(list(registered.clone(), args, ctx))),
            },
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
