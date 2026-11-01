//! Path and inline extension loading, conflict diagnostics, extension provenance and the
//! default event bus.
use super::{DefaultResourceLoader, overrides::apply};
use crate::utils::lock::{read, write};
use maestro_extensions::{
    EventBusController, ExtensionExecution, ExtensionLoadError, LoadExtensionsResult,
    create_event_bus_for, create_extension_runtime, load_extension_from_factory,
    load_extensions_with_runtime,
};
use maestro_models::{Execution, Local, Threaded};
use maestro_resources::{canonicalize_path, is_local_path};
use std::{
    collections::HashMap,
    io,
    path::Path,
    rc::Rc,
    sync::{Arc, RwLock},
};
use tokio::task::LocalSet;

/// Execution families whose default event bus runs listener continuations on the
/// caller's task set.
pub trait DefaultEventBus: ExtensionExecution {
    /// A bus whose continuations run on `local` and whose listener failures are written
    /// to standard error.
    fn default_event_bus(local: &Rc<LocalSet>) -> EventBusController<Self>;
    /// Retains the original local owners behind this family's callback representation.
    fn operation_handler(
        operations: Rc<crate::extension_operations::ExtensionOperations>,
        local: &Rc<LocalSet>,
    ) -> Self::Shared<Self::HostOperation>;
}

/// Writes a bus diagnostic, which carries its own final newline, to standard error.
pub(crate) fn report(text: &str) {
    eprint!("{text}");
}

impl DefaultEventBus for Local {
    fn operation_handler(
        operations: Rc<crate::extension_operations::ExtensionOperations>,
        _local: &Rc<LocalSet>,
    ) -> Self::Shared<Self::HostOperation> {
        Rc::new(move |operation| {
            let work = operations.call(operation);
            Box::pin(async move { work.await.map_err(|error| error.to_string()) })
        })
    }
    fn default_event_bus(local: &Rc<LocalSet>) -> EventBusController<Self> {
        let local = Rc::downgrade(local);
        create_event_bus_for::<Self>(
            Rc::new(move |tail| {
                if let Some(local) = local.upgrade() {
                    drop(local.spawn_local(tail));
                }
            }),
            Rc::new(report),
        )
    }
}

impl DefaultEventBus for Threaded {
    fn operation_handler(
        operations: Rc<crate::extension_operations::ExtensionOperations>,
        local: &Rc<LocalSet>,
    ) -> Self::Shared<Self::HostOperation> {
        let sender = crate::utils::task::forward(
            local,
            move |(operation, reply): (
                _,
                tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
            )| {
                let work = operations.call(operation);
                Box::pin(async move {
                    let _ = reply.send(work.await.map_err(|error| error.to_string()));
                })
            },
        );
        Arc::new(move |operation| {
            let (reply, response) = tokio::sync::oneshot::channel();
            let sent = sender.send((operation, reply));
            Box::pin(async move {
                sent.map_err(|_| "Extension host operations ended".to_owned())?;
                response
                    .await
                    .map_err(|_| "Extension host operation reply ended".to_owned())?
            })
        })
    }
    fn default_event_bus(local: &Rc<LocalSet>) -> EventBusController<Self> {
        let sender =
            crate::utils::task::forward(local, |tail: <Self as Execution>::Future<'static, ()>| {
                tail
            });
        create_event_bus_for::<Self>(
            Arc::new(move |tail| {
                // A closed channel means the task set is gone; the continuation is dropped.
                drop(sender.send(tail));
            }),
            Arc::new(report),
        )
    }
}

/// A conflict error for each tool or flag that a later extension path registers again.
fn conflicts<E: ExtensionExecution>(result: &LoadExtensionsResult<E>) -> Vec<ExtensionLoadError> {
    let mut found = Vec::new();
    let mut tool_owners = HashMap::new();
    let mut flag_owners = HashMap::new();
    for extension in &result.extensions {
        let extension = read(extension);
        for name in extension.tools.keys() {
            claim(
                &mut tool_owners,
                name,
                &extension.path,
                "Tool \"",
                &mut found,
            );
        }
        for name in extension.flags.keys() {
            claim(
                &mut flag_owners,
                name,
                &extension.path,
                "Flag \"--",
                &mut found,
            );
        }
    }
    found
}

/// Gives `name` to `path` unless another path owns it, which is a conflict reported as
/// `<label><name>" conflicts with <owner>`.
fn claim(
    owners: &mut HashMap<String, String>,
    name: &str,
    path: &str,
    label: &str,
    found: &mut Vec<ExtensionLoadError>,
) {
    match owners.get(name) {
        Some(owner) if owner != path => found.push(ExtensionLoadError {
            path: path.to_owned(),
            error: format!("{label}{name}\" conflicts with {owner}"),
        }),
        _ => {
            owners.insert(name.to_owned(), path.to_owned());
        }
    }
}

