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
    /// Read with its independently selected image policy.
    Read(String, Option<bool>, Option<crate::guest_calls::Handle>),
    /// Edit with native argument preparation and file operations.
    Edit(String),
}
impl Registry {
    /// Constructs the original typed definition and gives it an independent resource lease.
    pub(super) fn create_builtin(&self, factory: Factory) -> Result<Endpoint, String> {
        let definition: Rc<dyn crate::builtin_tools::Definition> = match factory {
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
            Factory::Edit(cwd) => Rc::new(RwLock::new(
                maestro_extensions::local::create_edit_tool_definition::<
                    crate::builtin_tools::Context,
                >(cwd, maestro_extensions::EditToolOptions::default()),
            )),
        };
        self.insert(Capability::Builtin(definition)).map(Endpoint)
    }
}
impl crate::api_reads::Endpoint {
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
    /// Retains native edit preparation without requiring a context or accessing a file.
    pub(crate) fn create_edit(&self, cwd: String) -> Result<Endpoint, String> {
        self.0.request(|_, reply| {
            Message::BuiltinOperation(Operation::Create(Factory::Edit(cwd), reply))
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
            let invocation = crate::builtin_tools::Invocation {
                id: execution.id,
                args: crate::writer_reads::decode(&execution.args)?,
                signal: execution
                    .signal
                    .as_ref()
                    .map(|signal| self.registry.cancellation(signal))
                    .transpose()?,
            };
            Ok(definition.clone().execute(invocation, model))
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

/// Native built-in operations share the original pump's admission and result owners.
pub(crate) enum Operation {
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
