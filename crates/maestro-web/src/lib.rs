//! Browser-facing attachments, offline session documents, localization and SVG glyphs.

pub mod browser;
/// The chat screen composing the session view and the artifacts panel.
#[cfg(target_arch = "wasm32")]
pub mod chat_panel;
pub mod components;
pub mod contributions;
pub mod dialogs;
pub mod docx_preview;
pub mod dom;
#[cfg(target_arch = "wasm32")]
pub use chat_panel::{ChatPanel, ChatPanelConfig, ToolsFactory};
#[cfg(target_arch = "wasm32")]
pub use components::agent_interface::AgentInterface;
pub use components::agent_interface::{
    AgentInterfaceProps, ApiKeyRequired, BeforeSend, BrowserSession,
};
pub use components::attachment_tile::AttachmentTile;
pub use components::custom_provider_card::CustomProviderCard;
#[cfg(target_arch = "wasm32")]
pub use components::input::Input;
#[cfg(target_arch = "wasm32")]
pub use components::message_editor::MessageEditor;
pub use components::message_editor::{EditorControls, MessageEditorProps};
pub use components::message_list::MessageList;
pub use components::message_renderer_registry::{
    MessageRenderer, MessageRole, get_message_renderer, register_message_renderer, render_message,
};
pub use components::messages::{
    AbortedMessage, AssistantMessage, ToolMessage, ToolMessageDebugView, UserMessage,
};
#[cfg(target_arch = "wasm32")]
pub use components::provider_key_input::ProviderKeyInput;
pub use components::streaming_message_container::StreamingMessageContainer;
#[cfg(target_arch = "wasm32")]
pub use components::theme_toggle::ThemeToggle;
pub use components::thinking_block::ThinkingBlock;
#[cfg(target_arch = "wasm32")]
pub use dialogs::attachment_overlay::AttachmentOverlay;
#[cfg(target_arch = "wasm32")]
pub use dialogs::{
    api_key_prompt_dialog::ApiKeyPromptDialog,
    custom_provider_dialog::CustomProviderDialog,
    model_selector_dialog::ModelSelector,
    persistent_storage_dialog::PersistentStorageDialog,
    providers_models_tab::ProvidersModelsTab,
    session_list_dialog::SessionListDialog,
    settings_dialog::{ApiKeysTab, ProxyTab, SettingsDialog, SettingsTab},
};
pub mod export_html;
pub mod icons;
pub mod storage;
#[cfg(target_arch = "wasm32")]
pub use storage::backends::indexeddb_storage_backend::IndexedDBStorageBackend;
pub mod utils;
#[cfg(target_arch = "wasm32")]
pub use utils::auth_token::{clear_auth_token, get_auth_token};
pub use utils::format::{format_cost, format_model_cost, format_token_count, format_usage};
#[cfg(target_arch = "wasm32")]
pub use utils::i18n::BrowserLanguageHost;
pub use utils::i18n::{
    I18n, I18nDiagnostic, I18nError, I18nMessages, LanguageCode, LanguageHost, TranslationFunction,
    TranslationValue, Translations,
};

pub use maestro_app::browser::components::messages::{
    ArtifactMessage, UserMessageWithAttachments, convert_attachments, default_convert_to_llm,
    is_artifact_message, is_user_message_with_attachments,
};
pub use maestro_app::browser::utils::attachment_utils::{
    Attachment, SharedAttachment, load_attachment,
};
pub use maestro_app::browser::utils::proxy_utils::{
    apply_proxy_if_needed, create_local_stream_fn, create_stream_fn, is_cors_error,
    should_use_proxy_for_provider,
};

pub use maestro_app::{
    AppStorage, AutoDiscoveryProviderType, CustomProvider, CustomProviderType,
    CustomProvidersStore, IndexConfig, IndexedDBConfig, ManualProviderType, ProviderKeysStore,
    SessionData, SessionMetadata, SessionsStore, SettingsStore, StorageBackend, StorageTransaction,
    Store, StoreConfig, get_app_storage, set_app_storage,
};

pub use maestro_app::browser::storage::{
    IndexDirection, QuotaInfo, StorageOperation, TransactionMode,
};

pub use components::sandbox::runtime_message_bridge::RuntimeMessageBridge;

#[cfg(target_arch = "wasm32")]
pub use components::sandbox::runtime_message_router::RUNTIME_MESSAGE_ROUTER;

pub use maestro_app::browser::components::sandbox::{
    artifacts_runtime_provider::ArtifactsRuntimeProvider,
    attachments_runtime_provider::AttachmentsRuntimeProvider,
    console_runtime_provider::{ConsoleLog, ConsoleRuntimeProvider},
    file_download_runtime_provider::{DownloadableFile, FileDownloadRuntimeProvider},
};
pub use maestro_app::browser::tools::prompts::ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RW;
pub mod tools;
pub use tools::artifacts::ArtifactElement;

#[cfg(target_arch = "wasm32")]
pub use components::sandboxed_iframe::{BrowserSandboxHost, SandboxIframe};
#[cfg(target_arch = "wasm32")]
pub use tools::artifacts::HtmlArtifact;
/// Artifact records and tabbed views using the application-selected filename.
pub use tools::artifacts::{Artifact, ArtifactsPanel, ArtifactsParams};

pub use maestro_app::{
    SandboxFile, SandboxResult, SandboxRuntimeProvider, SandboxUrlProvider,
    create_javascript_repl_tool,
};
#[cfg(target_arch = "wasm32")]
pub use tools::javascript_repl::JAVASCRIPT_REPL_TOOL;

pub use tools::artifacts::{TextArtifact, image_artifact::ImageArtifact};

/// Editable document artifact views.
pub use tools::artifacts::{MarkdownArtifact, SvgArtifact};
pub use tools::renderer_registry::{render_collapsible_header, render_header};
pub use tools::renderers::default_renderer::DefaultRenderer;
pub use tools::types::{ToolRenderResult, ToolRenderer};
pub use tools::{get_tool_renderer, register_tool_renderer, render_tool, set_show_json_mode};

pub use components::console_block::ConsoleBlock;

pub use components::expandable_section::ExpandableSection;
pub use tools::renderers::bash_renderer::BashRenderer;
pub use tools::renderers::calculate_renderer::CalculateRenderer;
pub use tools::renderers::get_current_time_renderer::GetCurrentTimeRenderer;

pub use tools::artifacts::{ArtifactsToolRenderer, artifact_pill::artifact_pill};

mod ui_error;

#[cfg(target_arch = "wasm32")]
pub use tools::artifacts::NativeArtifact;
