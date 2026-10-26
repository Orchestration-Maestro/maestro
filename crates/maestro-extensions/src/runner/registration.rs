//! Command invocation resolution and current flag queries.
use super::ExtensionRunner;
use crate::types::registration::tools::RetainedDefinition;
use crate::{ExtensionContext, types::RegisteredTool};
use crate::{
    ExtensionExecution,
    types::{
        ExtensionFlag, FlagValue, MessageRenderer, MessageRendererCall, ResolvedCommand, read,
        write,
    },
};
use indexmap::{IndexMap, IndexSet};
use maestro_models::Local;
use serde::{Serialize, de::DeserializeOwned};
use std::sync::{Arc, RwLock};

impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Returns supplied owner paths in current load order.
    #[must_use]
    pub fn get_extension_paths(&self) -> Vec<String> {
        read(&self.extensions)
            .iter()
            .map(|owner| read(owner).path.clone())
            .collect()
    }
    /// Retains the first registration of each flag name in load order.
    #[must_use]
    pub fn get_flags(&self) -> IndexMap<String, Arc<RwLock<ExtensionFlag>>> {
        let mut flags = IndexMap::new();
        for owner in read(&self.extensions).iter() {
            for (name, flag) in &read(owner).flags {
                flags
                    .entry(name.clone())
                    .or_insert_with(|| Arc::clone(flag));
            }
        }
        flags
    }
    /// Sets a shared value even when no owner registered that name.
    pub fn set_flag_value(&self, name: String, value: FlagValue) {
        write(&self.runtime.flag_values).insert(name, value);
    }
    /// Returns an independent ordered snapshot of shared values.
    #[must_use]
    pub fn get_flag_values(&self) -> IndexMap<String, FlagValue> {
        read(&self.runtime.flag_values).clone()
    }
    /// Returns the first owner's renderer for `custom_type` when it has details type `T`.
    ///
    /// A different details type on that owner is absent; later owners are not consulted.
    #[must_use]
    pub fn get_message_renderer<T: 'static>(
        &self,
        custom_type: &str,
    ) -> Option<MessageRenderer<T, E>> {
        self.get_message_renderer_call(custom_type)
            .map(|call| call.renderer)
    }
    /// Retains the first owner's renderer with the bus that registered it.
    ///
    /// A different details type on that owner is absent; later owners are not consulted.
    #[must_use]
    pub fn get_message_renderer_call<T: 'static>(
        &self,
        custom_type: &str,
    ) -> Option<MessageRendererCall<T, E>> {
        read(&self.extensions)
            .iter()
            .find_map(|owner| {
                read(owner)
                    .message_renderers
                    .get(custom_type)
                    .map(E::recover_message_renderer_call::<T>)
            })
            .flatten()
    }
    /// Resolves current commands and publishes fresh empty diagnostics.
    #[must_use]
    pub fn get_registered_commands(&self) -> Vec<ResolvedCommand<E>> {
        *write(&self.command_diagnostics) = Arc::default();
        self.resolve_registered_commands()
    }
    /// Looks up only the current resolved invocation name.
    #[must_use]
    pub fn get_command(&self, name: &str) -> Option<ResolvedCommand<E>> {
        self.resolve_registered_commands()
            .into_iter()
            .find(|command| command.invocation_name == name)
    }
    /// Copies registrations before allocating unique invocation names in order.
    fn resolve_registered_commands(&self) -> Vec<ResolvedCommand<E>> {
        let mut commands = Vec::new();
        let mut counts = IndexMap::new();
        for owner in read(&self.extensions).iter() {
            for command in read(owner).commands.values() {
                let command = read(command).clone();
                *counts.entry(command.name.clone()).or_insert(0) += 1;
                commands.push(command);
            }
        }
        let mut seen = IndexMap::new();
        let mut taken = IndexSet::new();
        commands
            .into_iter()
            .map(|command| {
                let occurrence = seen.entry(command.name.clone()).or_insert(0);
                *occurrence += 1;
                let mut suffix = *occurrence;
                let mut invocation_name = if counts[&command.name] > 1 {
                    format!("{}:{suffix}", command.name)
                } else {
                    command.name.clone()
                };
                while taken.contains(&invocation_name) {
                    suffix += 1;
                    invocation_name = format!("{}:{suffix}", command.name);
                }
                taken.insert(invocation_name.clone());
                ResolvedCommand {
                    command,
                    invocation_name,
                }
            })
            .collect()
    }
}

impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Returns the first current-name registration in owner and insertion order.
    #[must_use]
    pub fn get_all_registered_tools(&self) -> Vec<RegisteredTool<E>> {
        let mut first = IndexMap::new();
        for owner in read(&self.extensions).iter() {
            for tool in read(owner).tools.values() {
                first
                    .entry(tool.definition.name())
                    .or_insert_with(|| tool.clone());
            }
        }
        first.into_values().collect()
    }
    #[must_use]
    /// Returns the first stored-key match when its parameter, details and state types match.
    pub fn get_tool_definition<
        P: DeserializeOwned + 'static,
        D: Serialize + 'static,
        S: Default + 'static,
    >(
        &self,
        name: &str,
    ) -> Option<
        maestro_tools::tool_definition_wrapper::RetainedToolDefinition<
            ExtensionContext,
            P,
            D,
            S,
            E,
            Local,
        >,
    > {
        let registered = read(&self.extensions)
            .iter()
            .find_map(|x| read(x).tools.get(name).cloned())?;
        registered
            .definition
            .as_any()
            .downcast_ref::<crate::types::registration::tools::Definition<P, D, S, E>>()
            .map(|x| x.definition.clone())
    }
}
