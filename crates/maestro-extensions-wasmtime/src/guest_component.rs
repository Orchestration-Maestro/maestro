//! Retained Local components expose guest failures through the existing fallible render path.
use crate::{
    guest_calls::{
        Call, Handle,
        renderers::{ComponentCall, ComponentMethod},
    },
    local_resources::{Registry, effects::EmitterEndpoint},
};
use maestro_extensions::{
    Component, EventBus, InputHandler, Local,
    widgets::{FocusFlag, Focusable},
};
use std::cell::RefCell;

/// A native view owns the actual guest component and its independent admission.
pub(crate) struct GuestComponent {
    /// Final native release queues destruction on the original Store driver.
    owner: Handle,
    /// Original capability registry, not another receiver.
    registry: Registry,
    /// Each synchronous call gets the original bus's native invocation frame.
    events: EventBus<Local>,
    /// The native observation of the authored focus flag.
    focus: FocusFlag,
    /// Infallible methods retain their first error until the next fallible render.
    error: RefCell<Option<String>>,
}
impl GuestComponent {
    /// Retains the resource returned by the actual authored renderer.
    pub(crate) fn new(owner: Handle, registry: Registry, events: EventBus<Local>) -> Self {
        Self {
            owner,
            registry,
            events,
            error: RefCell::default(),
            focus: FocusFlag::default(),
        }
    }
    /// Calls the original synchronous waiter with a fresh native invocation frame.
    fn invoke<T: Send + 'static>(
        &self,
        call: impl FnOnce(EmitterEndpoint, crate::lifecycle::replies::Reply<T>) -> ComponentCall,
    ) -> Result<T, String> {
        self.events
            .invoke_callback(|emitter| -> Result<T, maestro_extensions::EventError> {
                let emitter = self.registry.emitter(emitter.frame())?;
                self.owner
                    .endpoint()
                    .synchronous(&self.registry, |reply| {
                        Ok(Call::Component(call(emitter, reply)))
                    })
                    .map_err(Into::into)
            })
            .map_err(|error| error.to_string())
    }
    /// Keeps failures from methods whose toolkit signature cannot return an error.
    fn retain_error<T: Default>(&self, result: Result<T, String>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => {
                self.error.borrow_mut().get_or_insert(error);
                T::default()
            }
        }
    }
    /// Reads a pending failure before entering the guest for a new render.
    fn lines(&self, width: usize) -> Result<Vec<String>, String> {
        if let Some(error) = self.error.borrow_mut().take() {
            return Err(error);
        }
        let width = u32::try_from(width).map_err(|error| error.to_string())?;
        self.invoke(|emitter, reply| {
            ComponentCall::Render(self.owner.clone(), width, emitter, reply)
        })
    }
}
impl Component for GuestComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.retain_error(self.lines(width))
    }
    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        self.lines(width).map_err(Into::into)
    }
    fn invalidate(&self) {
        self.retain_error(self.invoke(|emitter, reply| {
            ComponentCall::Unit(
                self.owner.clone(),
                ComponentMethod::Invalidate,
                emitter,
                reply,
            )
        }));
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        let focused =
            self.retain_error(self.invoke(|emitter, reply| {
                ComponentCall::Focused(self.owner.clone(), emitter, reply)
            }))?;
        self.focus.set(focused);
        Some(self)
    }
    fn wants_key_release(&self) -> bool {
        self.retain_error(self.invoke(|emitter, reply| {
            ComponentCall::WantsKeyRelease(self.owner.clone(), emitter, reply)
        }))
    }
}
impl InputHandler for GuestComponent {
    fn handle_input(&self, data: &str) {
        self.retain_error(self.invoke(|emitter, reply| {
            ComponentCall::Unit(
                self.owner.clone(),
                ComponentMethod::Input(data.into()),
                emitter,
                reply,
            )
        }));
    }
    fn try_handle_input(&self, data: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.invoke(|emitter, reply| {
            ComponentCall::Unit(
                self.owner.clone(),
                ComponentMethod::Input(data.into()),
                emitter,
                reply,
            )
        })
        .map_err(Into::into)
    }
}

impl Focusable for GuestComponent {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
    fn set_focused(&self, focused: bool) {
        self.focus.set(focused);
        self.retain_error(self.invoke(|emitter, reply| {
            ComponentCall::Unit(
                self.owner.clone(),
                ComponentMethod::SetFocused(focused),
                emitter,
                reply,
            )
        }));
    }
}
