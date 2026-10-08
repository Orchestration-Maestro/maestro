//! Typed reads and scoped edits of the individual preferences.

use serde_json::{Map, Value};

use super::entries::SettingsListEntry;
use super::records::{
    Member, ResolvedBranchSummarySettings, ResolvedCompactionSettings,
    ResolvedProviderRetrySettings, ResolvedRetrySettings, ThinkingBudgetsSettings, get_member,
};
use super::vocabulary::{MessageDeliveryMode, ThinkingLevel, TransportSetting};
use super::{Change, SettingsManager, SettingsScope};

impl SettingsManager {
    /// Borrows a top-level string.
    pub(super) fn text(&self, field: &str) -> Option<&str> {
        self.effective.get(field).and_then(Value::as_str)
    }

    /// Reads a top-level boolean.
    pub(super) fn flag(&self, field: &str) -> Option<bool> {
        self.effective.get(field).and_then(Value::as_bool)
    }

    /// Reads a top-level list; a wrong-typed value reads as absent.
    pub(super) fn entries(&self, field: &str) -> Option<Vec<SettingsListEntry>> {
        self.effective.get(field).and_then(Member::read)
    }

    /// Borrows a top-level object; a wrong-typed value reads as absent.
    pub(super) fn object(&self, field: &str) -> Option<&Map<String, Value>> {
        self.effective.get(field).and_then(Value::as_object)
    }

    /// Reads a typed key of a top-level object; a wrong-typed value reads as absent.
    pub(super) fn nested<T: Member>(&self, field: &str, key: &str) -> Option<T> {
        get_member(self.object(field)?, key)
    }

    /// Sets a global top-level member.
    pub(super) fn set_global(&mut self, field: &'static str, value: impl Into<Value>) {
        self.edit(
            SettingsScope::Global,
            vec![Change::field(field, Some(value.into()))],
        );
    }

    /// Sets a global top-level member, or removes it for `None`.
    pub(super) fn set_global_optional(&mut self, field: &'static str, value: Option<Value>) {
        self.edit(SettingsScope::Global, vec![Change::field(field, value)]);
    }

    /// Sets one key of a global top-level object member.
    pub(super) fn set_global_key(
        &mut self,
        field: &'static str,
        key: &'static str,
        value: impl Into<Value>,
    ) {
        self.edit(
            SettingsScope::Global,
            vec![Change::key(field, key, value.into())],
        );
    }

    /// Returns the last changelog version shown, if any.
    #[must_use]
    pub fn get_last_changelog_version(&self) -> Option<String> {
        self.text("lastChangelogVersion").map(str::to_owned)
    }

    /// Remembers the last changelog version shown.
    pub fn set_last_changelog_version(&mut self, value: String) {
        self.set_global("lastChangelogVersion", value);
    }

    /// Returns the default provider, if any.
    #[must_use]
    pub fn get_default_provider(&self) -> Option<String> {
        self.text("defaultProvider").map(str::to_owned)
    }

    /// Returns the default model, if any.
    #[must_use]
    pub fn get_default_model(&self) -> Option<String> {
        self.text("defaultModel").map(str::to_owned)
    }

    /// Sets the default provider.
    pub fn set_default_provider(&mut self, value: String) {
        self.set_global("defaultProvider", value);
    }

    /// Sets the default model.
    pub fn set_default_model(&mut self, value: String) {
        self.set_global("defaultModel", value);
    }

    /// Sets the default provider and model with one queued save.
    pub fn set_default_model_and_provider(&mut self, provider: String, model_id: String) {
        let changes = vec![
            Change::field("defaultProvider", Some(provider.into())),
            Change::field("defaultModel", Some(model_id.into())),
        ];
        self.edit(SettingsScope::Global, changes);
    }

    /// Returns how steering messages are delivered; empty or wrong-typed values read as the default.
    #[must_use]
    pub fn get_steering_mode(&self) -> MessageDeliveryMode {
        self.delivery_mode("steeringMode")
    }

    /// Sets how steering messages are delivered.
    pub fn set_steering_mode(&mut self, value: MessageDeliveryMode) {
        self.set_global("steeringMode", value);
    }

    /// Returns how follow-up messages are delivered; empty or wrong-typed values read as the default.
    #[must_use]
    pub fn get_follow_up_mode(&self) -> MessageDeliveryMode {
        self.delivery_mode("followUpMode")
    }

