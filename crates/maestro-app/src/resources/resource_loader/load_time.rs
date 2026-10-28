//! Application decoding of the base packages' single load-time input channel.
use super::{DefaultEventBus, DefaultResourceLoader};
use maestro_extensions::{ExtensionLoadError, LoadExtensionsResult};
use maestro_packages::{PackageManager, ResolvedPaths, ResolvedResource};
use maestro_resources::{SourceScope, canonicalize_path};
use maestro_settings::{BaseSettingsLayer, SettingsScope};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    io,
};

/// A contributed package source resolved in its supplied installation scope.
#[derive(Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct PackageInput {
    /// The ordinary package source spelling.
    source: String,
    /// Its installation and resource provenance scope.
    scope: SourceScope,
}

/// One scoped publication through the settings owner's existing layer operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LayerInput {
    /// User means global settings; temporary publication is invalid.
    scope: SourceScope,
    /// The settings owner's record, or an explicit removal.
    #[serde(deserialize_with = "Option::deserialize")]
    layer: Option<BaseSettingsLayer>,
}

/// One package's publication identity and the extension named in diagnostics.
struct Publisher {
    /// The source shared by every entrypoint of this package.
    source: String,
    /// Its first publishing extension.
    path: String,
}

/// The generic load-time input format; no manifest policy is interpreted here.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
struct Inputs {
    /// Package sources collected after ordinary packages.
    package_sources: Vec<PackageInput>,
    /// A system prompt below local files and explicit inputs.
    base_system_prompt: Option<String>,
    /// Text preceding the ordinary appended prompt.
    base_append_text: Vec<String>,
    /// Model files preceding the catalog's construction-time files.
    model_files: Vec<String>,
    /// Scoped settings publications by one package owner.
    base_layers: Vec<LayerInput>,
}

impl<E: DefaultEventBus> DefaultResourceLoader<E> {
    /// Decodes every contribution before applying it and resolves each supplied source once.
    pub(super) async fn collect_load_time(
        &self,
        extensions: &mut LoadExtensionsResult<E>,
        resources: &[ResolvedResource],
    ) -> io::Result<ResolvedPaths> {
        let owners = resources
            .iter()
            .map(|resource| {
                (
                    canonicalize_path(&resource.path),
                    resource.metadata.source.as_str(),
                )
            })
            .collect();
        let mut paths = ResolvedPaths::default();
        for package in self.load_time_inputs(extensions, &owners) {
            let found = self
                .package_manager
                .resolve_extension_sources(&[package.source], Some(package.scope))
                .await?;
            super::base_packages::append_paths(&mut paths, found);
        }
        Ok(paths)
    }

    /// Starts a fresh publication window and attributes decoding or publisher failures.
    fn load_time_inputs(
        &self,
        extensions: &mut LoadExtensionsResult<E>,
        owners: &HashMap<String, &str>,
    ) -> Vec<PackageInput> {
        {
            let mut state = self.state.borrow_mut();
            state.base_system_prompt = None;
            state.base_append_text.clear();
            state.model_files.clear();
        }
        let loaded: HashSet<_> = extensions
            .extensions
            .iter()
            .map(|owner| crate::utils::lock::read(owner).path.clone())
            .collect();
        let mut publisher = None;
        let mut packages = Vec::new();
        for (path, payload) in extensions.runtime.finish_load_time() {
            if !loaded.contains(&path) {
                continue;
            }
            let source = owners
                .get(&canonicalize_path(&path))
                .copied()
                .unwrap_or(&path);
            let accepted = serde_json::from_value::<Inputs>(payload)
                .map_err(|error| error.to_string())
                .and_then(|inputs| self.accept_load_time(inputs, &path, source, &mut publisher));
            let inputs = match accepted {
                Ok(inputs) => inputs,
                Err(error) => {
                    extensions.errors.push(ExtensionLoadError {
                        path,
                        error: format!("Invalid load-time contribution: {error}"),
                    });
                    continue;
                }
            };
            for package in inputs {
                add_package_input(&mut packages, package);
            }
        }
        packages
    }

    /// Validates the complete publication before changing settings or prompt inputs.
    fn accept_load_time(
        &self,
        inputs: Inputs,
        path: &str,
        source: &str,
        publisher: &mut Option<Publisher>,
    ) -> Result<Vec<PackageInput>, String> {
        if !inputs.base_layers.is_empty()
            && let Some(owner) = publisher.as_ref().filter(|owner| owner.source != source)
        {
            return Err(format!(
                "Base layer publisher already registered by {}",
                owner.path
            ));
        }
        let layers: Result<Vec<_>, _> = inputs
            .base_layers
            .into_iter()
            .map(|input| {
                let scope = match input.scope {
                    SourceScope::User => SettingsScope::Global,
                    SourceScope::Project => SettingsScope::Project,
                    SourceScope::Temporary => {
                        return Err("A base layer requires user or project scope".to_owned());
                    }
                };
                Ok((scope, input.layer))
            })
            .collect();
        let layers = layers?;
        let model_files = self
            .merge_paths(&inputs.model_files, [])
            .map_err(|error| error.to_string())?;
        if !layers.is_empty() {
            publisher.get_or_insert_with(|| Publisher {
                source: source.to_owned(),
                path: path.to_owned(),
            });
        }
        for (scope, layer) in layers {
            self.settings_manager
                .borrow_mut()
                .publish_base_layer(scope, layer);
        }
        let mut state = self.state.borrow_mut();
        if state.base_system_prompt.is_none() {
            state.base_system_prompt = inputs.base_system_prompt;
        }
        state.base_append_text.extend(inputs.base_append_text);
        state.model_files.extend(model_files);
        Ok(inputs.package_sources)
    }
}

/// Retains the first contribution of one package source in its scope.
fn add_package_input(packages: &mut Vec<PackageInput>, package: PackageInput) {
    if !packages.contains(&package) {
        packages.push(package);
    }
}
