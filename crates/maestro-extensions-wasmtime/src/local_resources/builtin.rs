//! Native tool resource ownership on the original local request pump.
use super::{Admission, Capability, Message, Registry, pump::State};
use crate::builtin_reads::{Endpoint, Execution, Request};
use maestro_extensions::ReadToolOptions;
use std::{
    rc::Rc,
    sync::{Arc, RwLock},
};

impl Registry {
    /// Constructs the native definition lazily and gives it an independent resource lease.
    pub(super) fn create_read(
        &self,
        cwd: String,
        auto_resize_images: Option<bool>,
    ) -> Result<Endpoint, String> {
        let definition = maestro_extensions::local::create_read_tool_definition::<
            crate::builtin_tools::Context,
        >(
            cwd,
            ReadToolOptions {
                auto_resize_images,
                operations: None,
            },
        );
        self.insert(Capability::Builtin(Rc::new(RwLock::new(definition))))
            .map(Endpoint)
    }
}
impl crate::api_reads::Endpoint {
    /// Retains a native read without retaining the API or selecting a callback context.
    pub(crate) fn create_read(
        &self,
        cwd: String,
        resize: Option<bool>,
    ) -> Result<Endpoint, String> {
        self.0
            .request(|_, reply| Message::CreateRead(cwd, resize, reply))
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
