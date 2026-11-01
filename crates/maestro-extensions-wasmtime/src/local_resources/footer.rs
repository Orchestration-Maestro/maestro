//! Footer metadata lent to a footer factory, served on the local owner.
use super::{Capability, pump::State};

impl State {
    /// Serves the lent footer metadata after releasing the capability-table borrow.
    pub(super) fn footer(&self, id: u64, request: crate::footer_data::Request) {
        let capability = self.registry.capability(id);
        match capability.as_deref() {
            Some(Capability::Footer(footer)) => request.answer(footer, &self.registry),
            _ => request.reject("extension footer resource ended"),
        }
    }
}
