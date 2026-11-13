//! OAuth interaction requests use their original local owner.
use super::State;
impl State {
    /// Keeps reverse input on the same pending-work owner as native authorization.
    pub(super) fn oauth_interaction(&self, request: crate::oauth_interaction::Request) {
        if let Some((work, admission)) = request.answer() {
            self.admit(work, admission);
        }
    }
}
