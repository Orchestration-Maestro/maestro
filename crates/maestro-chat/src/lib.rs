//! Shared interactive chat components.

pub mod components;

pub use components::dynamic_border::DynamicBorder;

pub use components::extension_widgets::ExtensionWidgetHost;

pub use components::countdown_timer::CountdownTimer;

pub use components::bordered_loader::{BorderedLoader, BorderedLoaderOptions};

pub use components::custom_editor::{ActionHandlers, CustomEditor};
pub use components::session_selector_search::*;
pub use components::user_message::UserMessageComponent;

pub use components::assistant_message::{AssistantMessageComponent, AssistantMessageOptions};

pub use components::branch_summary_message::BranchSummaryMessageComponent;

pub use components::custom_message::CustomMessageComponent;

#[cfg(not(target_arch = "wasm32"))]
pub use components::skill_invocation_message::SkillInvocationMessageComponent;

pub use components::compaction_summary_message::CompactionSummaryMessageComponent;

pub use components::thinking_selector::ThinkingSelectorComponent;

pub use components::show_images_selector::ShowImagesSelectorComponent;

pub use components::theme_selector::ThemeSelectorComponent;

pub use components::user_message_selector::UserMessageSelectorComponent;

#[cfg(not(target_arch = "wasm32"))]
pub use components::extension_editor::{ExtensionEditorComponent, ExtensionEditorOptions};
pub use components::oauth_selector::{
    AuthSelectorProvider, AuthType, OAuthSelectorComponent, OAuthSelectorMode, OAuthSelectorOptions,
};

pub use components::extension_input::{ExtensionInputComponent, ExtensionInputOptions};
pub use components::extension_selector::{ExtensionSelectorComponent, ExtensionSelectorOptions};

pub use components::model_selector::{
    ModelSelectorCallbacks, ModelSelectorComponent, ModelSelectorOptions,
};
pub use components::settings_selector::{
    BehaviorSettings, SessionSettings, SettingsCallbacks, SettingsConfig,
    SettingsSelectorComponent, TerminalDisplaySettings,
};
pub use components::tree_selector::{
    FilterMode, SessionTreeNode, TreeList, TreeSelectorCallbacks, TreeSelectorComponent,
    TreeSelectorOptions,
};

pub use components::login_dialog::{
    LoginDialogComponent, LoginDialogError, LoginDialogOptions, LoginDialogPlatform,
    LoginInputFuture,
};

pub use components::session_selector::*;

#[cfg(test)]
extern crate self as maestro_chat;
// The shared native test helpers name their child-process owner at the crate root.
pub use components::bash_execution::BashExecutionComponent;
#[cfg(test)]
#[cfg(unix)]
use components::session_selector::list::tests::session_picker::native_child as child;

#[cfg(not(target_arch = "wasm32"))]
pub use components::footer::FooterComponent;

pub use components::tool_execution::{
    ToolExecutionComponent, ToolExecutionOptions, ToolExecutionParams, ToolExecutionResult,
};

#[cfg(not(target_arch = "wasm32"))]
pub mod interactive_mode;
#[cfg(not(target_arch = "wasm32"))]
mod local_dispatch;

#[cfg(not(target_arch = "wasm32"))]
pub use interactive_mode::{InteractiveMode, InteractiveModeOptions};

#[cfg(not(target_arch = "wasm32"))]
mod external_editor;
mod whole_cell_width;

#[cfg(not(target_arch = "wasm32"))]
mod temporary_file;
