//! Shared interactive chat components.

pub mod components;

pub use components::dynamic_border::DynamicBorder;

pub use components::countdown_timer::CountdownTimer;

pub use components::bordered_loader::{BorderedLoader, BorderedLoaderOptions};

pub use components::custom_editor::{ActionHandlers, CustomEditor};
pub use components::session_selector_search::*;
pub use components::user_message::UserMessageComponent;

pub use components::assistant_message::{AssistantMessageComponent, AssistantMessageOptions};

pub use components::branch_summary_message::BranchSummaryMessageComponent;

pub use components::compaction_summary_message::CompactionSummaryMessageComponent;

pub use components::thinking_selector::ThinkingSelectorComponent;

pub use components::show_images_selector::ShowImagesSelectorComponent;

pub use components::theme_selector::ThemeSelectorComponent;

pub use components::user_message_selector::UserMessageSelectorComponent;

pub use components::oauth_selector::{
    AuthSelectorProvider, AuthType, OAuthSelectorComponent, OAuthSelectorMode, OAuthSelectorOptions,
};

pub use components::extension_input::{ExtensionInputComponent, ExtensionInputOptions};
pub use components::extension_selector::{ExtensionSelectorComponent, ExtensionSelectorOptions};

pub use components::model_selector::{
    ModelSelectorCallbacks, ModelSelectorComponent, ModelSelectorOptions,
};
pub use components::settings_selector::{
    BehaviorSettings, SessionSettings, SettingsCallbacks, SettingsConfig, SettingsSelectorComponent,
};
pub use components::tree_selector::{FilterMode, SessionTreeNode, TreeList};
