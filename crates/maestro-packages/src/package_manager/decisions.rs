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
        let scope = scope.unwrap_or(InstalledSourceScope::User);
        {
            let settings = self.options.settings_manager.borrow();
            let layer = match scope {
                InstalledSourceScope::User => settings.get_global_base_layer(),
                InstalledSourceScope::Project => settings.get_base_layer(),
            };
            if !settings.has_decider() && layer.is_none() {
                return Ok(None);
            }
        }
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

impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Admits package intent against its scoped publisher and persisted required sources.
    pub(super) fn check_package_operation(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
        remove: bool,
    ) -> io::Result<()> {
        let scope = scope.unwrap_or(InstalledSourceScope::User);
        let settings = self.options.settings_manager.borrow();
        let (raw_scope, layer) = match scope {
            InstalledSourceScope::User => (SettingsScope::Global, settings.get_global_base_layer()),
            InstalledSourceScope::Project => (SettingsScope::Project, settings.get_base_layer()),
        };
        let result = self
            .check_package_layer(source, scope, remove, layer)
            .and_then(|()| {
                let request = if remove {
                    DecisionRequest::PackageRemove {
                        source,
                        scope: raw_scope,
                    }
                } else {
                    DecisionRequest::PackageInstall {
                        source,
                        scope: raw_scope,
                    }
                };
                settings
                    .decide(&request)
                    .map_err(|message| io::Error::new(io::ErrorKind::PermissionDenied, message))
            });
        if let Err(error) = &result {
            settings.report_refusal(raw_scope, error.to_string());
        }
        result
    }
    /// Enforces persisted package locks and compares required sources by package identity.
    fn check_package_layer(
        &self,
        source: &str,
        scope: InstalledSourceScope,
        remove: bool,
        layer: Option<&maestro_settings::BaseSettingsLayer>,
    ) -> io::Result<()> {
        let Some(layer) = layer else {
            return Ok(());
        };
        let forbidden = if remove {
            let requested = super::configuration::identity(source, &self.options.cwd)?;
            super::configuration::packages(layer.settings.clone())?
                .iter()
                .try_fold(false, |required, entry| {
                    let stored = super::configuration::entry_source(entry)?;
                    Ok::<_, io::Error>(
                        required
                            || super::configuration::identity(stored, &self.base(scope))?
                                == requested,
                    )
                })?
        } else {
            layer.is_key_path_locked("packages", &[])
        };
        if forbidden {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "Package operation refused: {source} is governed by {} (recorded {})",
                    layer.source, layer.recorded_at
                ),
            ));
        }
        Ok(())
    }
}
