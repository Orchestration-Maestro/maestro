//! Registration and invalidation of owned extension records.
use crate::{
    EventBus, EventError, SourceInfo,
    types::{
        Extension, ExtensionAPI, ExtensionFlag, ExtensionFlagOptions, ExtensionHandle,
        ExtensionRuntimeState, ExtensionShortcut, ExtensionShortcutOptions, FlagValue,
        RegisteredCommand, RegisteredCommandOptions, read, write,
    },
};
use indexmap::IndexMap;
use maestro_tui::KeyId;
use std::sync::{Arc, RwLock};

impl<C, CC> Extension<C, CC> {
    /// Creates an empty owner with supplied paths and provenance.
    #[must_use]
    pub fn new(path: String, resolved_path: String, source_info: Arc<RwLock<SourceInfo>>) -> Self {
        Self {
            path,
            resolved_path,
            source_info,
            commands: IndexMap::new(),
            flags: IndexMap::new(),
            shortcuts: IndexMap::new(),
        }
    }
}
impl ExtensionRuntimeState {
    /// Creates active registration state with no flag values.
    #[must_use]
    pub fn new() -> Self {
        Self {
            flag_values: RwLock::default(),
            stale_message: RwLock::default(),
        }
    }
    /// Returns the first invalidation error, including an empty message.
    ///
    /// # Errors
    /// Returns the retained message when registration is stale.
    pub fn assert_active(&self) -> Result<(), EventError> {
        match read(&self.stale_message).as_ref() {
            Some(message) => Err(message.clone().into()),
            None => Ok(()),
        }
    }
    /// Retains the first invalidation message or the default stale guidance.
    pub fn invalidate(&self, message: Option<String>) {
        write(&self.stale_message).get_or_insert_with(|| message.unwrap_or_else(|| "This extension ctx is stale after session replacement or reload. Do not use a captured maestro or command ctx after ctx.newSession(), ctx.fork(), ctx.switchSession(), or ctx.reload(). For newSession, fork, and switchSession, move post-replacement work into withSession and use the ctx passed to withSession. For reload, do not use the old ctx after await ctx.reload().".into()));
    }
}
impl Default for ExtensionRuntimeState {
    fn default() -> Self {
        Self::new()
    }
}
impl<C, CC> ExtensionAPI<C, CC> {
    /// Retains a registration owner, shared state and event capability.
    #[must_use]
    pub fn new(
        extension: ExtensionHandle<C, CC>,
        runtime: Arc<ExtensionRuntimeState>,
        events: EventBus,
    ) -> Self {
        Self {
            events,
            extension,
            runtime,
        }
    }
    /// Registers or replaces a command in its original ordered slot.
    ///
    /// # Errors
    /// Returns the runtime's stale message before registration.
    pub fn register_command(
        &self,
        name: String,
        options: RegisteredCommandOptions<CC>,
    ) -> Result<(), EventError> {
        self.runtime.assert_active()?;
        let replaced = {
            let mut owner = write(&self.extension);
            let command = RegisteredCommand {
                name: name.clone(),
                source_info: Arc::clone(&owner.source_info),
                description: options.description,
                get_argument_completions: options.get_argument_completions,
                handler: options.handler,
            };
            owner.commands.insert(name, Arc::new(RwLock::new(command)))
        };
        drop(replaced);
        Ok(())
    }
    /// Registers or replaces a raw shortcut in its original ordered slot.
    ///
    /// # Errors
    /// Returns the runtime's stale message before registration.
    pub fn register_shortcut(
        &self,
        shortcut: KeyId,
        options: ExtensionShortcutOptions<C>,
    ) -> Result<(), EventError> {
        self.runtime.assert_active()?;
        let replaced = {
            let mut owner = write(&self.extension);
            let record = ExtensionShortcut {
                shortcut: shortcut.clone(),
                description: options.description,
                handler: options.handler,
                extension_path: owner.path.clone(),
            };
            owner
                .shortcuts
                .insert(shortcut, Arc::new(RwLock::new(record)))
        };
        drop(replaced);
        Ok(())
    }
    /// Registers flag metadata and seeds only a missing shared value.
    ///
    /// # Errors
    /// Returns the runtime's stale message before registration.
    pub fn register_flag(
        &self,
        name: String,
        options: ExtensionFlagOptions,
    ) -> Result<(), EventError> {
        self.runtime.assert_active()?;
        let mut owner = write(&self.extension);
        let flag = ExtensionFlag {
            name: name.clone(),
            description: options.description,
            r#type: options.r#type,
            default: options.default.clone(),
            extension_path: owner.path.clone(),
        };
        owner
            .flags
            .insert(name.clone(), Arc::new(RwLock::new(flag)));
        if let Some(value) = options.default {
            write(&self.runtime.flag_values)
                .entry(name)
                .or_insert(value);
        }
        Ok(())
    }
    /// Reads a shared value only for a flag registered by this owner.
    ///
    /// # Errors
    /// Returns the runtime's stale message before reading registrations.
    pub fn get_flag(&self, name: &str) -> Result<Option<FlagValue>, EventError> {
        self.runtime.assert_active()?;
        if !read(&self.extension).flags.contains_key(name) {
            return Ok(None);
        }
        Ok(read(&self.runtime.flag_values).get(name).cloned())
    }
}