    /// Sets how follow-up messages are delivered.
    pub fn set_follow_up_mode(&mut self, value: MessageDeliveryMode) {
        self.set_global("followUpMode", value);
    }

    /// Reads a delivery mode, falling back to one at a time.
    fn delivery_mode(&self, field: &str) -> MessageDeliveryMode {
        self.text(field).filter(|text| !text.is_empty()).map_or(
            MessageDeliveryMode::OneAtATime,
            MessageDeliveryMode::from_text,
        )
    }

    /// Returns the theme name, if any.
    #[must_use]
    pub fn get_theme(&self) -> Option<String> {
        self.text("theme").map(str::to_owned)
    }

    /// Sets the theme name.
    pub fn set_theme(&mut self, value: String) {
        self.set_global("theme", value);
    }

    /// Returns the default thinking level, if any.
    #[must_use]
    pub fn get_default_thinking_level(&self) -> Option<ThinkingLevel> {
        self.text("defaultThinkingLevel")
            .map(ThinkingLevel::from_text)
    }

    /// Sets the default thinking level.
    pub fn set_default_thinking_level(&mut self, value: ThinkingLevel) {
        self.set_global("defaultThinkingLevel", value);
    }

    /// Returns the transport; unset or wrong-typed values read as automatic.
    #[must_use]
    pub fn get_transport(&self) -> TransportSetting {
        self.text("transport")
            .map_or(TransportSetting::Auto, TransportSetting::from_text)
    }

    /// Sets the transport.
    pub fn set_transport(&mut self, value: TransportSetting) {
        self.set_global("transport", value);
    }

    /// Returns whether automatic compaction is enabled (default true).
    #[must_use]
    pub fn get_compaction_enabled(&self) -> bool {
        self.nested("compaction", "enabled").unwrap_or(true)
    }

    /// Enables or disables automatic compaction.
    pub fn set_compaction_enabled(&mut self, value: bool) {
        self.set_global_key("compaction", "enabled", value);
    }

    /// Returns the tokens reserved for compaction (default 16384).
    #[must_use]
    pub fn get_compaction_reserve_tokens(&self) -> f64 {
        self.nested("compaction", "reserveTokens")
            .unwrap_or(16384.0)
    }

    /// Returns the recent tokens kept by compaction (default 20000).
    #[must_use]
    pub fn get_compaction_keep_recent_tokens(&self) -> f64 {
        self.nested("compaction", "keepRecentTokens")
            .unwrap_or(20000.0)
    }

    /// Returns every compaction preference with its default applied.
    #[must_use]
    pub fn get_compaction_settings(&self) -> ResolvedCompactionSettings {
        ResolvedCompactionSettings {
            enabled: self.get_compaction_enabled(),
            reserve_tokens: self.get_compaction_reserve_tokens(),
            keep_recent_tokens: self.get_compaction_keep_recent_tokens(),
        }
    }

    /// Returns every branch summary preference with its default applied.
    #[must_use]
    pub fn get_branch_summary_settings(&self) -> ResolvedBranchSummarySettings {
        ResolvedBranchSummarySettings {
            reserve_tokens: self
                .nested("branchSummary", "reserveTokens")
                .unwrap_or(16384.0),
            skip_prompt: self.nested("branchSummary", "skipPrompt").unwrap_or(false),
        }
    }

    /// Returns whether the branch summary prompt is skipped (default false).
    #[must_use]
    pub fn get_branch_summary_skip_prompt(&self) -> bool {
        self.nested("branchSummary", "skipPrompt").unwrap_or(false)
    }

    /// Returns whether failed requests are retried (default true).
    #[must_use]
    pub fn get_retry_enabled(&self) -> bool {
        self.nested("retry", "enabled").unwrap_or(true)
    }

    /// Enables or disables automatic retry.
    pub fn set_retry_enabled(&mut self, value: bool) {
        self.set_global_key("retry", "enabled", value);
    }

    /// Returns every retry preference with its default applied.
    #[must_use]
    pub fn get_retry_settings(&self) -> ResolvedRetrySettings {
        ResolvedRetrySettings {
            enabled: self.get_retry_enabled(),
            max_retries: self.nested("retry", "maxRetries").unwrap_or(3.0),
            base_delay_ms: self.nested("retry", "baseDelayMs").unwrap_or(2000.0),
        }
    }

