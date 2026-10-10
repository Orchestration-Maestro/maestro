//! Scoped raw-entry selection and identity-preserving settings edits.
use super::{
    DefaultPackageManager, InstalledSourceScope, PackageOperations,
    sources::{self, Source},
};
use maestro_settings::{PackageSource, Settings};
use serde_json::Value;
use std::io;
impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Appends one normalized source unless the chosen scope already contains its identity.
    /// # Errors
    /// Returns consumed settings, borrow or local resolution failures.
    pub(super) fn add_source(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
    ) -> io::Result<bool> {
        let scope = scope.unwrap_or(InstalledSourceScope::User);
        let snapshot = self.snapshot(scope)?;
        let normalized = self.normalize_source(source, scope)?;
        let entries = packages(snapshot)?;
        for entry in &entries {
            if self.matches(entry_source(entry)?, source, scope)? {
                return Ok(false);
            }
        }
        let mut next: Vec<_> = entries.into_iter().map(PackageSource::Unknown).collect();
        next.push(PackageSource::Source(normalized));
        self.write(scope, next)?;
        Ok(true)
    }
    /// Removes every matching entry after examining the complete selected list.
    /// # Errors
    /// Returns consumed settings, borrow or local resolution failures.
    pub(super) fn remove_source(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
    ) -> io::Result<bool> {
        let scope = scope.unwrap_or(InstalledSourceScope::User);
        let entries = packages(self.snapshot(scope)?)?;
        let mut next = Vec::new();
        let mut changed = false;
        for entry in entries {
            if self.matches(entry_source(&entry)?, source, scope)? {
                changed = true;
            } else {
                next.push(PackageSource::Unknown(entry));
            }
        }
        if changed {
            self.write(scope, next)?;
        }
        Ok(changed)
    }
    /// Lists original rows in user-then-project order with per-row content lookup.
    /// # Errors
    /// Returns a consumed entry, borrow, path or root lookup failure.
    pub(super) fn configured_packages(&self) -> io::Result<Vec<super::ConfiguredPackage>> {
        let snapshots = {
            let settings = self
                .options
                .settings_manager
                .try_borrow()
                .map_err(io::Error::other)?;
            [
                settings.get_global_settings(),
                settings.get_project_settings(),
            ]
        };
        let mut result = Vec::new();
        for (scope, snapshot) in [InstalledSourceScope::User, InstalledSourceScope::Project]
            .into_iter()
            .zip(snapshots)
        {
            for entry in packages(snapshot)? {
                let source = entry_source(&entry)?;
                let installed_path = self.installed_path(source, scope)?;
                result.push(super::ConfiguredPackage {
                    source: source.to_owned(),
                    scope,
                    filtered: entry.is_object(),
                    installed_path,
                });
            }
        }
        Ok(result)
    }
    /// Takes a scoped owned snapshot before any adapter call.
    fn snapshot(&self, scope: InstalledSourceScope) -> io::Result<Settings> {
        let settings = self
            .options
            .settings_manager
            .try_borrow()
            .map_err(io::Error::other)?;
        Ok(match scope {
            InstalledSourceScope::User => settings.get_global_settings(),
            InstalledSourceScope::Project => settings.get_project_settings(),
        })
    }
    /// Publishes an edit through the existing scoped setter.
    fn write(&self, scope: InstalledSourceScope, entries: Vec<PackageSource>) -> io::Result<()> {
        let mut settings = self
            .options
            .settings_manager
            .try_borrow_mut()
            .map_err(io::Error::other)?;
        match scope {
            InstalledSourceScope::User => settings.set_packages(entries),
            InstalledSourceScope::Project => settings.set_project_packages(entries),
        }
        Ok(())
    }
    /// Compares stored scope-relative identity before input cwd-relative identity.
    fn matches(&self, stored: &str, input: &str, scope: InstalledSourceScope) -> io::Result<bool> {
        let stored = self.identity(stored, &self.base(scope))?;
        let input = self.identity(input, &self.options.cwd)?;
        Ok(stored == input)
    }
    /// Keeps the source kind separate from its identity string.
    fn identity<'a>(&self, source: &'a str, base: &str) -> io::Result<Identity<'a>> {
        Ok(match sources::parse(source) {
            Source::Local(path) => Identity::Local(self.resolve_local(path, base)?),
            Source::Git(git) => Identity::Git(git.host, git.path),
            Source::Npm { name, .. } => Identity::Npm(name),
        })
    }
}
/// Kind-specific source identity, independent of clone address and pins.
#[derive(PartialEq, Eq)]
enum Identity<'a> {
    /// The package name.
    Npm(&'a str),
    /// The repository host and path.
    Git(String, String),
    /// The resolved local location.
    Local(String),
}
/// Consumes the packages root without inspecting later entries.
fn packages(mut snapshot: Settings) -> io::Result<Vec<Value>> {
    match snapshot.0.shift_remove("packages") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(entries)) => Ok(entries),
        Some(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "packages is not an array",
        )),
    }
}
/// Reads only the source of the currently consumed entry.
fn entry_source(entry: &Value) -> io::Result<&str> {
    entry
        .as_str()
        .or_else(|| entry.as_object()?.get("source")?.as_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "package source is not a string"))
}
