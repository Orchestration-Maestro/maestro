//! Live sandbox message dispatch and registered-window admission.
use maestro_app::browser::components::sandboxed_iframe::SandboxValue;
use maestro_app::{BoxFuture, DiagnosticErrorInfo};

/// Receives each sandbox message after its runtime providers.
pub trait MessageConsumer<V: SandboxValue> {
    /// Handle the retained message, preserving asynchronous failure.
    fn handle_message(&self, message: V) -> BoxFuture<Result<(), DiagnosticErrorInfo>>;
}
/// Shared native-tested state, compiled only where routing is used.
#[cfg(any(target_arch = "wasm32", test))]
mod routing {
    use super::{MessageConsumer, SandboxValue};
    use maestro_app::browser::components::sandbox::sandbox_runtime_provider::SandboxRuntimeProvider;
    use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};
    /// Retained providers whose mutations remain visible during dispatch.
    pub(super) type Providers<V> = Rc<RefCell<Vec<Arc<dyn SandboxRuntimeProvider<V>>>>>;
    /// Mutable frame retained independently by response callbacks.
    type Frame<F> = Rc<RefCell<Option<Rc<F>>>>;
    /// Sandbox context captured by an admitted message.
    pub(super) struct SandboxContext<V: SandboxValue, F> {
        /// Current reply destination.
        pub(super) frame: Frame<F>,
        /// Caller-owned provider collection.
        pub(super) providers: Providers<V>,
        /// Stable insertion positions, including removed entries.
        pub(super) consumers: RefCell<Vec<Option<Arc<dyn MessageConsumer<V>>>>>,
        /// Live allocation identities and their insertion positions.
        pub(super) positions: RefCell<HashMap<usize, usize>>,
    }
    /// Registry shared by both message channels.
    pub(super) struct Registry<V: SandboxValue, F> {
        /// Replaceable entries; admitted contexts survive replacement.
        pub(super) sandboxes: RefCell<HashMap<String, Rc<SandboxContext<V, F>>>>,
    }
    impl<V: SandboxValue, F> Default for Registry<V, F> {
        fn default() -> Self {
            Self {
                sandboxes: RefCell::new(HashMap::new()),
            }
        }
    }
    impl<V: SandboxValue, F> Registry<V, F> {
        /// Whether global listeners can be released.
        pub(super) fn is_empty(&self) -> bool {
            self.sandboxes.borrow().is_empty()
        }
        /// Replace a registry entry while preserving admitted contexts.
        pub(super) fn register(
            &self,
            id: String,
            providers: Providers<V>,
            consumers: Vec<Arc<dyn MessageConsumer<V>>>,
        ) {
            let context = Rc::new(SandboxContext {
                frame: Rc::new(RefCell::new(None)),
                providers,
                consumers: RefCell::new(Vec::new()),
                positions: RefCell::new(HashMap::new()),
            });
            for consumer in consumers {
                context.add(consumer);
            }
            let old = self.sandboxes.borrow_mut().insert(id, context);
            drop(old);
        }
        /// Remove the current entry without invalidating captured contexts.
        pub(super) fn unregister(&self, id: &str) {
            let old = self.sandboxes.borrow_mut().remove(id);
            drop(old);
        }
        /// Update only the current context's reply destination.
        pub(super) fn set_frame(&self, id: &str, frame: F) {
            let context = self.sandboxes.borrow().get(id).cloned();
            if let Some(context) = context {
                let old = context.frame.replace(Some(Rc::new(frame)));
                drop(old);
            }
        }
        /// Capture a context only for a nonempty string identifier.
        pub(super) fn context(&self, id: Option<&str>) -> Option<Rc<SandboxContext<V, F>>> {
            self.sandboxes
                .borrow()
                .get(id.filter(|id| !id.is_empty())?)
                .cloned()
        }
        /// Admit only the current registered frame's actual source.
        pub(super) fn window_context<S: PartialEq>(
            &self,
            id: Option<&str>,
            source: Option<&S>,
            window: impl FnOnce(&F) -> Option<S>,
        ) -> Option<Rc<SandboxContext<V, F>>> {
            let context = self.context(id)?;
            let frame = context.frame.borrow().clone()?;
            let expected = window(&frame)?;
            (source == Some(&expected)).then_some(context)
        }
    }
    impl<V: SandboxValue, F> SandboxContext<V, F> {
        /// Remove identity without shifting any visitation position.
        pub(super) fn remove(&self, consumer: &Arc<dyn MessageConsumer<V>>) {
            let key = Arc::as_ptr(consumer).cast::<()>() as usize;
            let index = self.positions.borrow_mut().remove(&key);
            if let Some(index) = index {
                let removed = self.consumers.borrow_mut()[index].take();
                drop(removed);
            }
        }

        /// Insert a new allocation once at a stable position.
        pub(super) fn add(&self, consumer: Arc<dyn MessageConsumer<V>>) {
            let key = Arc::as_ptr(&consumer).cast::<()>() as usize;
            let mut positions = self.positions.borrow_mut();
            if let std::collections::hash_map::Entry::Vacant(entry) = positions.entry(key) {
                let mut consumers = self.consumers.borrow_mut();
                entry.insert(consumers.len());
                consumers.push(Some(consumer));
            }
        }
    }
}
#[cfg(any(target_arch = "wasm32", test))]
impl<V: SandboxValue + Clone, F> SandboxContext<V, F> {
    /// Walk current providers, then stable live consumer positions.
    pub(super) async fn dispatch(
        &self,
        message: V,
        respond: SandboxRespond<V>,
    ) -> Result<(), DiagnosticErrorInfo> {
        let mut index = 0;
        loop {
            let Some(provider) = self.providers.borrow().get(index).cloned() else {
                break;
            };
            provider
                .handle_message(message.clone(), respond.clone())
                .await?;
            index += 1;
        }
        index = 0;
        loop {
            let Some(consumer) = self.consumers.borrow().get(index).cloned() else {
                break;
            };
            if let Some(consumer) = consumer {
                consumer.handle_message(message.clone()).await?;
            }
            index += 1;
        }
        Ok(())
    }
}
#[cfg(any(target_arch = "wasm32", test))]
use maestro_app::browser::components::sandbox::sandbox_runtime_provider::SandboxRespond;
#[cfg(any(target_arch = "wasm32", test))]
use routing::{Providers, Registry, SandboxContext};
#[cfg(test)]
mod tests;

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::{RUNTIME_MESSAGE_ROUTER, RuntimeMessageRouter};
