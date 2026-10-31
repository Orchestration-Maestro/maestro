//! Scoped raw-entry selection and identity-preserving settings edits.
use super::{
    DefaultPackageManager, InstalledSourceScope, PackageOperations,
    sources::{self, Source},
};
use maestro_resources::paths::home_directory;
use maestro_settings::{PackageSource, Settings};
use serde_json::Value;
use std::io;
impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Appends a source or replaces the first matching identity when its normalized spelling differs.
    /// # Errors
    /// Returns consumed settings, borrow or local resolution failures.
    pub(super) fn add_source(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
    ) -> io::Result<bool> {
        self.record_source(source, scope)
    }
    /// Edits only the first matching entry, preserving unread raw fields and list order.
    pub(super) fn record_source(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
    ) -> io::Result<bool> {
        let scope = scope.unwrap_or(InstalledSourceScope::User);
        let snapshot = self.snapshot(scope)?;
        let normalized = self.normalize_source(source, scope)?;
        let mut entries = packages(snapshot)?;
        let mut found = None;
        for (index, entry) in entries.iter().enumerate() {
            let stored = entry_source(entry)?;
            if !self.matches(stored, source, scope)? {
                continue;
            }
            if stored == normalized {
                return Ok(false);
            }
            found = Some(index);
            break;
        }
        if let Some(index) = found {
            match &mut entries[index] {
                Value::Object(object) => {
                    object.insert("source".into(), Value::String(normalized));
                }
                entry => *entry = Value::String(normalized),
            }
        } else {
            entries.push(Value::String(normalized));
        }
        let next = entries.into_iter().map(PackageSource::Unknown).collect();
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
    /// Selects original project-first entries by lexical package identity.
    pub(super) fn selected_package_entries<'a>(
        &self,
        user: &'a Settings,
        project: &'a Settings,
    ) -> io::Result<Vec<(InstalledSourceScope, &'a Value)>> {
        let mut selected = Vec::new();
        let mut identities = std::collections::HashSet::new();
        let entries = package_rows(project)?
            .iter()
            .map(|entry| (InstalledSourceScope::Project, entry))
            .chain(
                package_rows(user)?
                    .iter()
                    .map(|entry| (InstalledSourceScope::User, entry)),
            );
        for (scope, entry) in entries {
            if identities.insert(identity(entry_source(entry)?, &self.base(scope))?) {
                selected.push((scope, entry));
            }
        }
        Ok(selected)
    }
    /// Takes a scoped owned snapshot before any adapter call.
    pub(super) fn snapshot(&self, scope: InstalledSourceScope) -> io::Result<Settings> {
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
        let stored = identity(stored, &self.base(scope))?;
        let input = identity(input, &self.options.cwd)?;
        Ok(stored == input)
    }
}
/// Kind-specific source identity, independent of clone address and pins.
#[derive(PartialEq, Eq, Hash)]
pub(super) enum Identity<'a> {
    /// The package name.
    Npm(&'a str),
    /// The repository host and path.
    Git(String, String),
    /// The resolved local location.
    Local(String),
}
/// Consumes the packages root without inspecting later entries.
pub(super) fn packages(mut snapshot: Settings) -> io::Result<Vec<Value>> {
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
pub(super) fn entry_source(entry: &Value) -> io::Result<&str> {
    entry
        .as_str()
        .or_else(|| entry.as_object()?.get("source")?.as_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "package source is not a string"))
}

/// Borrows the configured package root without consuming filter fields.
fn package_rows(snapshot: &Settings) -> io::Result<&[Value]> {
    match snapshot.0.get("packages") {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(entries)) => Ok(entries),
        Some(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "packages is not an array",
        )),
    }
}

/// Keeps the source kind separate from its identity string.
pub(super) fn identity<'a>(source: &'a str, base: &str) -> io::Result<Identity<'a>> {
    Ok(match sources::parse(source) {
        Source::Local(path) => {
            Identity::Local(super::paths::resolve_local(path, base, home_directory)?)
        }
        Source::Git(git) => Identity::Git(git.host, git.path),
        Source::Npm { name, .. } => Identity::Npm(name),
    })
}

/// Projects the same kind-specific identity used by precedence selection.
pub(super) fn identity_text(source: &str, base: &str) -> io::Result<String> {
    Ok(match identity(source, base)? {
        Identity::Npm(name) => format!("npm:{name}"),
        Identity::Git(host, path) => format!("git:{host}/{path}"),
        Identity::Local(path) => format!("local:{path}"),
    })
}
