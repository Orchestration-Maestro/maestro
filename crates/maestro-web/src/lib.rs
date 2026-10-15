//! Browser-facing attachments, offline session documents, localization and SVG glyphs.

pub mod browser;
pub mod components;
pub mod dialogs;
pub use components::attachment_tile::AttachmentTile;
pub use components::input::Input;
pub use dialogs::attachment_overlay::AttachmentOverlay;
pub mod export_html;
pub mod icons;
pub mod storage;
#[cfg(target_arch = "wasm32")]
pub use storage::backends::indexeddb_storage_backend::IndexedDBStorageBackend;
pub mod utils;
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
pub use maestro_app::browser::utils::attachment_utils::{Attachment, load_attachment};
pub use maestro_app::browser::utils::proxy_utils::{
    apply_proxy_if_needed, create_stream_fn, is_cors_error, should_use_proxy_for_provider,
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
