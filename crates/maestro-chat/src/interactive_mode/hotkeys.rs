//! Keyboard help using the live display registry.
use super::InteractiveMode;
use std::{error::Error, fmt::Write, sync::PoisonError};

impl InteractiveMode {
    /// Appends the current keyboard help as a bordered transcript card.
    ///
    /// # Errors
    /// Returns a failure reading the heading's theme.
    pub fn handle_hotkeys_command(&self) -> Result<(), Box<dyn Error>> {
        self.append_markdown_panel("Keyboard Shortcuts", self.hotkeys_markdown())
    }

    /// The fixed tables followed by the current runner's selected shortcuts.
    pub(super) fn hotkeys_markdown(&self) -> String {
        let mut text = format!(
            "{}\n\n{}\n\n{}",
            navigation_keys(),
            editing_keys(cfg!(windows)),
            other_keys()
        );
        let session = self.runtime_host.session();
        let shortcuts = session
            .extension_runner()
            .get_shortcuts(&self.keybindings.get_effective_config());
        if !shortcuts.is_empty() {
            text.push_str("\n\n**Extensions**\n| Key | Action |\n|-----|--------|");
            for (key, shortcut) in shortcuts {
                let shortcut = shortcut.read().unwrap_or_else(PoisonError::into_inner);
                let description = shortcut
                    .description
                    .as_deref()
                    .unwrap_or(&shortcut.extension_path);
                let _ = write!(
                    text,
                    "\n| `{}` | {description} |",
                    capitalize_shortcut(&key)
                );
            }
        }
        text
    }
}

/// The navigation table in displayed order.
fn navigation_keys() -> String {
    let key = InteractiveMode::get_app_key_display;
    format!(
        r"**Navigation**
| Key | Action |
|-----|--------|
| `{}` / `{}` / `{}` / `{}` | Move cursor / browse history |
| `{}` / `{}` | Move by word |
| `{}` | Start of line |
| `{}` | End of line |
| `{}` | Jump forward to character |
| `{}` | Jump backward to character |
| `{}` / `{}` | Scroll by page |",
        key("tui.editor.cursorUp"),
        key("tui.editor.cursorDown"),
        key("tui.editor.cursorLeft"),
        key("tui.editor.cursorRight"),
        key("tui.editor.cursorWordLeft"),
        key("tui.editor.cursorWordRight"),
        key("tui.editor.cursorLineStart"),
        key("tui.editor.cursorLineEnd"),
        key("tui.editor.jumpForward"),
        key("tui.editor.jumpBackward"),
        key("tui.editor.pageUp"),
        key("tui.editor.pageDown"),
    )
}

/// The editing table in displayed order.
fn editing_keys(windows: bool) -> String {
    let key = InteractiveMode::get_app_key_display;
    let guidance = if windows {
        " (Ctrl+Enter on Windows Terminal)"
    } else {
        ""
    };
    format!(
        r"**Editing**
| Key | Action |
|-----|--------|
| `{}` | Send message |
| `{}` | New line{guidance} |
| `{}` | Delete word backwards |
| `{}` | Delete word forwards |
| `{}` | Delete to start of line |
| `{}` | Delete to end of line |
| `{}` | Paste the most-recently-deleted text |
| `{}` | Cycle through the deleted text after pasting |
| `{}` | Undo |",
        key("tui.input.submit"),
        key("tui.input.newLine"),
        key("tui.editor.deleteWordBackward"),
        key("tui.editor.deleteWordForward"),
        key("tui.editor.deleteToLineStart"),
        key("tui.editor.deleteToLineEnd"),
        key("tui.editor.yank"),
        key("tui.editor.yankPop"),
        key("tui.editor.undo"),
    )
}

/// The other table in displayed order.
fn other_keys() -> String {
    let key = InteractiveMode::get_app_key_display;
    format!(
        r"**Other**
| Key | Action |
|-----|--------|
| `{}` | Path completion / accept autocomplete |
| `{}` | Cancel autocomplete / abort streaming |
| `{}` | Clear editor (first) / exit (second) |
| `{}` | Exit (when editor is empty) |
| `{}` | Suspend to background |
| `{}` | Cycle thinking level |
| `{}` / `{}` | Cycle models |
| `{}` | Open model selector |
| `{}` | Toggle tool output expansion |
| `{}` | Toggle thinking block visibility |
| `{}` | Edit message in external editor |
| `{}` | Queue follow-up message |
| `{}` | Restore queued messages |
| `{}` | Paste image from clipboard |
| `/` | Slash commands |
| `!` | Run bash command |
| `!!` | Run bash command (excluded from context) |",
        key("tui.input.tab"),
        key("app.interrupt"),
        key("app.clear"),
        key("app.exit"),
        key("app.suspend"),
        key("app.thinking.cycle"),
        key("app.model.cycleForward"),
        key("app.model.cycleBackward"),
        key("app.model.select"),
        key("app.tools.expand"),
        key("app.thinking.toggle"),
        key("app.editor.external"),
        key("app.message.followUp"),
        key("app.message.dequeue"),
        key("app.clipboard.pasteImage"),
    )
}

/// An extension shortcut in the display spelling of the other key hints: each part starts
/// with a capital letter and `alt` takes the platform's name.
fn capitalize_shortcut(key: &str) -> String {
    maestro_app::format_key_text(key, true)
}

#[cfg(test)]
mod tests;
