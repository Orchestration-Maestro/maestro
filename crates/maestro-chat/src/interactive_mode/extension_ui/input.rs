//! Identity-preserving adaptation of bound extension input to toolkit listeners.
use super::super::InteractiveMode;
use maestro_app::{Listeners, TerminalInputHandler, UIUnsubscribe};
use maestro_tui::tui::InputListenerResult;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// The current toolkit removal operation for each bound callback identity.
type InputCache = HashMap<TerminalInputHandler, Box<dyn Fn()>>;

/// Tracked cleanup operations in registration order.
type Removers = Rc<RefCell<Listeners<Rc<dyn Fn()>>>>;

/// Extension-only input registrations, separate from unrelated toolkit listeners.
#[derive(Default)]
pub(in super::super) struct ExtensionInput {
    /// Current toolkit registration indexed by the originating callback allocation.
    cache: Rc<RefCell<InputCache>>,
    /// Removers retained until explicit removal or cleanup.
    removers: Removers,
}

impl ExtensionInput {
    /// Registers one toolkit listener per callback identity and tracks each remover.
    pub(super) fn register(
        &self,
        mode: &InteractiveMode,
        handler: TerminalInputHandler,
    ) -> UIUnsubscribe {
        if !self.cache.borrow().contains_key(&handler) {
            let remove = listen(mode, handler.clone());
            self.cache.borrow_mut().insert(handler.clone(), remove);
        }
        let cache = Rc::downgrade(&self.cache);
        let remove: Rc<dyn Fn()> = Rc::new(move || {
            let Some(cache) = cache.upgrade() else {
                return;
            };
            let removed = cache.borrow_mut().remove(&handler);
            if let Some(remove) = removed {
                remove();
            }
        });
        let untrack = Listeners::subscribe(&self.removers, remove.clone());
        Rc::new(move || {
            remove();
            untrack();
            Ok(())
        })
    }

    /// Removes the tracked extension listeners, leaving other toolkit listeners alone.
    pub(in super::super) fn clear(&self) {
        let mut removers = std::mem::take(&mut *self.removers.borrow_mut());
        for remove in removers.snapshot() {
            remove();
        }
    }
}

impl Drop for ExtensionInput {
    fn drop(&mut self) {
        self.clear();
    }
}

/// Installs the callback through the toolkit, consuming and reporting a failed chunk.
fn listen(mode: &InteractiveMode, callback: TerminalInputHandler) -> Box<dyn Fn()> {
    let runtime = mode.tool_environment.runtime.clone();
    mode.ui.add_input_listener(Rc::new(move |data| {
        callback.call(data.to_owned()).unwrap_or_else(|error| {
            let error: Box<dyn std::error::Error> = error;
            runtime.spawn_local(Box::pin(std::future::ready(Err(error))));
            InputListenerResult::Consume
        })
    }))
}
