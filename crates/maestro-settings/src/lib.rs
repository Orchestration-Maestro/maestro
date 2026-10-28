#![doc = include_str!("../../../docs/settings.md")]

pub mod settings_manager;

#[cfg(not(target_arch = "wasm32"))]
pub use settings_manager::FileSettingsStorage;
pub use settings_manager::{
    BaseSettingsLayer, BranchSummarySettings, CompactionSettings, DecisionRequest,
    DoubleEscapeAction, FilteredPackage, ImageSettings, InMemorySettingsStorage, MarkdownSettings,
    MessageDeliveryMode, PackageSource, PreparedSettingsEdit, ProviderRetrySettings,
    ResolvedBranchSummarySettings, ResolvedCompactionSettings, ResolvedProviderRetrySettings,
    ResolvedRetrySettings, RetrySettings, Settings, SettingsChange, SettingsDecider,
    SettingsDeciderHandle, SettingsError, SettingsListEntry, SettingsManager, SettingsScope,
    SettingsStorage, SettingsStorageError, SettingsStorageHandle, SettingsUpdate, TerminalSettings,
    ThinkingBudgetsSettings, ThinkingLevel, TransportSetting, TreeFilterMode, WarningSettings,
};
