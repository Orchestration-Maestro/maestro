//! Application decoding of the base packages' single load-time input channel.
use super::{DefaultEventBus, DefaultResourceLoader};
use maestro_extensions::{ExtensionLoadError, LoadExtensionsResult};
use maestro_packages::{PackageManager, ResolvedPaths};
use maestro_resources::SourceScope;
use serde::Deserialize;
use std::io;

/// A contributed package source resolved in its supplied installation scope.
#[derive(Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct PackageInput {
    /// The ordinary package source spelling.
    source: String,
    /// Its installation and resource provenance scope.
    scope: SourceScope,
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
}

impl<E: DefaultEventBus> DefaultResourceLoader<E> {
    /// Decodes every contribution before applying it and resolves each supplied source once.
    pub(super) async fn collect_load_time(
        &self,
        extensions: &mut LoadExtensionsResult<E>,
    ) -> io::Result<ResolvedPaths> {
        let mut paths = ResolvedPaths::default();
        for package in self.load_time_inputs(extensions) {
            let found = self
                .package_manager
                .resolve_extension_sources(&[package.source], Some(package.scope))
                .await?;
            super::base_packages::append_paths(&mut paths, found);
        }
        Ok(paths)
    }

    /// Decodes inputs without holding the loader state across asynchronous acquisition.
    fn load_time_inputs(&self, extensions: &mut LoadExtensionsResult<E>) -> Vec<PackageInput> {
        let mut packages = Vec::new();
        let mut state = self.state.borrow_mut();
        state.base_system_prompt = None;
        state.base_append_text.clear();
        for (path, payload) in extensions.runtime.finish_load_time() {
            let inputs = match serde_json::from_value::<Inputs>(payload) {
                Ok(inputs) => inputs,
                Err(error) => {
                    extensions.errors.push(ExtensionLoadError {
                        path,
                        error: format!("Invalid load-time contribution: {error}"),
                    });
                    continue;
                }
            };
            if state.base_system_prompt.is_none() {
                state.base_system_prompt = inputs.base_system_prompt;
            }
            state.base_append_text.extend(inputs.base_append_text);
            for package in inputs.package_sources {
                add_package_input(&mut packages, package);
            }
        }
        packages
    }
}

/// Retains the first contribution of one package source in its scope.
fn add_package_input(packages: &mut Vec<PackageInput>, package: PackageInput) {
    if !packages.contains(&package) {
        packages.push(package);
    }
}
