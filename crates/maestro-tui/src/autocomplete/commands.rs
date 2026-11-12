//! Command projection and argument routing.

use super::{AutocompleteItem, AutocompleteSuggestions, Command, CompletionError};
use crate::fuzzy::fuzzy_filter;
use crate::text::utils::is_whitespace_scalar;

/// The input form's command identity.
fn name(command: &Command) -> &str {
    match command {
        Command::SlashCommand(command) => &command.name,
        Command::AutocompleteItem(item) => &item.value,
    }
}

/// Project a command into its displayed candidate.
fn project(command: &Command) -> AutocompleteItem {
    let (hint, description) = match command {
        Command::SlashCommand(command) => (
            command.argument_hint.as_deref(),
            command.description.as_deref(),
        ),
        Command::AutocompleteItem(item) => (None, item.description.as_deref()),
    };
    let description = match (
        hint.filter(|s| !s.is_empty()),
        description.filter(|s| !s.is_empty()),
    ) {
        (Some(hint), Some(description)) => Some(format!("{hint} — {description}")),
        (Some(text), None) | (None, Some(text)) => Some(text.to_owned()),
        (None, None) => None,
    };
    AutocompleteItem {
        value: name(command).to_owned(),
        label: name(command).to_owned(),
        description,
    }
}

/// Complete names or await the first exact command's argument callback.
pub(super) async fn suggest(
    commands: &[Command],
    text: &str,
) -> Result<Option<AutocompleteSuggestions>, CompletionError> {
    let Some((command_name, arguments)) = text[1..].split_once(' ') else {
        let projected: Vec<_> = commands.iter().map(project).collect();
        let items = fuzzy_filter(&projected, &text[1..], |item| &item.value)
            .into_iter()
            .cloned()
            .collect();
        return Ok(nonempty(items, text));
    };
    let Some(Command::SlashCommand(command)) = commands
        .iter()
        .find(|command| name(command) == command_name)
    else {
        return Ok(None);
    };
    let Some(callback) = &command.get_argument_completions else {
        return Ok(None);
    };
    Ok(callback(arguments)
        .await?
        .and_then(|items| nonempty(items, arguments)))
}

/// An absent list differs from a list of candidates.
pub(super) fn nonempty(
    items: Vec<AutocompleteItem>,
    prefix: &str,
) -> Option<AutocompleteSuggestions> {
    (!items.is_empty()).then(|| AutocompleteSuggestions {
        items,
        prefix: prefix.to_owned(),
    })
}

/// Explicit Tab's slash rule is separate from natural path recognition.
pub(super) fn should_trigger(text: &str) -> bool {
    let text = text.trim_matches(is_whitespace_scalar);
    !text.starts_with('/') || text.contains(' ')
}
