//! Typed-area retention using the settings owner's global constraint queries.

use std::collections::BTreeMap;

use maestro_settings::BaseSettingsLayer;

use super::{super::model::LoadedConfig, FieldSource};

/// Declared decoder sources for rejected semantic areas.
pub(super) type AreaSchemas = BTreeMap<&'static str, Vec<FieldSource>>;

/// Ask the owner about exactly the sources consumed by the decoder.
fn sources_locked(layer: &BaseSettingsLayer, sources: &[FieldSource]) -> bool {
    sources.iter().any(|source| {
        if source.fields.is_empty() {
            return path_locked(layer, source.root, None);
        }
        source
            .fields
            .iter()
            .any(|key| path_locked(layer, source.root, Some(key)))
    })
}

/// Turn a declared source location into the owner's literal-path query.
fn path_locked(layer: &BaseSettingsLayer, root: &str, key: Option<&str>) -> bool {
    let mut path = root.split('.');
    let Some(field) = path.next() else {
        return false;
    };
    let mut keys: Vec<_> = path.collect();
    keys.extend(key);
    layer.is_key_path_locked(field, &keys)
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
            .is_some_and(|sources| sources_locked(layer, sources))
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
