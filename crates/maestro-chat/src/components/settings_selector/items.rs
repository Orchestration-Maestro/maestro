//! Ordered settings rows and notifications to their typed owners.
use super::{SettingsCallbacks, SettingsConfig, submenus};
use crate::components::thinking_selector::{thinking_item, thinking_text};
use maestro_app::{DoubleEscapeAction, MessageDeliveryMode, Transport, TreeFilterMode};
use maestro_theme::LiveTheme;
use maestro_tui::{SelectItem, SettingItem};
use std::{cell::RefCell, rc::Rc};

/// Creates a setting with optional cyclic choices.
fn row(text: (&str, &str, &str), current: String, values: &[&str]) -> SettingItem {
    SettingItem {
        id: text.0.into(),
        label: text.1.into(),
        description: Some(text.2.into()),
        current_value: current,
        values: (!values.is_empty()).then(|| values.iter().map(|value| (*value).into()).collect()),
        submenu: None,
    }
}
/// The follow-up mode description, naming the key bound to queue a follow-up.
fn follow_up_description(key: &str) -> String {
    let _ = key;
    "Alt+Enter queues follow-up messages until agent stops. 'one-at-a-time': deliver one, wait for response. 'all': deliver all at once.".to_owned()
}

/// Creates the supplied rows in their display order.
pub(super) fn rows(
    config: SettingsConfig,
    callbacks: &Rc<SettingsCallbacks>,
    theme: &LiveTheme,
    supports_images: bool,
) -> Vec<SettingItem> {
    let mut items = session_rows(&config);
    items.splice(1..1, display_rows(&config, supports_images));
    items.extend(behavior_rows(&config));

    items.push(warnings(config.warnings, callbacks.clone(), theme.clone()));
    items.push(thinking(
        &config.thinking_level,
        config.available_thinking_levels,
        callbacks.clone(),
        theme.clone(),
    ));
    items.push(themes(
        config.current_theme,
        config.available_themes,
        callbacks.clone(),
        theme.clone(),
    ));
    items
}
/// Composes one display segment using the construction-time capability.
fn display_rows(config: &SettingsConfig, supports_images: bool) -> Vec<SettingItem> {
    let mut items = if supports_images {
        inline_image_rows(config)
    } else {
        Vec::new()
    };
    items.extend(image_policy_rows(config));
    items.extend(terminal_rows(config));
    items
}
/// Inline image choices for image-capable terminals.
fn inline_image_rows(config: &SettingsConfig) -> Vec<SettingItem> {
    vec![
        row(
            (
                "show-images",
                "Show images",
                "Render images inline in terminal",
            ),
            config.show_images.to_string(),
            &["true", "false"],
        ),
        row(
            (
                "image-width-cells",
                "Image width",
                "Preferred inline image width in terminal cells",
            ),
            ryu_js::Buffer::new()
                .format(config.image_width_cells)
                .into(),
            &["60", "80", "120"],
        ),
    ]
}
/// Image request policies and skill command registration.
fn image_policy_rows(config: &SettingsConfig) -> Vec<SettingItem> {
    vec![
        row(
            (
                "auto-resize-images",
                "Auto-resize images",
                "Resize large images to 2000x2000 max for better model compatibility",
            ),
            config.auto_resize_images.to_string(),
            &["true", "false"],
        ),
        row(
            (
                "block-images",
                "Block images",
                "Prevent images from being sent to LLM providers",
            ),
            config.block_images.to_string(),
            &["true", "false"],
        ),
        row(
            (
                "skill-commands",
                "Skill commands",
                "Register skills as /skill:name commands",
            ),
            config.session.enable_skill_commands.to_string(),
            &["true", "false"],
        ),
    ]
}
/// Terminal cursor, editor and progress choices.
fn terminal_rows(config: &SettingsConfig) -> Vec<SettingItem> {
    vec![
        row(
            (
                "show-hardware-cursor",
                "Show hardware cursor",
                "Show the terminal cursor while still positioning it for IME support",
            ),
            config.terminal.show_hardware_cursor.to_string(),
            &["true", "false"],
        ),
        row(
            (
                "editor-padding",
                "Editor padding",
                "Horizontal padding for input editor (0-3)",
            ),
            ryu_js::Buffer::new().format(config.editor_padding_x).into(),
            &["0", "1", "2", "3"],
        ),
        row(
            (
                "autocomplete-max-visible",
                "Autocomplete max items",
                "Max visible items in autocomplete dropdown (3-20)",
            ),
            ryu_js::Buffer::new()
                .format(config.autocomplete_max_visible)
                .into(),
            &["3", "5", "7", "10", "15", "20"],
        ),
        row(
            (
                "clear-on-shrink",
                "Clear on shrink",
                "Clear empty rows when content shrinks (may cause flicker)",
            ),
            config.terminal.clear_on_shrink.to_string(),
            &["true", "false"],
        ),
        row(
            (
                "terminal-progress",
                "Terminal progress",
                "Show OSC 9;4 progress indicators in the terminal tab bar",
            ),
            config.terminal.show_terminal_progress.to_string(),
            &["true", "false"],
        ),
    ]
}
/// Session policies preceding the behavior rows.
fn session_rows(config: &SettingsConfig) -> Vec<SettingItem> {
    vec![
        row(
            (
                "autocompact",
                "Auto-compact",
                "Automatically compact context when it gets too large",
            ),
            config.session.auto_compact.to_string(),
            &["true", "false"],
        ),
        row(
            (
                "steering-mode",
                "Steering mode",
                "Enter while streaming queues steering messages. 'one-at-a-time': deliver one, wait for response. 'all': deliver all at once.",
            ),
            config.steering_mode.as_str().into(),
            &["one-at-a-time", "all"],
        ),
        row(
            (
                "follow-up-mode",
                "Follow-up mode",
                &follow_up_description(&maestro_app::key_display_text("app.message.followUp")),
            ),
            config.follow_up_mode.as_str().into(),
            &["one-at-a-time", "all"],
        ),
        row(
            (
                "transport",
                "Transport",
                "Preferred transport for providers that support multiple transports",
            ),
            config.transport.as_str().into(),
            &["sse", "websocket", "websocket-cached", "auto"],
        ),
    ]
}
/// Behavior preferences in their displayed order.
fn behavior_rows(config: &SettingsConfig) -> Vec<SettingItem> {
    vec![
        row(
            (
                "hide-thinking",
                "Hide thinking",
                "Hide thinking blocks in assistant responses",
            ),
            config.behavior.hide_thinking_block.to_string(),
            &["true", "false"],
        ),
        row(
            (
                "collapse-changelog",
                "Collapse changelog",
                "Show condensed changelog after updates",
            ),
            config.behavior.collapse_changelog.to_string(),
            &["true", "false"],
        ),
        row(
            (
                "quiet-startup",
                "Quiet startup",
                "Disable verbose printing at startup",
            ),
            config.session.quiet_startup.to_string(),
            &["true", "false"],
        ),
        row(
            (
                "install-telemetry",
                "Install telemetry",
                "Send an anonymous version/update ping after changelog-detected updates",
            ),
            config.behavior.enable_install_telemetry.to_string(),
            &["true", "false"],
        ),
        row(
            (
                "double-escape-action",
                "Double-escape action",
                "Action when pressing Escape twice with empty editor",
            ),
            config.double_escape_action.as_str().into(),
            &["tree", "fork", "none"],
        ),
        row(
            (
                "tree-filter-mode",
                "Tree filter mode",
                "Default filter when opening /tree",
            ),
            config.tree_filter_mode.as_str().into(),
            &["default", "no-tools", "user-only", "labeled-only", "all"],
        ),
    ]
}
/// Retains warning changes across submenu openings.
fn warnings(
    initial: maestro_app::WarningSettings,
    callbacks: Rc<SettingsCallbacks>,
    theme: LiveTheme,
) -> SettingItem {
    let mut item = row(
        (
            "warnings",
            "Warnings",
            "Enable or disable individual warnings",
        ),
        "configure".into(),
        &[],
    );
    let state = Rc::new(RefCell::new(initial));
    item.submenu = Some(Rc::new(move |_, done| {
        let initial = state.borrow().clone();
        let state = state.clone();
        let callback = callbacks.on_warnings_change.clone();
        Rc::new(submenus::WarningSettingsSubmenu::new(
            initial,
            Rc::new(move |value| {
                state.replace(value.clone());
                callback(value);
            }),
            Rc::new(move || done(None)),
            theme.clone(),
        ))
    }));
    item
}
/// Offers reasoning choices without preview notification.
fn thinking(
    current: &maestro_app::ThinkingLevel,
    levels: Vec<maestro_app::ThinkingLevel>,
    callbacks: Rc<SettingsCallbacks>,
    theme: LiveTheme,
) -> SettingItem {
    let mut item = row(
        (
            "thinking",
            "Thinking level",
            "Reasoning depth for thinking-capable models",
        ),
        thinking_text(current).0.into(),
        &[],
    );
    item.submenu = Some(Rc::new(move |current, done| {
        let cancel = done.clone();
        let callback = callbacks.on_thinking_level_change.clone();
        Rc::new(submenus::SelectSubmenu::new(
            submenus::Selection {
                title: "Thinking Level",
                description: "Select reasoning depth for thinking-capable models",
                options: levels.iter().map(thinking_item).collect(),
                current,
            },
            submenus::SelectionCallbacks {
                select: Rc::new(move |value| {
                    if let Ok(level) = value.parse() {
                        callback(level);
                        done(Some(value.into()));
                    }
                }),
                cancel: Rc::new(move || cancel(None)),
                preview: None,
            },
            theme.clone(),
        ))
    }));
    item
}
/// Captures the latest committed theme for each cancellation.
fn themes(
    current: String,
    names: Vec<String>,
    callbacks: Rc<SettingsCallbacks>,
    theme: LiveTheme,
) -> SettingItem {
    let mut item = row(
        ("theme", "Theme", "Color theme for the interface"),
        current,
        &[],
    );
    item.submenu = Some(Rc::new(move |current, done| {
        let cancel = done.clone();
        let callback = callbacks.on_theme_change.clone();
        let preview = callbacks.on_theme_preview.clone();
        let restore = current.clone();
        Rc::new(submenus::SelectSubmenu::new(
            submenus::Selection {
                title: "Theme",
                description: "Select color theme",
                options: names
                    .iter()
                    .map(|name| SelectItem {
                        value: name.clone(),
                        label: name.clone(),
                        description: None,
                    })
                    .collect(),
                current,
            },
            submenus::SelectionCallbacks {
                select: Rc::new(move |value| {
                    callback(value);
                    done(Some(value.into()));
                }),
                cancel: Rc::new(move || {
                    if let Some(preview) = &preview {
                        preview(&restore);
                    }
                    cancel(None);
                }),
                preview: callbacks.on_theme_preview.clone(),
            },
            theme.clone(),
        ))
    }));
    item
}
/// Converts only the selected cyclic setting into its existing owner type.
pub(super) fn change(callbacks: &SettingsCallbacks, id: &str, value: &str) {
    let numeric_callback = match id {
        "image-width-cells" => Some(&callbacks.on_image_width_cells_change),
        "editor-padding" => Some(&callbacks.on_editor_padding_x_change),
        "autocomplete-max-visible" => Some(&callbacks.on_autocomplete_max_visible_change),
        _ => None,
    };
    if let Some(callback) = numeric_callback {
        if let Ok(value) = value.parse() {
            callback(value);
        }
        return;
    }
    match id {
        "show-images" => (callbacks.on_show_images_change)(value == "true"),
        "auto-resize-images" => (callbacks.on_auto_resize_images_change)(value == "true"),
        "block-images" => (callbacks.on_block_images_change)(value == "true"),
        "skill-commands" => (callbacks.on_enable_skill_commands_change)(value == "true"),
        "show-hardware-cursor" => (callbacks.on_show_hardware_cursor_change)(value == "true"),
        "clear-on-shrink" => (callbacks.on_clear_on_shrink_change)(value == "true"),
        "terminal-progress" => (callbacks.on_show_terminal_progress_change)(value == "true"),
        "autocompact" => (callbacks.on_auto_compact_change)(value == "true"),
        "steering-mode" => (callbacks.on_steering_mode_change)(delivery(value)),
        "follow-up-mode" => (callbacks.on_follow_up_mode_change)(delivery(value)),
        "transport" => (callbacks.on_transport_change)(match value {
            "websocket" => Transport::Websocket,
            "websocket-cached" => Transport::WebsocketCached,
            "auto" => Transport::Auto,
            _ => Transport::Sse,
        }),
        "hide-thinking" => (callbacks.on_hide_thinking_block_change)(value == "true"),
        "collapse-changelog" => (callbacks.on_collapse_changelog_change)(value == "true"),
        "quiet-startup" => (callbacks.on_quiet_startup_change)(value == "true"),
        "install-telemetry" => (callbacks.on_enable_install_telemetry_change)(value == "true"),
        "double-escape-action" => (callbacks.on_double_escape_action_change)(match value {
            "fork" => DoubleEscapeAction::Fork,
            "none" => DoubleEscapeAction::None,
            _ => DoubleEscapeAction::Tree,
        }),
        "tree-filter-mode" => (callbacks.on_tree_filter_mode_change)(match value {
            "no-tools" => TreeFilterMode::NoTools,
            "user-only" => TreeFilterMode::UserOnly,
            "labeled-only" => TreeFilterMode::LabeledOnly,
            "all" => TreeFilterMode::All,
            _ => TreeFilterMode::Default,
        }),
        _ => {}
    }
}
/// Converts the fixed delivery choices.
fn delivery(value: &str) -> MessageDeliveryMode {
    if value == "all" {
        MessageDeliveryMode::All
    } else {
        MessageDeliveryMode::OneAtATime
    }
}

#[cfg(test)]
mod tests;
