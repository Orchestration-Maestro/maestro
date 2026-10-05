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
    user: Map<String, Value>,
    project: Map<String, Value>,
    errors: Vec<SettingsError>,
    user_load_failed: bool,
    project_load_failed: bool,
}
impl Settings {
    /// Validates engine/manifest governance, then attempts both stored scope loads.
    /// Load failures contribute empty maps and drainable diagnostics, disabling
    /// persistence for that scope until a successful reload read. Healthy layers
    /// merge recursively and are guarded separately; governance failures return Err.
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
            user: Map::new(),
            project: Map::new(),
            errors: Vec::new(),
            user_load_failed: false,
            project_load_failed: false,
        };
        let user = settings.load_scope(SettingsScope::User).unwrap_or_default();
        let project = settings
            .load_scope(SettingsScope::Project)
            .unwrap_or_default();
        settings.snapshot = resolve_stored(&settings.baseline, &user, &project)?;
        settings.user = user;
        settings.project = project;
        Ok(settings)
    }
    /// Returns detached cached values, leaf origins and lock declarations without I/O.
    /// Unrequested external edits become visible only after successful reload.
    pub fn resolve(&self) -> SettingsSnapshot {
        self.snapshot.clone()
    }

    /// Returns a detached accepted scope, including shadowed values, without I/O.
    /// Other layers, ephemeral overrides and unrequested external edits are excluded.
    /// Session-only stored edits are included even when persistence is disabled.
    pub fn read_scope(&self, scope: SettingsScope) -> Map<String, Value> {
        match scope {
            SettingsScope::User => self.user.clone(),
            SettingsScope::Project => self.project.clone(),
        }
    }

    /// Removes accumulated scoped load failures without changing values or reloading.
    /// Draining never clears a scope's persistence latch. Synchronous mutation
    /// failures are returned directly, not queued here.
    pub fn drain_errors(&mut self) -> Vec<SettingsError> {
        std::mem::take(&mut self.errors)
    }

    fn load_scope(&mut self, scope: SettingsScope) -> Result<Map<String, Value>, SettingsError> {
        let result = self.storage.read(scope);
        match scope {
            SettingsScope::User => self.user_load_failed = result.is_err(),
            SettingsScope::Project => self.project_load_failed = result.is_err(),
        }
        result.inspect_err(|error| self.errors.push(error.clone()))
    }

    /// Recursively overlays current effective values without changing stored scopes.
    /// Supplied leaves receive the override origin; untouched origins remain.
    /// Lock rejection preserves the entire accepted snapshot.
    pub fn apply_overrides(
        &mut self,
        source: String,
        values: Map<String, Value>,
    ) -> Result<SettingsSnapshot, SettingsError> {
        let origin = SettingsOrigin::Override(source);
        let mut next = self.snapshot.clone();
        merge(&mut next, &values, origin.clone());
        guard(&self.baseline, &next.values, &origin)?;
        self.snapshot = next;
        Ok(self.resolve())
    }

    /// Replaces one literal-addressed value or subtree; an empty path requires
    /// an object and replaces the root. Missing object ancestors are created.
    /// Stored edits validate cached and fresh target maps independently, discarding
    /// overrides on success and publishing only the cached map plus this edit.
    /// A scope with a failed load changes only cached state until successful reload;
    /// otherwise only that scope is transacted, preserving unrelated external keys.
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
                let (current, other, load_failed) = match scope {
                    SettingsScope::User => (&self.user, &self.project, self.user_load_failed),
                    SettingsScope::Project => (&self.project, &self.user, self.project_load_failed),
                };
                let baseline = &self.baseline;
                let (edited, next) = edit_stored(baseline, scope, current, other, path, &value)?;
                if !load_failed {
                    self.storage.transact(scope, &mut |fresh| {
                        let (persisted, _) =
                            edit_stored(baseline, scope, fresh, other, path, &value)?;
                        Ok(Some(persisted))
                    })?;
                }
                self.snapshot = next;
                match scope {
                    SettingsScope::User => self.user = edited,
                    SettingsScope::Project => self.project = edited,
                }
            }
        }
        Ok(self.resolve())
    }
    /// Selects explicit, nonempty supplied environment, effective `sessionDir`,
    /// then selected user-root/sessions without I/O or snapshot mutation.
    /// Selected text is trimmed, home-expanded and lexically normalized at the
    /// working directory; non-string selected values fail. Raw stored text remains.
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
    /// Both reads are attempted and load errors queued in user/project order;
    /// the first error is returned. Each successful read clears its scope's
    /// persistence latch even if the other read fails. All accepted scope maps,
    /// values and overrides publish atomically only after both reads and guards
    /// succeed. Governance failure is not a load failure or queued diagnostic.
    pub fn reload(&mut self) -> Result<SettingsSnapshot, SettingsError> {
        let user = self.load_scope(SettingsScope::User);
        let project = self.load_scope(SettingsScope::Project);
        let user = user?;
        let project = project?;
        self.snapshot = resolve_stored(&self.baseline, &user, &project)?;
        self.user = user;
        self.project = project;
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

fn edit_stored(
    baseline: &SettingsSnapshot,
    scope: SettingsScope,
    current: &Map<String, Value>,
    other: &Map<String, Value>,
    path: &[String],
    value: &Value,
) -> Result<(Map<String, Value>, SettingsSnapshot), SettingsError> {
    let origin = match scope {
        SettingsScope::User => SettingsOrigin::User,
        SettingsScope::Project => SettingsOrigin::Project,
    };
    let mut edited = current.clone();
    replace(&mut edited, path, value.clone(), &origin)?;
    // Explicit ancestor replacements must contain every frozen child,
    // even when lower-layer fallback could otherwise restore it.
    for lock in &baseline.locks {
        if lock.starts_with(path) && lookup(&edited, lock) != lookup(&baseline.values, lock) {
            return Err(SettingsError::LockConflict {
                path: lock.clone(),
                locked_by: SettingsOrigin::Manifest,
                attempted_by: origin.clone(),
            });
        }
    }
    let next = match scope {
        SettingsScope::User => resolve_stored(baseline, &edited, other)?,
        SettingsScope::Project => resolve_stored(baseline, other, &edited)?,
    };
    Ok((edited, next))
}
