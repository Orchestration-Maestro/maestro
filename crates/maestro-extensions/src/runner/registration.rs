//! Command invocation resolution and current flag queries.
use super::ExtensionRunner;
use crate::types::{ExtensionFlag, FlagValue, ResolvedCommand, read, write};
use indexmap::{IndexMap, IndexSet};
use std::sync::{Arc, RwLock};

impl<C, CC> ExtensionRunner<C, CC> {
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
    /// Resolves current commands and publishes fresh empty diagnostics.
    #[must_use]
    pub fn get_registered_commands(&self) -> Vec<ResolvedCommand<CC>> {
        *write(&self.command_diagnostics) = Arc::default();
        self.resolve_registered_commands()
    }
    /// Looks up only the current resolved invocation name.
    #[must_use]
    pub fn get_command(&self, name: &str) -> Option<ResolvedCommand<CC>> {
        self.resolve_registered_commands()
            .into_iter()
            .find(|command| command.invocation_name == name)
    }
    /// Copies registrations before allocating unique invocation names in order.
    fn resolve_registered_commands(&self) -> Vec<ResolvedCommand<CC>> {
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
