//! Native operations forwarded without introducing a guest clock.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{Imports, signals::Binding};
use crate::{AbortSignal, ExtensionFuture, io::NativeIo};
use std::time::Duration;

/// Independently retained import capability.
pub(super) struct Io<I: Imports>(pub(super) I);

impl<I: Imports> NativeIo for Io<I> {
    fn watch(
        &self,
        path: String,
        listener: crate::fs::WatchListener,
    ) -> crate::ExtensionResult<std::rc::Rc<dyn crate::fs::FsWatcher>> {
        let identity = super::callbacks::Identity::new(&self.0);
        let handle = self.0.watch(path, &identity.handle)?;
        identity.keep(WatchCallback(listener));
        Ok(std::rc::Rc::new(Watcher(
            super::byte_resources::Resource::new(&self.0, handle),
        )))
    }
    fn fetch(
        &self,
        request: crate::http::Request,
    ) -> ExtensionFuture<'_, std::rc::Rc<dyn crate::http::Response>> {
        Box::pin(async move {
            let binding = Binding::new(&self.0, request.signal.as_ref())?;
            let request = crate::bindings::maestro::extension::host::HttpRequest {
                url: request.url,
                method: request.method,
                headers: request.headers,
                body: request.body,
            };
            let handle = self.0.fetch(request, binding.signal()).await?;
            Ok(std::rc::Rc::new(Response {
                resource: super::byte_resources::Resource::new(&self.0, handle),
                binding: std::rc::Rc::new(binding),
                body: std::cell::OnceCell::new(),
            }) as std::rc::Rc<dyn crate::http::Response>)
        })
    }
    fn spawn(
        &self,
        command: String,
        args: Vec<String>,
        options: crate::child_process::SpawnOptions,
    ) -> ExtensionFuture<'_, std::rc::Rc<dyn crate::child_process::ChildProcess>> {
        Box::pin(async move {
            let child = self.0.spawn(command, args, options).await?;
            Ok(
                std::rc::Rc::new(super::byte_resources::Child::new(&self.0, child))
                    as std::rc::Rc<dyn crate::child_process::ChildProcess>,
            )
        })
    }
    fn kill(&self, pid: i32, signal: Option<String>) -> crate::ExtensionResult<()> {
        self.0.kill(pid, signal)
    }

    fn sleep(&self, duration: Duration, signal: Option<AbortSignal>) -> ExtensionFuture<'_, ()> {
        Box::pin(async move {
            let binding = Binding::new(&self.0, signal.as_ref())?;
            let result = self
                .0
                .sleep(
                    (duration.as_secs(), duration.subsec_nanos()),
                    binding.signal(),
                )
                .await;
            binding.settle(result)
        })
    }
}

/// Retains the original response and cancellation binding across callbacks.
struct Response<I: Imports> {
    /// Original native response.
    resource: super::byte_resources::Resource<I, I::Response>,
    /// A mirrored author signal stays attached until the response is released.
    binding: std::rc::Rc<Binding<I>>,
    /// Stable author identity for the same consuming native body.
    body: std::cell::OnceCell<Option<std::rc::Rc<dyn crate::streams::Readable>>>,
}
impl<I: Imports> crate::http::Response for Response<I> {
    fn json(&self) -> ExtensionFuture<'_, serde_json::Value> {
        self.resource.imports.response_json(&self.resource.handle)
    }
    fn text(&self) -> ExtensionFuture<'_, String> {
        self.resource.imports.response_text(&self.resource.handle)
    }
    fn status_text(&self) -> String {
        self.resource
            .imports
            .response_status_text(&self.resource.handle)
    }
    fn headers(&self) -> Vec<(String, String)> {
        self.resource
            .imports
            .response_headers(&self.resource.handle)
    }
    fn body(&self) -> Option<std::rc::Rc<dyn crate::streams::Readable>> {
        self.body
            .get_or_init(|| {
                self.resource
                    .imports
                    .response_body(&self.resource.handle)
                    .map(|handle| {
                        std::rc::Rc::new(Body {
                            reader: super::byte_resources::Resource::new(
                                &self.resource.imports,
                                handle,
                            ),
                            _binding: self.binding.clone(),
                        }) as std::rc::Rc<dyn crate::streams::Readable>
                    })
            })
            .clone()
    }
    fn status(&self) -> u16 {
        self.resource.imports.response_status(&self.resource.handle)
    }
}

/// A body alias keeps authored cancellation forwarding after its response is dropped.
struct Body<I: Imports> {
    /// Original consuming native byte resource.
    reader: super::byte_resources::Resource<I, I::Readable>,
    /// Shared response-lifetime binding, not a second signal subscription.
    _binding: std::rc::Rc<Binding<I>>,
}
impl<I: Imports> crate::streams::Readable for Body<I> {
    fn read(&self) -> ExtensionFuture<'_, Option<Vec<u8>>> {
        self.reader.imports.readable_read(&self.reader.handle)
    }
    fn close(&self) -> crate::ExtensionResult<()> {
        self.reader.imports.readable_close(&self.reader.handle)
    }
}

/// Owns the original native watch resource, not a separate callback registration.
struct Watcher<I: Imports>(super::byte_resources::Resource<I, I::Watcher>);
impl<I: Imports> crate::fs::FsWatcher for Watcher<I> {
    fn close(&self) -> crate::ExtensionResult<()> {
        self.0.imports.watcher_close(&self.0.handle)
    }
}
impl<I: Imports> super::Exports<I> {
    /// Runs one native notification under its retained callback identity.
    pub(crate) fn invoke_watch(
        &self,
        handler: u32,
        emitter: &I::Emitter,
    ) -> crate::ExtensionResult<()> {
        let listener = super::callbacks::find::<WatchCallback>(handler)?;
        let port = super::event_bus::Emitter {
            imports: &self.imports,
            handle: emitter,
        };
        (listener.0)(crate::CallbackEmitter::new(&port))
    }
}

/// Distinguishes repeated watch delivery from the same-shaped one-shot abort listener.
#[derive(Clone)]
pub(super) struct WatchCallback(pub(super) crate::fs::WatchListener);
