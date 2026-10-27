//! Plain accepted documents retained independently for each typed configuration area.

use std::{collections::BTreeMap, path::Path};

use maestro_settings::{BaseSettingsLayer, Settings};

use super::{super::model::LoadedConfig, project_global_settings};

/// Loader-owned documents last accepted by each independent area decoder.
#[derive(Clone, Debug, Default)]
pub struct AcceptedAreas(BTreeMap<String, Settings>);

impl AcceptedAreas {
    /// Retains decoded plain JSON, never a serialization of typed configuration.
    pub(super) fn remember(&mut self, area: &str, settings: &Settings) {
        self.0.insert(area.to_owned(), settings.clone());
    }

    /// Advances only accepted areas, retaining documents rejected by runtime checks.
    pub(crate) fn accept(&mut self, loaded: &LoadedConfig) {
        self.0.extend(
            loaded
                .accepted_areas
                .0
                .iter()
                .filter(|(area, _)| !loaded.invalid_sections.contains(area))
                .map(|(area, document)| (area.clone(), document.clone())),
        );
    }

    /// Re-decodes rejected areas after the settings owner restores current locks.
    pub(super) fn recover(
        &self,
        loaded: &mut LoadedConfig,
        layer: Option<&BaseSettingsLayer>,
        agent_dir: &Path,
    ) {
        macro_rules! recover {
            ($($field:ident),* $(,)?) => { $(
                if let Some(recovered) = self.recover_area(loaded, layer, stringify!($field), agent_dir) {
                    loaded.config.$field = recovered.config.$field;
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

    /// Recovers one rejected area while retaining its original failure diagnostic.
    fn recover_area(
        &self,
        loaded: &mut LoadedConfig,
        layer: Option<&BaseSettingsLayer>,
        area: &str,
        agent_dir: &Path,
    ) -> Option<LoadedConfig> {
        if !loaded.invalid_sections.iter().any(|name| name == area) {
            return None;
        }
        if let Some(previous) = self.0.get(area) {
            loaded.accepted_areas.remember(area, previous);
        }
        let document = self.restored_document(area, layer?);
        let recovered = project_global_settings(&document, agent_dir);
        if recovered.invalid_sections.iter().any(|name| name == area) {
            return None;
        }
        loaded.invalid_sections.retain(|name| name != area);
        loaded.accepted_areas.remember(area, &document);
        Some(recovered)
    }

    /// Uses the accepted area document, or the ordinary empty-user startup view.
    fn restored_document(&self, area: &str, layer: &BaseSettingsLayer) -> Settings {
        let Some(mut document) = self.0.get(area).cloned() else {
            return layer.compose_settings(&Settings::default());
        };
        layer.restore_locked_settings(&mut document);
        document
    }
}
