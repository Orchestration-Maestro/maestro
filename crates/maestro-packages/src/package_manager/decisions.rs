//! Preference admission before explicit package acquisition or removal.
use super::{DefaultPackageManager, InstalledSourceScope, PackageOperations};
use maestro_settings::{DecisionRequest, PreparedSettingsEdit, SettingsChange, SettingsScope};
use std::io;

/// The source delta admitted before acquisition or removal.
#[derive(Clone, Copy, Debug)]
pub(super) enum SourceEdit {
    /// Publish the requested source spelling.
    Add,
    /// Remove all matching source entries.
    Remove,
}

impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Stages exact edits only when admission is installed, preserving ordinary late reads.
    pub(super) fn prepare_source_edit(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
        remove: bool,
    ) -> io::Result<Option<PreparedSettingsEdit>> {
        let scope = scope.unwrap_or(InstalledSourceScope::User);
        {
            let settings = self
                .options
                .settings_manager
                .try_borrow()
                .map_err(io::Error::other)?;
            let layer = match scope {
                InstalledSourceScope::User => settings.get_global_base_layer(),
                InstalledSourceScope::Project => settings.get_base_layer(),
            };
            if !settings.has_decider() && layer.is_none() {
                return Ok(None);
            }
        }
        self.check_package_operation(source, Some(scope), remove)?;
        let changes = vec![SettingsChange {
            path: vec!["packages".into()],
            value: Some(serde_json::Value::Array(
                self.source_edit_entries(source, scope, remove)?,
            )),
        }];
        let scope = match scope {
            InstalledSourceScope::User => SettingsScope::Global,
            InstalledSourceScope::Project => SettingsScope::Project,
        };
        let settings = self
            .options
            .settings_manager
            .try_borrow()
            .map_err(io::Error::other)?;
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
        Ok(Some(edit))
    }
}

impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Captures the intended list even when the source delta is already satisfied.
    fn source_edit_entries(
        &self,
        source: &str,
        scope: InstalledSourceScope,
        remove: bool,
    ) -> io::Result<Vec<serde_json::Value>> {
        let entries = if remove {
            self.source_removal(source, scope)?
        } else {
            self.source_addition(source, scope)?
        };
        entries.map_or_else(
            || super::configuration::packages(self.snapshot(scope)?),
            |entries| {
                Ok(entries
                    .into_iter()
                    .map(maestro_settings::PackageSource::into_value)
                    .collect())
            },
        )
    }

    /// Publishes the recomputed list for the already-admitted source delta.
    pub(super) fn publish_source_edit(
        &self,
        source: &str,
        scope: InstalledSourceScope,
        edit: PreparedSettingsEdit,
        operation: SourceEdit,
    ) -> io::Result<bool> {
        let entries = match operation {
            SourceEdit::Add => self.source_addition(source, scope)?,
            SourceEdit::Remove => self.source_removal(source, scope)?,
        };
        let Some(entries) = entries else {
            return Ok(false);
        };
        let scope = match scope {
            InstalledSourceScope::User => SettingsScope::Global,
            InstalledSourceScope::Project => SettingsScope::Project,
        };
        let edit = edit
            .with_recomputed_list(
                scope,
                &["packages"],
                entries
                    .into_iter()
                    .map(maestro_settings::PackageSource::into_value)
                    .collect(),
            )
            .map_err(io::Error::other)?;
        self.options
            .settings_manager
            .try_borrow_mut()
            .map_err(io::Error::other)?
            .apply_prepared_edit(edit);
        Ok(true)
    }

    /// Admits package intent against its scoped publisher and persisted required sources.
    pub(super) fn check_package_operation(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
        remove: bool,
    ) -> io::Result<()> {
        let scope = scope.unwrap_or(InstalledSourceScope::User);
        let settings = self
            .options
            .settings_manager
            .try_borrow()
            .map_err(io::Error::other)?;
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
