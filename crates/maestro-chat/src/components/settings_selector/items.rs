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
/// Creates the thirteen supplied rows in their display order.
pub(super) fn rows(
    config: SettingsConfig,
    callbacks: &Rc<SettingsCallbacks>,
    theme: &LiveTheme,
) -> Vec<SettingItem> {
    let mut items = session_rows(&config);
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
                "Alt+Enter queues follow-up messages until agent stops. 'one-at-a-time': deliver one, wait for response. 'all': deliver all at once.",
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
    match id {
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
