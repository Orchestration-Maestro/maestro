use crate::resolve::{guard, lookup, merge, replace, replace_effective};
use crate::{
    ManifestSettings, SettingsError, SettingsLocations, SettingsOrigin, SettingsScope,
    SettingsSnapshot, SettingsStorage, SettingsTarget,
};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Owns precedence, provenance and lock enforcement above injected storage.
pub struct Settings {
    snapshot: SettingsSnapshot,
    baseline: SettingsSnapshot,
    storage: Box<dyn SettingsStorage>,
}
impl Settings {
    /// Reads four layers and validates manifest locks against engine plus manifest.
    /// Objects merge recursively; arrays, scalars and null replace. Each stored
    /// layer is checked before the next can conceal a conflict.
    pub fn new(
        engine: Map<String, Value>,
        manifest: ManifestSettings,
        storage: Box<dyn SettingsStorage>,
    ) -> Result<Self, SettingsError> {
        let mut baseline = SettingsSnapshot {
            values: Map::new(),
            origins: BTreeMap::new(),
            locks: manifest.locks,
        };
        merge(
            &mut baseline,
            &crate::defaults::defaults(),
            SettingsOrigin::Engine,
        );
        merge(&mut baseline, &engine, SettingsOrigin::Engine);
        merge(&mut baseline, &manifest.values, SettingsOrigin::Manifest);
        for lock in &baseline.locks {
            match lookup(&baseline.values, lock) {
                Err(()) => return Err(SettingsError::InvalidLock { path: lock.clone() }),
                Ok(None) => return Err(SettingsError::MissingLockTarget { path: lock.clone() }),
                Ok(Some(_)) => {}
            }
        }
        let mut settings = Self {
            snapshot: baseline.clone(),
            baseline,
            storage,
        };
        settings.reload()?;
        Ok(settings)
    }
    /// Returns detached values, leaf origins and lock declarations without I/O.
    pub fn resolve(&self) -> SettingsSnapshot {
        self.snapshot.clone()
    }

    /// Replaces one literal-addressed value or subtree; an empty path requires
    /// an object and replaces the root. Missing object ancestors are created.
    /// Stored edits use the transaction's current map and discard overrides.
    /// Overrides affect only the effective snapshot. Logical rejection preserves
    /// settings bytes; native write failures can leave partial files. All failures
    /// preserve the last accepted effective snapshot.
    pub fn set(
        &mut self,
        target: SettingsTarget,
        path: &[String],
        value: Value,
    ) -> Result<SettingsSnapshot, SettingsError> {
        match target {
            SettingsTarget::Override(label) => {
                let origin = SettingsOrigin::Override(label);
                let mut next = self.snapshot.clone();
                replace_effective(&mut next, path, value, &origin)?;
                guard(&self.baseline, &next.values, &origin)?;
                self.snapshot = next;
            }
            SettingsTarget::Stored(scope) => {
                let origin = match scope {
                    SettingsScope::User => SettingsOrigin::User,
                    SettingsScope::Project => SettingsOrigin::Project,
                };
                let other_scope = match scope {
                    SettingsScope::User => SettingsScope::Project,
                    SettingsScope::Project => SettingsScope::User,
                };
                let other = self.storage.read(other_scope)?;
                let mut accepted = None;
                let baseline = &self.baseline;
                self.storage.transact(scope, &mut |current| {
                    let mut edited = current.clone();
                    replace(&mut edited, path, value.clone(), &origin)?;
                    // An explicit ancestor replacement must contain each frozen child,
                    // even if lower-layer fallback could otherwise restore it.
                    for lock in &baseline.locks {
                        if lock.starts_with(path)
                            && lookup(&edited, lock) != lookup(&baseline.values, lock)
                        {
                            return Err(SettingsError::LockConflict {
                                path: lock.clone(),
                                locked_by: SettingsOrigin::Manifest,
                                attempted_by: origin.clone(),
                            });
                        }
                    }
                    let next = match scope {
                        SettingsScope::User => resolve_stored(baseline, &edited, &other)?,
                        SettingsScope::Project => resolve_stored(baseline, &other, &edited)?,
                    };
                    accepted = Some(next);
                    Ok(Some(edited))
                })?;
                self.snapshot = accepted.expect("successful transaction invokes the callback once");
            }
        }
        Ok(self.resolve())
    }
    /// Selects explicit, nonempty supplied environment, effective `sessionDir`,
    /// then selected user-root/sessions without I/O or snapshot mutation.
    /// Relative paths use the working directory; non-string selected values fail.
    /// Explicit and environment winners are checked as raw `cli`/`environment`
    /// overrides by the manifest guard before expansion. Empty environment input
    /// is absent; explicit/effective empty strings select the working directory.
    pub fn session_directory(
        &self,
        locations: &SettingsLocations,
        explicit: Option<&str>,
        environment: Option<&str>,
    ) -> Result<std::path::PathBuf, SettingsError> {
        let selected = explicit.or(environment.filter(|s| !s.is_empty()));
        if let Some(raw) = selected {
            let origin = SettingsOrigin::Override(
                if explicit.is_some() {
                    "cli"
                } else {
                    "environment"
                }
                .into(),
            );
            let mut candidate = self.snapshot.clone();
            replace_effective(
                &mut candidate,
                &["sessionDir".into()],
                Value::String(raw.into()),
                &origin,
            )?;
            guard(&self.baseline, &candidate.values, &origin)?;
            return Ok(locations.invocation_path(std::path::Path::new(raw)));
        }
        match self.snapshot.values.get("sessionDir") {
            Some(Value::String(raw)) => Ok(locations.invocation_path(std::path::Path::new(raw))),
            Some(_) => Err(SettingsError::Location {
                input: "sessionDir",
                origin: self
                    .snapshot
                    .origins
                    .iter()
                    .filter(|(path, _)| path.first().is_some_and(|segment| segment == "sessionDir"))
                    .max_by_key(|(_, origin)| match origin {
                        SettingsOrigin::Engine => 0,
                        SettingsOrigin::Manifest => 1,
                        SettingsOrigin::User => 2,
                        SettingsOrigin::Project => 3,
                        SettingsOrigin::Override(_) => 4,
                    })
                    .expect("present settings value has a leaf origin")
                    .1
                    .clone(),
            }),
            None => Ok(locations
                .configuration_directory(SettingsScope::User)
                .join("sessions")),
        }
    }

    /// Re-reads both stored scopes and discards overrides only after validation.
    /// Failure preserves the last usable snapshot; memory state is not reset.
    pub fn reload(&mut self) -> Result<SettingsSnapshot, SettingsError> {
        let user = self.storage.read(SettingsScope::User)?;
        let project = self.storage.read(SettingsScope::Project)?;
        self.snapshot = resolve_stored(&self.baseline, &user, &project)?;
        Ok(self.resolve())
    }
}
fn resolve_stored(
    baseline: &SettingsSnapshot,
    user: &Map<String, Value>,
    project: &Map<String, Value>,
) -> Result<SettingsSnapshot, SettingsError> {
    let mut next = baseline.clone();
    for (layer, origin) in [
        (user, SettingsOrigin::User),
        (project, SettingsOrigin::Project),
    ] {
        merge(&mut next, layer, origin.clone());
        guard(baseline, &next.values, &origin)?;
    }
    Ok(next)
}
