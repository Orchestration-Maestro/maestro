//! Native tool resource ownership on the original local request pump.
use super::{Admission, Capability, Message, Registry, pump::State};
use crate::builtin_reads::{Endpoint, Execution, Request};
use maestro_extensions::ReadToolOptions;
use std::{
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Constructor operands of native definitions, without executing their callbacks.
pub(crate) enum Factory {
    /// The original named definition selector with all independent options.
    Selected(u64, String, String, Box<super::builtin_selection::Options>),
    /// Native shell construction on the original facade's supplied binary directory.
    Bash(u64, String, super::builtin_bash::Options),
    /// Read with its independently selected image policy.
    Read(String, Option<bool>, Option<crate::guest_calls::Handle>),
    /// Content search using the application search services.
    Grep(u64, String, Option<crate::guest_calls::Handle>),
    /// Find using the application's original search manager.
    Find(u64, String, Option<crate::guest_calls::Handle>),
    /// Edit with native argument preparation and file operations.
    Edit(String, Option<crate::guest_calls::Handle>),
    /// Write through native ordering and independently retained effects.
    Write(String, Option<crate::guest_calls::Handle>),
    /// Directory listing with native ordering and output selection.
    Ls(String, Option<crate::guest_calls::Handle>),
}
impl Registry {
    /// Constructs the original typed definition and gives it an independent resource lease.
    pub(super) fn create_builtin(&self, factory: Factory) -> Result<Endpoint, String> {
        let definition: Rc<dyn crate::builtin_tools::Definition> = match factory {
            Factory::Selected(api, name, cwd, options) => {
                self.selected(api, &name, cwd, *options)?
            }
            Factory::Bash(api, cwd, options) => self.bash(api, cwd, options)?,
            Factory::Ls(cwd, operations) => Self::ls(cwd, operations),
            Factory::Grep(api, cwd, operations) => self.grep(api, cwd, operations)?,
            Factory::Find(api, cwd, operations) => self.find(api, cwd, operations)?,
            Factory::Read(cwd, auto_resize_images, operations) => Rc::new(RwLock::new(
                maestro_extensions::local::create_read_tool_definition::<
                    crate::builtin_tools::Context,
                >(
                    cwd,
                    ReadToolOptions {
                        auto_resize_images,
                        operations: operations.map(|owner| {
                            Arc::new(crate::guest_calls::read::Operations(owner))
                                as Arc<dyn maestro_extensions::ReadOperations>
                        }),
                    },
                ),
            )),
            Factory::Write(cwd, operations) => Rc::new(RwLock::new(
                maestro_extensions::local::create_write_tool_definition::<
                    crate::builtin_tools::Context,
                >(
                    cwd,
                    maestro_extensions::WriteToolOptions {
                        operations: operations.map(|owner| {
                            Arc::new(crate::guest_calls::write::Operations(owner))
                                as Arc<dyn maestro_extensions::WriteOperations>
                        }),
                    },
                ),
            )),
            Factory::Edit(cwd, operations) => Rc::new(RwLock::new(
                maestro_extensions::local::create_edit_tool_definition::<
                    crate::builtin_tools::Context,
                >(
                    cwd,
                    maestro_extensions::EditToolOptions {
                        operations: operations.map(|owner| {
                            Arc::new(crate::guest_calls::edit::Operations(owner))
                                as Arc<dyn maestro_extensions::EditOperations>
                        }),
                    },
                ),
            )),
        };
        self.insert(Capability::Builtin(definition)).map(Endpoint)
    }
}
impl Registry {
    /// Constructs the original listing definition without filesystem work.
    fn ls(
        cwd: String,
        operations: Option<crate::guest_calls::Handle>,
    ) -> Rc<dyn crate::builtin_tools::Definition> {
        Rc::new(RwLock::new(
            maestro_extensions::local::create_ls_tool_definition::<crate::builtin_tools::Context>(
                cwd,
                maestro_extensions::LsToolOptions {
                    operations: operations.map(|owner| {
                        Arc::new(crate::guest_calls::ls::Operations(owner))
                            as Arc<dyn maestro_extensions::LsOperations>
                    }),
                },
            ),
        ))
    }
    /// Constructs the original grep definition on the application-owned manager.
    fn grep(
        &self,
        api: u64,
        cwd: String,
        operations: Option<crate::guest_calls::Handle>,
    ) -> Result<Rc<dyn crate::builtin_tools::Definition>, String> {
        let services = self.tool_construction(api)?;
        Ok(Rc::new(RwLock::new(
            maestro_extensions::local::create_grep_tool_definition::<crate::builtin_tools::Context>(
                cwd,
                maestro_extensions::GrepToolOptions {
                    operations: operations.map(|owner| {
                        Arc::new(crate::guest_calls::grep::Operations(owner))
                            as Arc<dyn maestro_extensions::GrepOperations>
                    }),
                },
                services.tools_manager,
            ),
        )))
    }
    /// Constructs the original find definition on the application-owned manager.
    fn find(
        &self,
        api: u64,
        cwd: String,
        operations: Option<crate::guest_calls::Handle>,
    ) -> Result<Rc<dyn crate::builtin_tools::Definition>, String> {
        let services = self.tool_construction(api)?;
        Ok(Rc::new(RwLock::new(
            maestro_extensions::local::create_find_tool_definition::<crate::builtin_tools::Context>(
                cwd,
                maestro_extensions::FindToolOptions {
                    operations: operations.map(|owner| {
                        Arc::new(crate::guest_calls::find::Operations(owner))
                            as Arc<dyn maestro_extensions::FindOperations>
                    }),
                },
                services.tools_manager,
            ),
        )))
    }
    /// Reads application-supplied factory services outside the capability-table borrow.
    pub(super) fn tool_construction(
        &self,
        api: u64,
    ) -> Result<maestro_extensions::ToolConstruction, String> {
        match self.capability(api).as_deref() {
            Some(Capability::Api(api)) => api.tool_construction(),
            _ => Err("extension API resource ended".into()),
        }
    }
}
impl crate::api_reads::Endpoint {
    /// Retains native listing without observing directories during construction.
    pub(crate) fn create_ls(
        &self,
        cwd: String,
        operations: Option<crate::guest_calls::Handle>,
    ) -> Result<Endpoint, String> {
        self.0.request(|_, reply| {
            Message::BuiltinOperation(Operation::Create(Factory::Ls(cwd, operations), reply))
        })
    }
    /// Retains native content search with the original facade's services.
    pub(crate) fn create_grep(
        &self,
        cwd: String,
        operations: Option<crate::guest_calls::Handle>,
    ) -> Result<Endpoint, String> {
        self.0.request(|id, reply| {
            Message::BuiltinOperation(Operation::Create(Factory::Grep(id, cwd, operations), reply))
        })
    }
    /// Retains a finder using services from the original native facade.
    pub(crate) fn create_find(
        &self,
        cwd: String,
        operations: Option<crate::guest_calls::Handle>,
    ) -> Result<Endpoint, String> {
        self.0.request(|id, reply| {
            Message::BuiltinOperation(Operation::Create(Factory::Find(id, cwd, operations), reply))
        })
    }
    /// Retains a native read without retaining the API or selecting a callback context.
    pub(crate) fn create_read(
        &self,
        cwd: String,
        resize: Option<bool>,
        operations: Option<crate::guest_calls::Handle>,
    ) -> Result<Endpoint, String> {
        self.0.request(|_, reply| {
            Message::BuiltinOperation(Operation::Create(
                Factory::Read(cwd, resize, operations),
                reply,
            ))
        })
    }
    /// Retains native write policy without performing its effects.
    pub(crate) fn create_write(
        &self,
        cwd: String,
        operations: Option<crate::guest_calls::Handle>,
    ) -> Result<Endpoint, String> {
        self.0.request(|_, reply| {
            Message::BuiltinOperation(Operation::Create(Factory::Write(cwd, operations), reply))
        })
    }
    /// Retains native edit preparation without requiring a context or accessing a file.
    pub(crate) fn create_edit(
        &self,
        cwd: String,
        operations: Option<crate::guest_calls::Handle>,
    ) -> Result<Endpoint, String> {
        self.0.request(|_, reply| {
            Message::BuiltinOperation(Operation::Create(Factory::Edit(cwd, operations), reply))
        })
    }
}
impl State {
    /// Reads the original native owner after releasing the capability-table borrow.
    pub(super) fn builtin(&self, id: u64, request: Request) {
        let capability = self.registry.capability(id);
        if let Some(Capability::Builtin(definition)) = capability.as_deref() {
            request.answer(&**definition);
        } else {
            request.reject("extension built-in tool resource ended");
        }
    }
    /// Starts native execution with the caller's model and independent cancellation.
    pub(super) fn builtin_execute(
        &self,
        id: u64,
        execution: Execution,
        reply: tokio::sync::oneshot::Sender<Result<String, String>>,
        admission: Admission,
    ) {
        let capability = self.registry.capability(id);
        let result = (|| {
            let Some(Capability::Builtin(definition)) = capability.as_deref() else {
                return Err("extension built-in tool resource ended".to_owned());
            };
            let model = execution
                .model
                .as_deref()
                .map(crate::writer_reads::decode)
                .transpose()?
                .map(Arc::new);
            let initial_error = Rc::new(std::cell::RefCell::new(None));
            let update = execution
                .update
                .map(|handler| self.builtin_update(handler, &execution.api, initial_error.clone()))
                .transpose()?;
            let invocation = crate::builtin_tools::Invocation {
                id: execution.id,
                args: crate::writer_reads::decode(&execution.args)?,
                signal: execution
                    .signal
                    .as_ref()
                    .map(|signal| self.registry.cancellation(signal))
                    .transpose()?,
                update,
            };
            let work = definition.clone().execute(invocation, model);
            // Native initial progress runs synchronously, before the backend future is polled.
            initial_error.borrow_mut().take().map_or(Ok(work), Err)
        })();
        match result {
            Ok(work) => self.admit(
                Box::pin(async move {
                    let result = work
                        .await
                        .map_err(|error| error.to_string())
                        .and_then(|result| {
                            serde_json::to_string(&result).map_err(|error| error.to_string())
                        });
                    let _ = reply.send(result);
                }),
                admission,
            ),
            Err(error) => {
                let _ = reply.send(Err(error));
            }
        }
    }
}

impl State {
    /// Delivers native progress through the existing synchronous frame and diagnostic owner.
    fn builtin_update(
        &self,
        handler: crate::guest_calls::Handle,
        api: &crate::api_reads::Endpoint,
        initial_error: Rc<std::cell::RefCell<Option<String>>>,
    ) -> Result<
        maestro_extensions::AgentToolUpdateCallback<serde_json::Value, maestro_extensions::Local>,
        String,
    > {
        let events = self.registry.events(api)?;
        let registry = self.registry.clone();
        Ok(Rc::new(move |partial| {
            let result = serde_json::to_string(&partial)
                .map_err(|error| error.to_string())
                .and_then(|data| {
                    crate::compact_calls::notify(
                        &handler,
                        &registry,
                        &events,
                        crate::guest_calls::SynchronousKind::Listener,
                        data,
                    )
                });
            if let Err(error) = result {
                events.report_error("tool-update", &std::io::Error::other(error.clone()));
                *initial_error.borrow_mut() = Some(error);
            }
        }))
    }
}

/// Native built-in operations share the original pump's admission and result owners.
pub(crate) enum Operation {
    /// Retains the original ordered native preset.
    Preset(
        u64,
        super::builtin_selection::Preset,
        String,
        Box<super::builtin_selection::Options>,
        std::sync::mpsc::Sender<Result<Vec<(super::builtin_selection::Name, Endpoint)>, String>>,
    ),
    /// Constructs an independently retained original local backend.
    LocalShell(
        u64,
        Option<String>,
        std::sync::mpsc::Sender<Result<super::local_bash::Endpoint, String>>,
    ),
    /// Executes local raw-byte operations through the original request pump.
    LocalExec(
        u64,
        super::local_bash::Execution,
        tokio::sync::oneshot::Sender<Result<Option<f64>, String>>,
        Admission,
    ),
    /// Creates and retains an original typed native definition.
    Create(
        Factory,
        std::sync::mpsc::Sender<Result<crate::builtin_reads::Endpoint, String>>,
    ),
    /// Executes a retained native tool without blocking request consumption.
    Execute(
        u64,
        crate::builtin_reads::Execution,
        tokio::sync::oneshot::Sender<Result<String, String>>,
        Admission,
    ),
    /// Renders through an original typed native definition and lent native row.
    Render(
        u64,
        crate::native_renders::Render,
        std::sync::mpsc::Sender<Result<crate::import_route::Value, String>>,
    ),
    /// Uses one original retained native component.
    Component(
        u64,
        crate::native_renders::ComponentRequest,
        std::sync::mpsc::Sender<Result<crate::import_route::Value, String>>,
    ),
}
impl State {
    /// Dispatches one native built-in operation without adding another request receiver.
    pub(super) fn builtin_operation(&self, operation: Operation) {
        match operation {
            Operation::Preset(api, preset, cwd, options, reply) => {
                let _ = reply.send(self.registry.preset(api, preset, cwd, *options));
            }
            Operation::LocalShell(api, shell_path, reply) => {
                let _ = reply.send(self.registry.local_shell(api, shell_path));
            }
            Operation::LocalExec(id, execution, reply, admission) => {
                self.local_exec(id, execution, reply, admission);
            }
            Operation::Create(factory, reply) => {
                let _ = reply.send(self.registry.create_builtin(factory));
            }
            Operation::Execute(id, execution, reply, admission) => {
                self.builtin_execute(id, execution, reply, admission);
            }
            Operation::Render(id, render, reply) => {
                let _ = reply.send(self.builtin_render(id, render));
            }
            Operation::Component(id, request, reply) => {
                let _ = reply.send(self.native_component(id, request));
            }
        }
    }
}
