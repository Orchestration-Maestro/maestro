//! Shared interactive chat components.

pub mod components;

pub use components::dynamic_border::DynamicBorder;

pub use components::countdown_timer::CountdownTimer;

pub use components::loading::{BorderedLoader, BorderedLoaderOptions};

pub use components::authored::UserMessageComponent;
pub use components::custom_editor::{ActionHandlers, CustomEditor};
pub use components::session_selector_search::*;

pub use components::response::{AssistantMessageComponent, AssistantMessageOptions};

pub use components::history_summary::BranchSummaryMessageComponent;

pub use components::custom_content::CustomMessageComponent;

#[cfg(not(target_arch = "wasm32"))]
pub use components::invocation::SkillInvocationMessageComponent;

pub use components::context_summary::CompactionSummaryMessageComponent;

pub use components::reasoning::ThinkingSelectorComponent;

pub use components::images::ShowImagesSelectorComponent;

pub use components::appearance::ThemeSelectorComponent;

pub use components::history::UserMessageSelectorComponent;

#[cfg(not(target_arch = "wasm32"))]
pub use components::extension_composer::{ExtensionEditorComponent, ExtensionEditorOptions};
pub use components::oauth_selector::{
    AuthSelectorProvider, AuthType, OAuthSelectorComponent, OAuthSelectorMode, OAuthSelectorOptions,
};

pub use components::extension_prompt::{ExtensionInputComponent, ExtensionInputOptions};
pub use components::extensions::{ExtensionSelectorComponent, ExtensionSelectorOptions};

pub use components::models::{
    ModelSelectorCallbacks, ModelSelectorComponent, ModelSelectorOptions,
};
pub use components::preferences::{
    BehaviorSettings, SessionSettings, SettingsCallbacks, SettingsConfig,
    SettingsSelectorComponent, TerminalDisplaySettings,
};
pub use components::tree_picker::{
    FilterMode, SessionTreeNode, TreeList, TreeSelectorCallbacks, TreeSelectorComponent,
    TreeSelectorOptions,
};

pub use components::authentication::{
    LoginDialogComponent, LoginDialogError, LoginDialogOptions, LoginDialogPlatform,
    LoginInputFuture,
};

pub use components::session_picker::*;

#[cfg(test)]
extern crate self as maestro_chat;
// The shared native test helpers name their child-process owner at the crate root.
#[cfg(test)]
#[cfg(unix)]
use components::session_picker::list::tests::session_picker::native_child as child;
pub use components::shell_output::BashExecutionComponent;

#[cfg(not(target_arch = "wasm32"))]
pub use components::status::FooterComponent;

pub use components::tool_output::{
    ToolExecutionComponent, ToolExecutionOptions, ToolExecutionParams, ToolExecutionResult,
};

#[cfg(not(target_arch = "wasm32"))]
pub mod interaction;

#[cfg(not(target_arch = "wasm32"))]
pub use interaction::{InteractiveMode, InteractiveModeOptions};

#[cfg(not(target_arch = "wasm32"))]
mod external_editor;

#[cfg(not(target_arch = "wasm32"))]
mod temporary_file;