    /// Returns the provider retry preferences; the delay ceiling defaults to 60000.
    #[must_use]
    pub fn get_provider_retry_settings(&self) -> ResolvedProviderRetrySettings {
        let provider = self
            .object("retry")
            .and_then(|retry| retry.get("provider"))
            .and_then(Value::as_object);
        let member = |key| provider.and_then(|provider| get_member::<f64>(provider, key));
        ResolvedProviderRetrySettings {
            timeout_ms: member("timeoutMs"),
            max_retries: member("maxRetries"),
            max_retry_delay_ms: member("maxRetryDelayMs").unwrap_or(60000.0),
        }
    }

    /// Returns whether thinking blocks are hidden (default false).
    #[must_use]
    pub fn get_hide_thinking_block(&self) -> bool {
        self.flag("hideThinkingBlock").unwrap_or(false)
    }

    /// Hides or shows thinking blocks.
    pub fn set_hide_thinking_block(&mut self, value: bool) {
        self.set_global("hideThinkingBlock", value);
    }

    /// Returns the custom shell path, if any.
    #[must_use]
    pub fn get_shell_path(&self) -> Option<String> {
        self.text("shellPath").map(str::to_owned)
    }

    /// Sets the custom shell path; `None` removes it.
    pub fn set_shell_path(&mut self, value: Option<String>) {
        self.set_global_optional("shellPath", value.map(Value::from));
    }

    /// Returns whether startup output is quiet (default false).
    #[must_use]
    pub fn get_quiet_startup(&self) -> bool {
        self.flag("quietStartup").unwrap_or(false)
    }

    /// Enables or disables quiet startup.
    pub fn set_quiet_startup(&mut self, value: bool) {
        self.set_global("quietStartup", value);
    }

    /// Returns the prefix prepended to every shell command, if any.
    #[must_use]
    pub fn get_shell_command_prefix(&self) -> Option<String> {
        self.text("shellCommandPrefix").map(str::to_owned)
    }

    /// Sets the shell command prefix; `None` removes it.
    pub fn set_shell_command_prefix(&mut self, value: Option<String>) {
        self.set_global_optional("shellCommandPrefix", value.map(Value::from));
    }

    /// Returns the package-manager command as an argument vector, if any.
    #[must_use]
    pub fn get_npm_command(&self) -> Option<Vec<SettingsListEntry>> {
        self.entries("npmCommand")
    }

    /// Sets the package-manager command; `None` removes it.
    pub fn set_npm_command(&mut self, value: Option<Vec<SettingsListEntry>>) {
        self.set_global_optional("npmCommand", value.map(Member::write));
    }

    /// Returns whether the changelog is shown condensed (default false).
    #[must_use]
    pub fn get_collapse_changelog(&self) -> bool {
        self.flag("collapseChangelog").unwrap_or(false)
    }

    /// Shows the changelog condensed or in full.
    pub fn set_collapse_changelog(&mut self, value: bool) {
        self.set_global("collapseChangelog", value);
    }

    /// Returns whether the install telemetry preference is enabled (default true).
    #[must_use]
    pub fn get_enable_install_telemetry(&self) -> bool {
        self.flag("enableInstallTelemetry").unwrap_or(true)
    }

    /// Enables or disables the install telemetry preference.
    pub fn set_enable_install_telemetry(&mut self, value: bool) {
        self.set_global("enableInstallTelemetry", value);
    }

    /// Returns the model patterns used for cycling, if any.
    #[must_use]
    pub fn get_enabled_models(&self) -> Option<Vec<SettingsListEntry>> {
        self.entries("enabledModels")
    }

    /// Sets the model patterns used for cycling; `None` removes them.
    pub fn set_enabled_models(&mut self, value: Option<Vec<SettingsListEntry>>) {
        self.set_global_optional("enabledModels", value.map(Member::write));
    }

    /// Returns whether skills register as commands (default true).
    #[must_use]
    pub fn get_enable_skill_commands(&self) -> bool {
        self.flag("enableSkillCommands").unwrap_or(true)
    }

    /// Enables or disables skill commands.
    pub fn set_enable_skill_commands(&mut self, value: bool) {
        self.set_global("enableSkillCommands", value);
    }

    /// Returns the custom thinking budgets, if configured.
    #[must_use]
    pub fn get_thinking_budgets(&self) -> Option<ThinkingBudgetsSettings> {
        self.object("thinkingBudgets")
            .cloned()
            .map(ThinkingBudgetsSettings)
    }
}
