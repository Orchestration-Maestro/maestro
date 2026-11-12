//! Shared interactive chat components.

pub mod components;

pub use components::dynamic_border::DynamicBorder;

pub use components::extension_widgets::ExtensionWidgetHost;

pub use components::countdown_timer::CountdownTimer;

pub use components::{BorderedLoader, BorderedLoaderOptions};

pub use components::UserMessageComponent;
pub use components::custom_editor::{ActionHandlers, CustomEditor};
pub use components::session_selector_search::*;

pub use components::{AssistantMessageComponent, AssistantMessageOptions};

pub use components::BranchSummaryMessageComponent;

pub use components::CustomMessageComponent;

#[cfg(not(target_arch = "wasm32"))]
pub use components::SkillInvocationMessageComponent;

pub use components::CompactionSummaryMessageComponent;

pub use components::ThinkingSelectorComponent;

pub use components::ShowImagesSelectorComponent;

pub use components::ThemeSelectorComponent;

pub use components::UserMessageSelectorComponent;

pub use components::oauth_selector::{
    AuthSelectorProvider, AuthType, OAuthSelectorComponent, OAuthSelectorMode, OAuthSelectorOptions,
};
#[cfg(not(target_arch = "wasm32"))]
pub use components::{ExtensionEditorComponent, ExtensionEditorOptions};

pub use components::{ExtensionInputComponent, ExtensionInputOptions};
pub use components::{ExtensionSelectorComponent, ExtensionSelectorOptions};

pub use components::{
    BehaviorSettings, SessionSettings, SettingsCallbacks, SettingsConfig,
    SettingsSelectorComponent, TerminalDisplaySettings,
};
pub use components::{
    FilterMode, SessionTreeNode, TreeList, TreeSelectorCallbacks, TreeSelectorComponent,
    TreeSelectorOptions,
};
pub use components::{ModelSelectorCallbacks, ModelSelectorComponent, ModelSelectorOptions};

pub use components::{
    LoginDialogComponent, LoginDialogError, LoginDialogOptions, LoginDialogPlatform,
    LoginInputFuture,
};

pub use components::{
    SessionFileOperations, SessionList, SessionListOptions, SessionListProgress, SessionScope,
    SessionSelectorCallbacks, SessionSelectorComponent, SessionSelectorHeader,
    SessionSelectorHeaderOptions, SessionSelectorOptions, SessionsFuture, SessionsLoader,
    StatusMessage, TrashResult,
};

#[cfg(test)]
pub(crate) use components::session_selector::list::tests::platform_keys;

#[cfg(test)]
extern crate self as maestro_chat;
pub use components::BashExecutionComponent;

#[cfg(not(target_arch = "wasm32"))]
pub use components::FooterComponent;

pub use components::{
    ToolExecutionComponent, ToolExecutionOptions, ToolExecutionParams, ToolExecutionResult,
};

#[cfg(not(target_arch = "wasm32"))]
mod interactive_mode;
#[cfg(not(target_arch = "wasm32"))]
mod local_dispatch;

#[cfg(not(target_arch = "wasm32"))]
pub use interactive_mode::*;

mod environment;
#[cfg(not(target_arch = "wasm32"))]
mod external_editor;
mod whole_cell_width;

#[cfg(not(target_arch = "wasm32"))]
mod temporary_file;
