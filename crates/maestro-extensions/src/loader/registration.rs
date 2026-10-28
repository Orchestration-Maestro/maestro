//! Registration and invalidation of owned extension records.
use crate::types::registration::tools::RetainedDefinition;
use crate::types::{ExtensionHandler, RegisteredTool, RegisteredToolInput};
use crate::{
    EventBus, EventError, ExtensionExecution, SourceInfo,
    types::{
        ActivityGuard, Extension, ExtensionAPI, ExtensionFlag, ExtensionFlagOptions,
        ExtensionHandle, ExtensionRuntimeState, ExtensionShortcut, ExtensionShortcutOptions,
        FlagValue, MessageRenderer, RegisteredCommand, RegisteredCommandOptions, read, write,
    },
};
use indexmap::IndexMap;
use maestro_tui::KeyId;
use serde::{Serialize, de::DeserializeOwned};
use std::sync::{Arc, RwLock};

/// Guidance for an invalidated or released registration facade.
pub(crate) const STALE_MESSAGE: &str = "This extension ctx is stale after session replacement or reload. Do not use a captured maestro or command ctx after ctx.newSession(), ctx.fork(), ctx.switchSession(), or ctx.reload(). For newSession, fork, and switchSession, move post-replacement work into withSession and use the ctx passed to withSession. For reload, do not use the old ctx after await ctx.reload().";

impl<E: ExtensionExecution> Extension<E> {
    /// Creates an empty owner with supplied paths and provenance.
    #[must_use]
    pub fn new(path: String, resolved_path: String, source_info: Arc<RwLock<SourceInfo>>) -> Self {
        Self {
            path,
            resolved_path,
            source_info,
            handlers: IndexMap::new(),
            tools: IndexMap::new(),
            commands: IndexMap::new(),
            flags: IndexMap::new(),
            initial_flag_values: IndexMap::new(),
            shortcuts: IndexMap::new(),
            message_renderers: IndexMap::new(),
        }
    }
}
impl ActivityGuard {
    /// Returns the first invalidation error, including an empty message.
    ///
    /// # Errors
    /// Returns the retained message once invalidated.
    pub(crate) fn assert_active(&self) -> Result<(), EventError> {
        match read(&self.stale_message).as_ref() {
            Some(message) => Err(message.clone().into()),
            None => Ok(()),
        }
    }
    /// Retains the first invalidation message or the default stale guidance.
    pub(crate) fn invalidate(&self, message: Option<String>) {
        write(&self.stale_message)
            .get_or_insert_with(|| message.unwrap_or_else(|| STALE_MESSAGE.into()));
    }
}
impl ExtensionRuntimeState {
    /// Creates active registration state with no flag values.
    #[must_use]
    pub fn new() -> Self {
        Self {
            flag_values: RwLock::default(),
            activity: ActivityGuard::default(),
        }
    }
    /// Returns the first invalidation error, including an empty message.
    ///
    /// # Errors
    /// Returns the retained message when registration is stale.
    pub fn assert_active(&self) -> Result<(), EventError> {
        self.activity.assert_active()
    }
    /// Retains the first invalidation message or the default stale guidance.
    pub fn invalidate(&self, message: Option<String>) {
        self.activity.invalidate(message);
    }
}
impl Default for ExtensionRuntimeState {
    fn default() -> Self {
        Self::new()
    }
}
impl<E: ExtensionExecution> ExtensionAPI<E> {
    /// Borrows a registration owner, retaining shared state and event capability.
    /// The loader or runtime must keep a strong owner while the extension is live.
    #[must_use]
    pub fn new(
        extension: &ExtensionHandle<E>,
        runtime: E::Shared<crate::types::ExtensionRuntime<E>>,
        events: EventBus<E>,
        cwd: String,
    ) -> Self {
        #[cfg(target_arch = "wasm32")]
        drop(cwd);
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            cwd,
            events,
            extension: E::downgrade(extension),
            runtime,
        }
    }
    /// Check that this facade's original registration owner is still active.
    ///
    /// # Errors
    /// Returns the original stale error after invalidation or owner release.
    pub fn assert_active(&self) -> Result<(), EventError> {
        self.owner().map(drop)
    }
    /// Upgrades the live registration owner after checking invalidation.
    pub(super) fn owner(&self) -> Result<ExtensionHandle<E>, EventError> {
        self.runtime.assert_active()?;
        E::upgrade(&self.extension).ok_or_else(|| STALE_MESSAGE.into())
    }
    /// Registers or replaces a command in its original ordered slot.
    ///
    /// # Errors
    /// Returns the stale error if invalidated or released, before registration.
    pub fn register_command(
        &self,
        name: String,
        options: RegisteredCommandOptions<E>,
    ) -> Result<(), EventError> {
        let extension = self.owner()?;
        let replaced = {
            let mut owner = write(&extension);
            let command = RegisteredCommand {
                name: name.clone(),
                source_info: Arc::clone(&owner.source_info),
                description: options.description,
                get_argument_completions: options.get_argument_completions,
                handler: options.handler,
            };
            owner.commands.insert(name, E::share(RwLock::new(command)))
        };
        drop(replaced);
        Ok(())
    }
    /// Registers or replaces a raw shortcut in its original ordered slot.
    ///
    /// # Errors
    /// Returns the stale error if invalidated or released, before registration.
    pub fn register_shortcut(
        &self,
        shortcut: KeyId,
        options: ExtensionShortcutOptions<E>,
    ) -> Result<(), EventError> {
        let extension = self.owner()?;
        let replaced = {
            let mut owner = write(&extension);
            let record = ExtensionShortcut {
                shortcut: shortcut.clone(),
                description: options.description,
                handler: options.handler,
                extension_path: owner.path.clone(),
            };
            owner
                .shortcuts
                .insert(shortcut, E::share(RwLock::new(record)))
        };
        drop(replaced);
        Ok(())
    }
    /// Registers or replaces a custom-message renderer in its original ordered slot.
    ///
    /// # Errors
    /// Returns the stale error if invalidated or released, before registration.
    pub fn register_message_renderer<T: 'static>(
        &self,
        custom_type: String,
        renderer: MessageRenderer<T, E>,
    ) -> Result<(), EventError> {
        let extension = self.owner()?;
        let replaced = write(&extension).message_renderers.insert(
            custom_type,
            E::erase_message_renderer(renderer, self.events.clone()),
        );
        drop(replaced);
        Ok(())
    }
    /// Registers flag metadata and seeds only a missing shared value.
    ///
    /// # Errors
    /// Returns the stale error if invalidated or released, before registration.
    pub fn register_flag(
        &self,
        name: String,
        options: ExtensionFlagOptions,
    ) -> Result<(), EventError> {
        let extension = self.owner()?;
        let mut owner = write(&extension);
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
            owner
                .initial_flag_values
                .entry(name.clone())
                .or_insert_with(|| value.clone());
            write(&self.runtime.flag_values)
                .entry(name)
                .or_insert(value);
        }
        Ok(())
    }
    /// Reads a shared value only for a flag registered by this owner.
    ///
    /// # Errors
    /// Returns the stale error if invalidated or released, before reading registrations.
    pub fn get_flag(&self, name: &str) -> Result<Option<FlagValue>, EventError> {
        let extension = self.owner()?;
        if !read(&extension).flags.contains_key(name) {
            return Ok(None);
        }
        Ok(read(&self.runtime.flag_values).get(name).cloned())
    }
}