impl<E: super::DefaultEventBus> DefaultResourceLoader<E> {
    /// Loads path extensions, then inline factories on the same runtime, reports conflicts
    /// and missing explicit paths, applies the override and stores the result with each
    /// extension's provenance.
    pub(super) async fn load_extension_resources(
        &self,
        paths: Vec<String>,
        mut result: LoadExtensionsResult<E>,
    ) -> io::Result<()> {
        let base_attempted = !result.extensions.is_empty() || !result.errors.is_empty();
        if base_attempted {
            write(&result.runtime.flag_values).clear();
        }
        result = if base_attempted {
            self.load_ordered_paths(paths, result).await?
        } else {
            load_extensions_with_runtime(
                paths,
                &self.cwd,
                &self.event_bus,
                &self.options.module_loader,
                result.runtime,
            )
            .await?
        };
        if base_attempted {
            restore_flag_defaults(&result);
        }
        self.load_inline_factories(&mut result).await;
        result.errors.extend(conflicts(&result));
        for path in &self.options.additional_extension_paths {
            if is_local_path(path) && !Path::new(&self.resolve_resource_path(path)?).exists() {
                result.errors.push(ExtensionLoadError {
                    path: path.clone(),
                    error: format!("Extension path does not exist: {path}"),
                });
            }
        }
        if base_attempted {
            restore_flag_defaults(&result);
        }
        let result = apply(self.options.extensions_override.as_ref(), result);
        self.apply_extension_provenance(&result)?;
        self.state.borrow_mut().extensions = Rc::new(result);
        Ok(())
    }

    /// Reuses early factories at their canonical ordinary slots, exposing each prefix's defaults.
    async fn load_ordered_paths(
        &self,
        paths: Vec<String>,
        mut result: LoadExtensionsResult<E>,
    ) -> io::Result<LoadExtensionsResult<E>> {
        let base = std::mem::take(&mut result.extensions);
        let mut by_path: HashMap<_, _> = base
            .iter()
            .map(|extension| (canonicalize_path(&read(extension).path), extension.clone()))
            .collect();
        let failed: HashMap<_, _> = result
            .errors
            .iter()
            .map(|error| (canonicalize_path(&error.path), error.path.clone()))
            .collect();
        for path in paths {
            let canonical = canonicalize_path(&path);
            if let Some(extension) = by_path.remove(&canonical) {
                let path = read(&extension).path.clone();
                result
                    .runtime
                    .replay_provider_operations(&path)
                    .map_err(io::Error::other)?;
                result.extensions.push(extension);
                restore_flag_defaults(&result);
                continue;
            }
            if let Some(path) = failed.get(&canonical) {
                result
                    .runtime
                    .replay_provider_operations(path)
                    .map_err(io::Error::other)?;
                continue;
            }
            let loaded = load_extensions_with_runtime(
                vec![path],
                &self.cwd,
                &self.event_bus,
                &self.options.module_loader,
                result.runtime.clone(),
            )
            .await?;
            result.extensions.extend(loaded.extensions);
            result.errors.extend(loaded.errors);
        }
        result
            .runtime
            .replay_remaining_provider_operations()
            .map_err(io::Error::other)?;
        result.extensions.extend(
            base.into_iter().filter(|extension| {
                by_path.contains_key(&canonicalize_path(&read(extension).path))
            }),
        );
        Ok(result)
    }

    /// Runs inline factories after every path factory on the same runtime.
    async fn load_inline_factories(&self, result: &mut LoadExtensionsResult<E>) {
        for (index, factory) in self.options.extension_factories.iter().enumerate() {
            let path = format!("<inline:{}>", index + 1);
            match load_extension_from_factory(
                factory.clone(),
                self.cwd.clone(),
                self.event_bus.for_extension(),
                result.runtime.clone(),
                Some(path.clone()),
            )
            .await
            {
                Ok(extension) => result.extensions.push(extension),
                Err(error) => result.errors.push(ExtensionLoadError {
                    path,
                    error: error.to_string(),
                }),
            }
        }
    }

    /// Installs the original application owners before the first factory runs.
    pub(super) fn extension_runtime(&self) -> E::Shared<maestro_extensions::ExtensionRuntime<E>> {
        let runtime = create_extension_runtime::<E>();
        runtime.set_host_operations(E::operation_handler(
            crate::extension_operations::ExtensionOperations::new(
                self.settings_manager.clone(),
                self.package_manager.clone(),
            ),
            &self.options.local,
        ));
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
        runtime
    }

    /// Gives each extension a fresh provenance record, shared with its commands and tools.
    fn apply_extension_provenance(&self, result: &LoadExtensionsResult<E>) -> io::Result<()> {
        let state = self.state.borrow();
        for extension in &result.extensions {
            let mut extension = write(extension);
            let source_info = match self.find_source_info(&extension.path, None, &state.metadata)? {
                Some(found) => found,
                None => self.default_source_info(&extension.path)?,
            };
            let shared = Arc::new(RwLock::new(source_info));
            for command in extension.commands.values() {
                write(command).source_info = Arc::clone(&shared);
            }
            for tool in extension.tools.values_mut() {
                tool.source_info = Arc::clone(&shared);
            }
            extension.source_info = shared;
        }
        Ok(())
    }
}

/// Defaults are seeded during factory execution, so reselect them from registration order.
fn restore_flag_defaults<E: ExtensionExecution>(result: &LoadExtensionsResult<E>) {
    let mut defaults = indexmap::IndexMap::new();
    for extension in &result.extensions {
        for (name, value) in &read(extension).initial_flag_values {
            defaults
                .entry(name.clone())
                .or_insert_with(|| value.clone());
        }
    }
    *write(&result.runtime.flag_values) = defaults;
}
