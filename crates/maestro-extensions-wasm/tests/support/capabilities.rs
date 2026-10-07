use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use maestro_extensions_wasm::{Error, EventBus, EventBusHandler, ListenerTail, Unsubscribe};
use serde_json::Value;

#[derive(Default)]
pub struct ControlledBus {
    listeners: Rc<RefCell<BTreeMap<usize, (String, EventBusHandler)>>>,
    next_id: RefCell<usize>,
    pub tails: RefCell<Vec<ListenerTail>>,
}

impl EventBus for ControlledBus {
    fn emit(&self, channel: &str, data: Value) -> Result<(), Error> {
        let handlers: Vec<_> = self
            .listeners
            .borrow()
            .values()
            .filter(|(name, _)| name == channel)
            .map(|(_, handler)| Rc::clone(handler))
            .collect();
        for handler in handlers {
            if let Some(tail) = handler(data.clone())? {
                self.tails.borrow_mut().push(tail);
            }
        }
        Ok(())
    }

    fn on(&self, channel: &str, handler: EventBusHandler) -> Result<Unsubscribe, Error> {
        let id = *self.next_id.borrow();
        *self.next_id.borrow_mut() += 1;
        self.listeners
            .borrow_mut()
            .insert(id, (channel.into(), handler));
        let listeners = Rc::clone(&self.listeners);
        Ok(Box::new(move || {
            let removed = listeners.borrow_mut().remove(&id);
            drop(removed);
            Ok(())
        }))
    }
}
