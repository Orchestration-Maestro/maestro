#![doc = include_str!("../../../docs/settings.md")]

mod settings_manager;

#[cfg(not(target_arch = "wasm32"))]
pub use settings_manager::FileSettingsStorage;
pub use settings_manager::{
    BranchSummarySettings, CompactionSettings, DoubleEscapeAction, ImageSettings,
    InMemorySettingsStorage, MarkdownSettings, MessageDeliveryMode, PackageFilters, PackageSource,
    ProviderRetrySettings, ResolvedBranchSummarySettings, ResolvedCompactionSettings,
    ResolvedProviderRetrySettings, ResolvedRetrySettings, RetrySettings, Settings, SettingsError,
    SettingsListEntry, SettingsManager, SettingsScope, SettingsStorage, SettingsStorageError,
    SettingsStorageHandle, SettingsUpdate, TerminalSettings, ThinkingBudgetsSettings,
    ThinkingLevel, TransportSetting, TreeFilterMode, WarningSettings,
};
