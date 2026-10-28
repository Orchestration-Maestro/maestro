//! Preference admission before explicit package acquisition or removal.
use super::{
    DefaultPackageManager, InstalledSourceScope, PackageOperations, acquisition::SourceAcquisition,
};
use maestro_settings::{DecisionRequest, PreparedSettingsEdit, SettingsChange, SettingsScope};
use std::io;

impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Stages exact edits only when admission is installed, preserving ordinary late reads.
    pub(super) fn prepare_source_edit(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
        remove: bool,
    ) -> io::Result<Option<(PreparedSettingsEdit, bool)>> {
        if !self.options.settings_manager.borrow().has_decider() {
            return Ok(None);
        }
        let scope = scope.unwrap_or(InstalledSourceScope::User);
        let entries = if remove {
            self.source_removal(source, scope)?
        } else {
            self.source_addition(source, scope, SourceAcquisition::Prepared)?
        };
        let modified = entries.is_some();
        let changes = entries.map_or_else(Vec::new, |entries| {
            vec![SettingsChange {
                path: vec!["packages".into()],
                value: Some(serde_json::Value::Array(
                    entries
                        .into_iter()
                        .map(maestro_settings::PackageSource::into_value)
                        .collect(),
                )),
            }]
        });
        let scope = match scope {
            InstalledSourceScope::User => SettingsScope::Global,
            InstalledSourceScope::Project => SettingsScope::Project,
        };
        let settings = self.options.settings_manager.borrow();
        let request = DecisionRequest::SettingsEdit {
            scope,
            changes: &changes,
        };
        let edit = settings
            .prepare_operation(&request, scope, changes.clone())
            .map_err(|message| {
                settings.report_refusal(scope, message.clone());
                io::Error::new(io::ErrorKind::PermissionDenied, message)
            })?;
        Ok(Some((edit, modified)))
    }
}
