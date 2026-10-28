//! Source dispatch for explicit install and removal, and settings edits after success.
use super::{
    DefaultPackageManager, InstalledSourceScope, PackageOperations, ProgressAction,
    decisions::SourceEditEffect,
    sources::{self, Source},
};
use std::{io, path::Path};

/// Whether acquisition changed an existing Git commit.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceAcquisition {
    /// Prepared contents or a non-Git acquisition.
    Prepared,
    /// Git acquisition retained the existing revision.
    CurrentGit,
}

impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Acquires or validates the source's contents; `None` means user scope.
    pub(super) async fn acquire(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
        prepared: Option<(maestro_settings::PreparedSettingsEdit, SourceEditEffect)>,
    ) -> io::Result<SourceAcquisition> {
        let scope = scope.unwrap_or(InstalledSourceScope::User);
        if prepared.is_none() {
            self.check_package_operation(source, Some(scope), false)?;
        }
        let parsed = sources::parse(source);
        let message = format!("Installing {source}...");
        let acquisition = self
            .with_progress(ProgressAction::Install, source, message, async {
                match parsed {
                    Source::Npm { spec, .. } => self
                        .install_npm(spec, scope.resource_scope())
                        .await
                        .map(|()| SourceAcquisition::Prepared),
                    Source::Git(git) => self.install_git(&git, scope.resource_scope()).await,
                    Source::Local(path) => self
                        .require_local(path)
                        .map(|()| SourceAcquisition::Prepared),
                }
            })
            .await?;
        if let Some((edit, effect)) = prepared
            && (acquisition != SourceAcquisition::CurrentGit
                || effect != SourceEditEffect::Replacement)
        {
            self.options
                .settings_manager
                .borrow_mut()
                .apply_prepared_edit(edit);
        }
        Ok(acquisition)
    }
    /// Fails unless the local path, resolved against the input base, exists.
    fn require_local(&self, path: &str) -> io::Result<()> {
        let resolved =
            super::paths::resolve_local(path, &self.options.cwd, super::operations::home_dir)?;
        if Path::new(&resolved).exists() {
            return Ok(());
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Path does not exist: {resolved}"),
        ))
    }
    /// Removes the selected managed contents; local sources have none to remove.
    pub(super) async fn release(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
        prepared: Option<(maestro_settings::PreparedSettingsEdit, SourceEditEffect)>,
    ) -> io::Result<()> {
        let scope = scope.unwrap_or(InstalledSourceScope::User);
        if prepared.is_none() {
            self.check_package_operation(source, Some(scope), true)?;
        }
        let parsed = sources::parse(source);
        let message = format!("Removing {source}...");
        self.with_progress(ProgressAction::Remove, source, message, async {
            match parsed {
                Source::Npm { name, .. } => self.uninstall_npm(name, scope).await,
                Source::Git(git) => self.remove_git(&git, scope),
                Source::Local(_) => Ok(()),
            }
        })
        .await?;
        if let Some((edit, _)) = prepared {
            self.options
                .settings_manager
                .borrow_mut()
                .apply_prepared_edit(edit);
        }
        Ok(())
    }
    /// Installs, then adds the source to the selected scope's settings.
    pub(super) async fn acquire_and_persist(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
    ) -> io::Result<()> {
        let prepared = self.prepare_source_edit(source, scope, false)?;
        let admitted = prepared.is_some();
        let acquisition = self.acquire(source, scope, prepared).await?;
        if admitted {
            Ok(())
        } else {
            self.record_source(source, scope, acquisition).map(drop)
        }
    }
    /// Removes the contents, then removes the source from settings.
    pub(super) async fn release_and_persist(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
    ) -> io::Result<bool> {
        let prepared = self.prepare_source_edit(source, scope, true)?;
        let modified = prepared
            .as_ref()
            .map(|(_, effect)| *effect != SourceEditEffect::Unchanged);
        self.release(source, scope, prepared).await?;
        modified.map_or_else(|| self.remove_source(source, scope), Ok)
    }
}
