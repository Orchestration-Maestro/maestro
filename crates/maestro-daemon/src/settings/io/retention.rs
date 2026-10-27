//! Typed-area retention using the settings owner's global constraint queries.

use std::collections::BTreeMap;

use maestro_settings::BaseSettingsLayer;

use super::super::model::LoadedConfig;

/// Stored namespace and declared members of a rejected semantic area.
pub(super) struct AreaSchema {
    /// Authored namespace used by this area's decoder.
    pub root: &'static str,
    /// Declared record members, or empty for a scalar area.
    pub fields: &'static [&'static str],
}

/// Decoder metadata for rejected semantic areas.
pub(super) type AreaSchemas = BTreeMap<&'static str, AreaSchema>;

impl AreaSchema {
    /// Ask the owner whether any declared input is constrained.
    fn is_locked(&self, layer: &BaseSettingsLayer, area: &str) -> bool {
        if self.fields.is_empty() {
            return self.path_locked(layer, area, None);
        }
        self.fields
            .iter()
            .any(|key| self.path_locked(layer, area, Some(key)))
    }

    /// Keep flattened fields attached to their authored namespaces.
    fn path_locked(&self, layer: &BaseSettingsLayer, area: &str, key: Option<&str>) -> bool {
        let root = match (area, key) {
            ("update", Some("detectionCatalogCheck")) | ("experimental", Some("paneHistory")) => {
                "daemon"
            }
            _ => self.root,
        };
        let mut path = root.split('.');
        let Some(field) = path.next() else {
            return false;
        };
        let mut keys: Vec<_> = path.collect();
        keys.extend(key);
        layer.is_key_path_locked(field, &keys)
    }
}

/// Replace rejected locked areas with validated base values instead of stale values.
pub(super) fn recover_locked_areas(
    loaded: &mut LoadedConfig,
    base: LoadedConfig,
    layer: &BaseSettingsLayer,
    schemas: &AreaSchemas,
) {
    macro_rules! recover {
        ($($field:ident),* $(,)?) => { $(
            if should_recover(loaded, &base.invalid_sections, layer, schemas, stringify!($field)) {
                loaded.config.$field = base.config.$field;
                describe_recovery(loaded, stringify!($field), layer);
            }
        )* };
    }
    recover!(
        onboarding,
        theme,
        terminal,
        session,
        server,
        update,
        ui,
        worktrees,
        advanced,
        experimental,
        remote
    );
}

/// Require a rejected user area, a valid base area and an owner-reported lock.
fn should_recover(
    loaded: &LoadedConfig,
    invalid_base: &[String],
    layer: &BaseSettingsLayer,
    schemas: &AreaSchemas,
    area: &str,
) -> bool {
    loaded.invalid_sections.iter().any(|name| name == area)
        && !invalid_base.iter().any(|name| name == area)
        && schemas
            .get(area)
            .is_some_and(|schema| schema.is_locked(layer, area))
}

/// Preserve the original type error while explaining the temporary base fallback.
fn describe_recovery(loaded: &mut LoadedConfig, area: &str, layer: &BaseSettingsLayer) {
    loaded.invalid_sections.retain(|name| name != area);
    let suffix = format!("; keeping current {area} settings");
    for diagnostic in &mut loaded.diagnostics {
        if let Some(error) = diagnostic.strip_suffix(&suffix) {
            *diagnostic = format!(
                "{error}; following locked base from {} (recorded {}) until the value is fixed",
                layer.source, layer.recorded_at
            );
        }
    }
}
