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
use wasmtime::component::{ComponentType, Lift, Lower};

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
        Ok(retain(definition).1)
    }
}

/// Erases only concrete argument/detail/state types, keeping the original native definition.
fn retain(
    definition: maestro_extensions::local::ToolDef<crate::builtin_tools::Context>,
) -> (Name, Rc<dyn crate::builtin_tools::Definition>) {
    use maestro_extensions::local::ToolDef;
    match definition {
        ToolDef::Read(tool) => (Name::Read, Rc::new(RwLock::new(tool))),
        ToolDef::Bash(tool) => (Name::Bash, Rc::new(RwLock::new(tool))),
        ToolDef::Edit(tool) => (Name::Edit, Rc::new(RwLock::new(tool))),
        ToolDef::Write(tool) => (Name::Write, Rc::new(RwLock::new(tool))),
        ToolDef::Grep(tool) => (Name::Grep, Rc::new(RwLock::new(tool))),
        ToolDef::Find(tool) => (Name::Find, Rc::new(RwLock::new(tool))),
        ToolDef::Ls(tool) => (Name::Ls, Rc::new(RwLock::new(tool))),
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

/// Closed native preset selection; order remains with the native factories.
#[derive(Clone, Copy, ComponentType, Lift)]
#[component(enum)]
#[repr(u8)]
#[expect(
    dead_code,
    reason = "constructed by the Wasmtime Lift macro from wire values; host code only matches them"
)]
pub(crate) enum Preset {
    /// Native coding preset.
    #[component(name = "coding")]
    Coding,
    /// Native read-only preset.
    #[component(name = "read-only")]
    ReadOnly,
    /// Every native built-in.
    #[component(name = "all")]
    All,
}

/// Binary transport of native built-in identity, independent of custom names.
#[derive(Clone, Copy, ComponentType, Lower)]
#[component(enum)]
#[repr(u8)]
pub(crate) enum Name {
    /// Native read identity.
    #[component(name = "read")]
    Read,
    /// Native shell identity.
    #[component(name = "bash")]
    Bash,
    /// Native edit identity.
    #[component(name = "edit")]
    Edit,
    /// Native write identity.
    #[component(name = "write")]
    Write,
    /// Native content-search identity.
    #[component(name = "grep")]
    Grep,
    /// Native file-search identity.
    #[component(name = "find")]
    Find,
    /// Native listing identity.
    #[component(name = "ls")]
    Ls,
}

impl Registry {
    /// Admits only the ordered collection the native preset returned.
    pub(super) fn preset(
        &self,
        api: u64,
        preset: Preset,
        cwd: String,
        options: Options,
    ) -> Result<Vec<(Name, Endpoint)>, String> {
        use maestro_extensions::local::{
            create_all_tool_definitions, create_coding_tool_definitions,
            create_read_only_tool_definitions,
        };
        let services = self.tool_construction(api)?;
        let options = options.native(self, api)?;
        let definitions = match preset {
            Preset::Coding => create_coding_tool_definitions(
                cwd,
                options,
                services.bin_dir,
                services.tools_manager,
            ),
            Preset::ReadOnly => create_read_only_tool_definitions(
                cwd,
                options,
                services.bin_dir,
                services.tools_manager,
            ),
            Preset::All => {
                create_all_tool_definitions(cwd, options, services.bin_dir, services.tools_manager)
                    .into_values()
                    .collect()
            }
        };
        definitions
            .into_iter()
            .map(|definition| {
                let (name, definition) = retain(definition);
                self.insert(super::Capability::Builtin(definition))
                    .map(|lease| (name, Endpoint(lease)))
            })
            .collect()
    }
}

impl crate::api_reads::Endpoint {
    /// Returns the original native preset through the same local pump.
    pub(crate) fn create_preset(
        &self,
        preset: Preset,
        cwd: String,
        options: Options,
    ) -> Result<Vec<(Name, Endpoint)>, String> {
        self.0.request(|id, reply| {
            Message::BuiltinOperation(Operation::Preset(id, preset, cwd, Box::new(options), reply))
        })
    }
}
