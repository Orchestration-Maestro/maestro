//! Native named selection reuses the tools-owned definition selector.
use super::{
    Message, Registry,
    builtin::{Factory, Operation},
};
use crate::{builtin_reads::Endpoint, guest_calls::Handle};
use std::{
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Independently retained effects; unused options are dropped by the native selector.
pub(crate) struct Options {
    /// Optional image policy.
    pub(crate) read_resize: Option<bool>,
    /// Read observations.
    pub(crate) read: Option<Handle>,
    /// Shell controls and effects.
    pub(crate) bash: super::builtin_bash::Options,
    /// Edit observations.
    pub(crate) edit: Option<Handle>,
    /// Write observations.
    pub(crate) write: Option<Handle>,
    /// Content-search observations.
    pub(crate) grep: Option<Handle>,
    /// File-search observations.
    pub(crate) find: Option<Handle>,
    /// Directory observations.
    pub(crate) ls: Option<Handle>,
}

impl Options {
    /// Adapts observations only; native factories choose which member they consume.
    fn native(
        self,
        registry: &Registry,
        api: u64,
    ) -> Result<maestro_extensions::local::ToolsOptions, String> {
        Ok(maestro_extensions::local::ToolsOptions {
            read: maestro_extensions::ReadToolOptions {
                auto_resize_images: self.read_resize,
                operations: self.read.map(|owner| {
                    Arc::new(crate::guest_calls::read::Operations(owner))
                        as Arc<dyn maestro_extensions::ReadOperations>
                }),
            },
            bash: registry.bash_options(api, self.bash)?,
            edit: maestro_extensions::EditToolOptions {
                operations: self.edit.map(|owner| {
                    Arc::new(crate::guest_calls::edit::Operations(owner))
                        as Arc<dyn maestro_extensions::EditOperations>
                }),
            },
            write: maestro_extensions::WriteToolOptions {
                operations: self.write.map(|owner| {
                    Arc::new(crate::guest_calls::write::Operations(owner))
                        as Arc<dyn maestro_extensions::WriteOperations>
                }),
            },
            grep: maestro_extensions::GrepToolOptions {
                operations: self.grep.map(|owner| {
                    Arc::new(crate::guest_calls::grep::Operations(owner))
                        as Arc<dyn maestro_extensions::GrepOperations>
                }),
            },
            find: maestro_extensions::FindToolOptions {
                operations: self.find.map(|owner| {
                    Arc::new(crate::guest_calls::find::Operations(owner))
                        as Arc<dyn maestro_extensions::FindOperations>
                }),
            },
            ls: maestro_extensions::LsToolOptions {
                operations: self.ls.map(|owner| {
                    Arc::new(crate::guest_calls::ls::Operations(owner))
                        as Arc<dyn maestro_extensions::LsOperations>
                }),
            },
        })
    }
}

impl Registry {
    /// Retains the definition returned by the original exact-name selector.
    pub(super) fn selected(
        &self,
        api: u64,
        name: &str,
        cwd: String,
        options: Options,
    ) -> Result<Rc<dyn crate::builtin_tools::Definition>, String> {
        let services = self.tool_construction(api)?;
        let definition =
            maestro_extensions::local::create_tool_definition::<crate::builtin_tools::Context>(
                name,
                cwd,
                options.native(self, api)?,
                services.bin_dir,
                services.tools_manager,
            )
            .map_err(|error| error.to_string())?;
        Ok(retain(definition))
    }
}

/// Erases only concrete argument/detail/state types, keeping the original native definition.
fn retain(
    definition: maestro_extensions::local::ToolDef<crate::builtin_tools::Context>,
) -> Rc<dyn crate::builtin_tools::Definition> {
    use maestro_extensions::local::ToolDef;
    match definition {
        ToolDef::Read(tool) => Rc::new(RwLock::new(tool)),
        ToolDef::Bash(tool) => Rc::new(RwLock::new(tool)),
        ToolDef::Edit(tool) => Rc::new(RwLock::new(tool)),
        ToolDef::Write(tool) => Rc::new(RwLock::new(tool)),
        ToolDef::Grep(tool) => Rc::new(RwLock::new(tool)),
        ToolDef::Find(tool) => Rc::new(RwLock::new(tool)),
        ToolDef::Ls(tool) => Rc::new(RwLock::new(tool)),
    }
}

impl crate::api_reads::Endpoint {
    /// Performs exact-name selection on the original local pump.
    pub(crate) fn create_selected(
        &self,
        name: String,
        cwd: String,
        options: Options,
    ) -> Result<Endpoint, String> {
        self.0.request(|id, reply| {
            Message::BuiltinOperation(Operation::Create(
                Factory::Selected(id, name, cwd, Box::new(options)),
                reply,
            ))
        })
    }
}
