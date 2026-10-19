//! Owned typed workspace preference areas.

use maestro_protocol::shared::Flag;
use serde::{Deserialize, Serialize};

mod modifier;
mod toast;

pub use modifier::RightClickPassthroughModifierConfig;
pub use toast::{
    ClipboardToastConfig, MAX_TOAST_DELAY_SECONDS, ToastClipboardPosition, ToastConfig,
    ToastDelivery, ToastMaestroPosition, WorkspaceToastConfig,
};

/// Interactive shell startup mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellModeConfig {
    /// Choose the shell's usual startup mode.
    #[default]
    Auto,
    /// Start a login shell.
    Login,
    /// Start a non-login shell.
    NonLogin,
}

/// Shell preferences for newly created interactive panes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct TerminalConfig {
    /// Executable; empty selects the supplied shell or platform fallback.
    #[serde(rename = "paneShell")]
    pub default_shell: String,
    /// Shell startup mode.
    #[serde(rename = "paneShellMode")]
    pub shell_mode: ShellModeConfig,
    /// Directory policy or authored path for newly created panes.
    #[serde(rename = "newPaneCwd")]
    pub new_cwd: NewTerminalCwdConfig,
}

/// Directory selection for new panes, tabs and workspaces.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum NewTerminalCwdConfig {
    /// Follow the focused pane.
    #[default]
    Follow,
    /// Use the home directory.
    Home,
    /// Use the current process directory.
    Current,
    /// Use an authored path without changing its spelling.
    Path(String),
}

impl<'de> Deserialize<'de> for NewTerminalCwdConfig {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Ok(match value.trim() {
            "" | "follow" => Self::Follow,
            "home" => Self::Home,
            "current" => Self::Current,
            _ => Self::Path(value),
        })
    }
}

/// Restore preferences for saved daemon sessions.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionConfig {
    /// Resume supported agent conversations when restoring panes.
    pub resume_agents_on_restore: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            resume_agents_on_restore: true,
        }
    }
}

/// Product release channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannelConfig {
    /// Stable releases.
    #[default]
    Stable,
    /// Preview releases.
    Preview,
}

impl UpdateChannelConfig {
    /// Stored channel name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Preview => "preview",
        }
    }
}

/// Update and detection catalog check preferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UpdateConfig {
    /// Preferred release channel.
    pub channel: UpdateChannelConfig,
    /// Check for product updates.
    pub version_check: bool,
    /// Check for detection catalog updates.
    #[serde(rename = "detectionCatalogCheck")]
    pub manifest_check: bool,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            channel: default_update_channel_for_build(
                std::env::consts::OS,
                maestro_protocol::build_info::is_preview(),
            ),
            version_check: true,
            manifest_check: true,
        }
    }
}

/// Select the channel from the supplied host and build identity.
#[must_use]
pub(crate) fn default_update_channel_for_build(host: &str, preview: bool) -> UpdateChannelConfig {
    if host == "windows" && preview {
        UpdateChannelConfig::Preview
    } else {
        UpdateChannelConfig::Stable
    }
}

/// Virtual terminal dimensions when no client is attached.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ServerConfig {
    /// Terminal width in columns.
    pub headless_cols: u16,
    /// Terminal height in rows.
    pub headless_rows: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            headless_cols: 120,
            headless_rows: 40,
        }
    }
}

impl ServerConfig {
    /// Validated dimensions, falling back as a pair when either is zero.
    #[must_use]
    pub fn headless_size(&self) -> (u16, u16) {
        if self.invalid_headless_size_diagnostic().is_some() {
            let defaults = Self::default();
            (defaults.headless_cols, defaults.headless_rows)
        } else {
            (self.headless_cols, self.headless_rows)
        }
    }

    /// Explain an invalid dimension pair.
    #[must_use]
    pub fn invalid_headless_size_diagnostic(&self) -> Option<String> {
        (self.headless_cols == 0 || self.headless_rows == 0).then(|| {
            format!(
                "daemon.headlessCols and daemon.headlessRows must be greater than zero (got {}x{})",
                self.headless_cols, self.headless_rows
            )
        })
    }
}

/// Root directory for repository and branch checkouts.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct WorktreesConfig {
    /// Authored directory, expanded only by its I/O consumer.
    #[serde(rename = "worktreesDirectory")]
    pub directory: String,
}

impl Default for WorktreesConfig {
    fn default() -> Self {
        Self {
            directory: "~/.maestro/worktrees".into(),
        }
    }
}

/// Per-pane terminal memory preferences.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AdvancedConfig {
    /// Maximum retained scrollback bytes per terminal.
    pub scrollback_limit_bytes: usize,
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            scrollback_limit_bytes: 10_000_000,
        }
    }
}

/// Cursor shape used for the forced input-method anchor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImeCursorShape {
    /// Blinking block.
    Block,
    /// Steady block.
    #[default]
    SteadyBlock,
    /// Blinking underline.
    Underline,
    /// Steady underline.
    SteadyUnderline,
    /// Blinking bar.
    Bar,
    /// Steady bar.
    SteadyBar,
}

impl ImeCursorShape {
    /// DECSCUSR parameter for this shape.
    #[must_use]
    pub const fn to_decscusr(self) -> u8 {
        match self {
            Self::Block => 1,
            Self::SteadyBlock => 2,
            Self::Underline => 3,
            Self::SteadyUnderline => 4,
            Self::Bar => 5,
            Self::SteadyBar => 6,
        }
    }
}

/// Independent optional client capabilities.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ExperimentalConfig {
    /// Permit a client inside an existing pane.
    pub allow_nested: Flag,
    /// Render local Kitty graphics.
    pub kitty_graphics: Flag,
    /// Persist terminal screen history.
    pub pane_history: Flag,
    /// Expose an input-method anchor even when the pane hides its cursor.
    pub reveal_hidden_cursor_for_cjk_ime: Flag,
    /// Restrict input-method cursor anchoring to these detected agent names.
    pub cjk_ime_agents: Vec<String>,
    /// Cursor shape used for the input-method anchor.
    pub cjk_ime_cursor_shape: ImeCursorShape,
    /// Temporarily select an ASCII input source while prefix mode is active.
    pub switch_ascii_input_source_in_prefix: Flag,
}
