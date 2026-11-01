//! Dialogs on the originally acquired UI, admitted on the local owner.
use super::{Admission, Capability, pump::State};
use crate::ui_reads::DialogRequest;

impl State {
    /// Resolves the original UI and cancellation, then admits the dialog until it settles.
    pub(super) fn dialog(&self, id: u64, request: DialogRequest, admission: Admission) {
        let capability = self.registry.capability(id);
        let Some(Capability::Ui(ui)) = capability.as_deref() else {
            request.reject("extension UI resource ended");
            return;
        };
        let signal = request
            .signal
            .as_ref()
            .map(|signal| self.registry.cancellation(signal))
            .transpose();
        match signal {
            Ok(signal) => self.admit(request.start(ui, signal, &self.registry), admission),
            Err(error) => request.reject(&error),
        }
    }
}