impl<E: ExtensionExecution> ExtensionAPI<E> {
    /// Appends even duplicate handlers for subsequent live runner dispatch.
    /// # Errors
    /// Returns the existing stale-owner error.
    pub fn on<V: 'static, R: 'static>(
        &self,
        event: String,
        handler: ExtensionHandler<V, R, E>,
    ) -> Result<(), EventError> {
        self.on_retained(event, E::erase_handler(handler))
    }
    /// Retains one opaque registration in the original ordered handler owner.
    ///
    /// Adapter callbacks under unknown names remain owned without a callable surrogate.
    /// # Errors
    /// Returns the existing stale-owner error before retaining the registration.
    pub fn on_retained(
        &self,
        event: String,
        registration: E::HandlerRegistration,
    ) -> Result<(), EventError> {
        let owner = self.owner()?;
        write(&owner)
            .handlers
            .entry(event)
            .or_default()
            .push(registration);
        Ok(())
    }
    /// Replace in the original slot, then invoke the live refresh outside the guard.
    /// # Errors
    /// Returns the stale error before writing, or the refresh error after writing.
    pub fn register_tool<
        P: DeserializeOwned + 'static,
        D: Serialize + 'static,
        S: Default + 'static,
    >(
        &self,
        definition: impl Into<RegisteredToolInput<P, D, S, E>>,
    ) -> Result<(), EventError> {
        let owner = self.owner()?;
        let definition = E::retain_definition(definition.into());
        let name = definition.name();
        let retired = {
            let mut owner = write(&owner);
            let source_info = owner.source_info.clone();
            owner.tools.insert(
                name,
                RegisteredTool {
                    definition,
                    source_info,
                },
            )
        };
        drop(retired);
        {
            let refresh = read(&self.runtime.actions).refresh_tools.clone();
            refresh()
        }
    }
}
