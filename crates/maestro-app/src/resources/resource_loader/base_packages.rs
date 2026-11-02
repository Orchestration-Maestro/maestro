//! Early base factories and their ordinary package resource positions.
use super::{DefaultEventBus, DefaultResourceLoader};
use maestro_extensions::{
    LoadExtensionsResult, create_extension_runtime, load_extensions_with_runtime,
};
use maestro_packages::{PackageManager, ResolvedPaths};
use maestro_resources::SourceScope;
use std::{io, sync::Arc};

/// Appends lower-precedence package resources without changing existing positions.
pub(super) fn append_paths(paths: &mut ResolvedPaths, extra: ResolvedPaths) {
    paths.extensions.extend(extra.extensions);
    paths.skills.extend(extra.skills);
    paths.prompts.extend(extra.prompts);
    paths.themes.extend(extra.themes);
    paths.contributions.extend(extra.contributions);
}

impl<E: DefaultEventBus> DefaultResourceLoader<E> {
    /// Runs base factories before resolving ordinary packages and combines their paths.
    pub(super) async fn resolve_packages(
        &self,
    ) -> io::Result<(ResolvedPaths, LoadExtensionsResult<E>)> {
        let (base_paths, mut extensions) = self.load_base_packages().await?;
        let contributions = self
            .collect_load_time(&mut extensions, &base_paths.extensions)
            .await?;
        let mut resolved = self.package_manager.resolve(None).await?;
        append_paths(&mut resolved, base_paths);
        append_paths(&mut resolved, contributions);
        Ok((resolved, extensions))
    }

    /// Resolves base packages and runs their factories before ordinary collection.
    pub(super) async fn load_base_packages(
        &self,
    ) -> io::Result<(ResolvedPaths, LoadExtensionsResult<E>)> {
        let resolved = self
            .package_manager
            .resolve_extension_sources(&self.options.base_package_sources, Some(SourceScope::User))
            .await?;
        let paths: Vec<_> = resolved
            .extensions
            .iter()
            .filter(|entry| entry.enabled)
            .map(|entry| entry.path.clone())
            .collect();
        let paths = self.merge_paths(&paths, [])?;
        let runtime = create_extension_runtime::<E>();
        if let Some(services) = &self.options.tool_construction {
            runtime.set_tool_construction(services.clone());
        }
        let auth_root = self.agent_dir.clone();
        let models_root = self.agent_dir.clone();
        runtime.set_catalog_construction(maestro_extensions::CatalogConstruction {
            auth_path: Arc::new(move || {
                Ok(crate::configuration::config::paths::get_auth_path(
                    &auth_root,
                ))
            }),
            models_path: Arc::new(move || {
                Ok(crate::configuration::config::paths::get_models_path(
                    &models_root,
                ))
            }),
            config_values: crate::runtime::agent_session_services::native_config_values(),
        });
        runtime.defer_provider_operations(&paths);
        runtime.begin_load_time(paths.clone());
        let result = load_extensions_with_runtime(
            paths,
            &self.cwd,
            &self.event_bus,
            &self.options.module_loader,
            runtime,
        )
        .await?;
        Ok((resolved, result))
    }
}
