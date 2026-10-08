//! Package, resource-list and warning preferences for both scopes.

use serde_json::Value;

use super::entries::{PackageSource, SettingsListEntry, write_entries};
use super::records::{Sparse, WarningSettings};
use super::{Change, SettingsManager, SettingsScope};

impl SettingsManager {
    /// Reads a resource list; an unset or wrong-typed value reads as empty.
    fn paths(&self, field: &str) -> Vec<SettingsListEntry> {
        self.entries(field).unwrap_or_default()
    }

    /// Replaces a resource list in one scope.
    fn set_list(&mut self, scope: SettingsScope, field: &'static str, value: Value) {
        self.edit(scope, vec![Change::field(field, Some(value))]);
    }

    /// Returns the configured package sources, project entries replacing global ones.
    #[must_use]
    pub fn get_packages(&self) -> Vec<PackageSource> {
        let stored = self.effective.get("packages").and_then(Value::as_array);
        stored.map_or_else(Vec::new, |items| {
            items.iter().map(PackageSource::from_value).collect()
        })
    }

    /// Replaces the global package sources.
    pub fn set_packages(&mut self, value: Vec<PackageSource>) {
        self.set_list(SettingsScope::Global, "packages", packages_value(value));
    }

    /// Replaces the project package sources.
    pub fn set_project_packages(&mut self, value: Vec<PackageSource>) {
        self.set_list(SettingsScope::Project, "packages", packages_value(value));
    }

    /// Returns the configured extension paths.
    #[must_use]
    pub fn get_extension_paths(&self) -> Vec<SettingsListEntry> {
        self.paths("extensions")
    }

    /// Replaces the global extension paths.
    pub fn set_extension_paths(&mut self, value: Vec<SettingsListEntry>) {
        self.set_list(SettingsScope::Global, "extensions", write_entries(value));
    }

    /// Replaces the project extension paths.
    pub fn set_project_extension_paths(&mut self, value: Vec<SettingsListEntry>) {
        self.set_list(SettingsScope::Project, "extensions", write_entries(value));
    }

    /// Returns the configured skill paths.
    #[must_use]
    pub fn get_skill_paths(&self) -> Vec<SettingsListEntry> {
        self.paths("skills")
    }

    /// Replaces the global skill paths.
    pub fn set_skill_paths(&mut self, value: Vec<SettingsListEntry>) {
        self.set_list(SettingsScope::Global, "skills", write_entries(value));
    }

    /// Replaces the project skill paths.
    pub fn set_project_skill_paths(&mut self, value: Vec<SettingsListEntry>) {
        self.set_list(SettingsScope::Project, "skills", write_entries(value));
    }

    /// Returns the configured prompt template paths.
    #[must_use]
    pub fn get_prompt_template_paths(&self) -> Vec<SettingsListEntry> {
        self.paths("prompts")
    }

    /// Replaces the global prompt template paths.
    pub fn set_prompt_template_paths(&mut self, value: Vec<SettingsListEntry>) {
        self.set_list(SettingsScope::Global, "prompts", write_entries(value));
    }

    /// Replaces the project prompt template paths.
    pub fn set_project_prompt_template_paths(&mut self, value: Vec<SettingsListEntry>) {
        self.set_list(SettingsScope::Project, "prompts", write_entries(value));
    }

    /// Returns the configured theme paths.
    #[must_use]
    pub fn get_theme_paths(&self) -> Vec<SettingsListEntry> {
        self.paths("themes")
    }

    /// Replaces the global theme paths.
    pub fn set_theme_paths(&mut self, value: Vec<SettingsListEntry>) {
        self.set_list(SettingsScope::Global, "themes", write_entries(value));
    }

    /// Replaces the project theme paths.
    pub fn set_project_theme_paths(&mut self, value: Vec<SettingsListEntry>) {
        self.set_list(SettingsScope::Project, "themes", write_entries(value));
    }

    /// Returns the warning preferences; unset members stay unset.
    #[must_use]
    pub fn get_warnings(&self) -> WarningSettings {
        self.record("warnings")
    }

    /// Replaces the global warning preferences.
    pub fn set_warnings(&mut self, value: WarningSettings) {
        self.set_global("warnings", Value::Object(value.into_map()));
    }
}

/// Writes package sources as a JSON array.
fn packages_value(sources: Vec<PackageSource>) -> Value {
    Value::Array(sources.into_iter().map(PackageSource::into_value).collect())
}
