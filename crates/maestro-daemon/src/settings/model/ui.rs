//! Client interface preferences and defaults.

use super::{RightClickPassthroughModifierConfig, ToastConfig};
use crate::settings::{
    sidebar::SidebarConfig, sound::SoundConfig, tab_bar::TabBarRightEntryConfig,
};
use maestro_protocol::shared::Flag;
use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;

/// Agent panel ordering selection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentPanelSortConfig {
    /// Spaces presentation.
    #[default]
    #[serde(alias = "workspaces")]
    Spaces,
    /// Priority presentation.
    Priority,
}

impl AgentPanelSortConfig {
    /// Stored selection name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Spaces => "spaces",
            Self::Priority => "priority",
        }
    }
}

/// Status indicator style selection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusIndicatorStyle {
    /// Dots presentation.
    #[default]
    Dots,
    /// Symbols presentation.
    Symbols,
}

impl StatusIndicatorStyle {
    /// Stored selection name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dots => "dots",
            Self::Symbols => "symbols",
        }
    }
}

/// Host cursor rendering selection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HostCursorModeConfig {
    /// Auto presentation.
    #[default]
    Auto,
    /// Native presentation.
    Native,
    /// Drawn presentation.
    Drawn,
}

/// Collapsed sidebar selection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SidebarCollapsedModeConfig {
    /// Compact presentation.
    #[default]
    Compact,
    /// Hidden presentation.
    Hidden,
}

/// Tab bar position selection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TabBarPositionConfig {
    /// Top presentation.
    #[default]
    Top,
    /// Bottom presentation.
    Bottom,
}

/// Client interface preferences.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UiConfig {
    /// Initial expanded sidebar width.
    pub sidebar_width: u16,
    /// Minimum sidebar width (columns) when expanded. Default: 18.
    pub sidebar_min_width: u16,
    /// Maximum sidebar width (columns) when expanded. Default: 36.
    pub sidebar_max_width: u16,
    /// Start with the sidebar collapsed. Default: false.
    pub sidebar_start_collapsed: Flag,
    /// Collapsed sidebar presentation. Default: compact.
    pub sidebar_collapsed_mode: SidebarCollapsedModeConfig,
    /// Terminal width at or below which Maestro uses the mobile single-column layout. Default: 64.
    pub mobile_width_threshold: u16,
    /// Capture mouse input for Maestro's mouse UI. Default: true.
    pub mouse_capture: Flag,
    /// Copy text selected with the mouse. Default: true.
    pub copy_on_select: Flag,
    /// Host cursor policy. Default: auto.
    pub host_cursor: HostCursorModeConfig,
    /// Modifier that lets right-click gestures pass through to pane apps. Empty disables it.
    pub right_click_passthrough_modifier: RightClickPassthroughModifierConfig,
    /// Force a full host-terminal redraw when the outer terminal regains focus. Default: true.
    pub redraw_on_focus_gained: Flag,
    /// Lines to scroll per mouse wheel notch. Default: 3.
    pub mouse_scroll_lines: Option<NonZeroUsize>,
    /// Ask for confirmation before closing a workspace. Default: true.
    pub confirm_close: Flag,
    /// Ask for a tab name before creating a new tab. Default: true.
    pub prompt_new_tab_name: Flag,
    /// Ask for a workspace name before interactive creation. Default: false.
    pub prompt_new_workspace_name: Flag,
    /// Draw borders around split panes. Default: true.
    pub pane_borders: Flag,
    /// Draw borders along the outside edge of the pane area. Default: true.
    pub pane_outer_borders: Flag,
    /// Draw interactive scrollbars beside terminal panes. Default: true.
    pub pane_scrollbars: Flag,
    /// Keep split panes visually separated instead of sharing divider borders. Default: true.
    pub pane_gaps: Flag,
    /// Show agent labels in split pane borders when no manual pane label is set. Default: false.
    pub show_agent_labels_on_pane_borders: Flag,
    /// Hide the tab row when the workspace has one tab. Default: false.
    pub hide_tab_bar_when_single_tab: Flag,
    /// Desktop tab row placement. Default: top.
    pub tab_bar_position: TabBarPositionConfig,
    /// Ordered entries shown at the right edge of the desktop tab row. Empty by default.
    pub tab_bar_right: Vec<TabBarRightEntryConfig>,
    /// Text inserted between visible right-side tab bar entries. Default: one space.
    pub tab_bar_right_separator: String,
    /// Format for the outer terminal window title. Empty leaves the title alone.
    /// Default: "{hostname}: {workspace}".
    pub window_title: String,
    /// Agent sidebar ordering. Saved values are "spaces" or "priority". Default: "spaces".
    pub agent_panel_sort: AgentPanelSortConfig,
    /// Agent status indicator style. Saved values are "dots" or "symbols". Default: "dots".
    pub status_indicators: StatusIndicatorStyle,
    /// Expanded sidebar row composition.
    pub sidebar: SidebarConfig,
    /// Accent color for highlights, borders, and navigation UI.
    /// Accepts hexadecimal, named colors or RGB function notation.
    pub accent: String,
    /// Optional visual toast notifications for background workspace events.
    pub toast: ToastConfig,
    /// Play sounds when agents change state in background workspaces.
    pub sound: SoundConfig,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            sidebar_width: 26,
            sidebar_min_width: 18,
            sidebar_max_width: 36,
            sidebar_start_collapsed: Flag(false),
            sidebar_collapsed_mode: SidebarCollapsedModeConfig::Compact,
            mobile_width_threshold: 64,
            mouse_capture: Flag(true),
            copy_on_select: Flag(true),
            host_cursor: HostCursorModeConfig::Auto,
            right_click_passthrough_modifier: RightClickPassthroughModifierConfig::default(),
            redraw_on_focus_gained: Flag(true),
            mouse_scroll_lines: None,
            confirm_close: Flag(true),
            prompt_new_tab_name: Flag(true),
            prompt_new_workspace_name: Flag(false),
            pane_borders: Flag(true),
            pane_outer_borders: Flag(true),
            pane_scrollbars: Flag(true),
            pane_gaps: Flag(true),
            show_agent_labels_on_pane_borders: Flag(false),
            hide_tab_bar_when_single_tab: Flag(false),
            tab_bar_position: TabBarPositionConfig::Top,
            tab_bar_right: Vec::new(),
            tab_bar_right_separator: " ".into(),
            window_title: crate::settings::window_title::default_window_title(),
            agent_panel_sort: AgentPanelSortConfig::Spaces,
            status_indicators: StatusIndicatorStyle::Dots,
            sidebar: SidebarConfig::default(),
            accent: "cyan".into(),
            toast: ToastConfig::default(),
            sound: SoundConfig::default(),
        }
    }
}

impl UiConfig {
    /// Lines scrolled per mouse-wheel notch.
    #[must_use]
    pub fn mouse_scroll_lines(&self) -> usize {
        self.mouse_scroll_lines.map_or(3, NonZeroUsize::get)
    }

    /// Shared modifier bits required for right-click pass-through.
    #[must_use]
    pub fn right_click_passthrough_modifiers(&self) -> Option<u8> {
        self.right_click_passthrough_modifier.modifiers()
    }
}
