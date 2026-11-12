//! Retained credential, tree-node and OAuth requests use their original local owners.
use super::{Admission, Capability, State};
impl State {
    /// Retains accepted credentials and their selected effects until the key settles.
    pub(super) fn start_credential_key(
        &self,
        id: u64,
        (provider, include): (String, Option<bool>),
        reply: tokio::sync::oneshot::Sender<Result<Option<String>, String>>,
        admission: Admission,
    ) {
        let capability = self.registry.capability(id);
        let Some(Capability::Credentials(owner, effects)) = capability.as_deref() else {
            let _ = reply.send(Err("extension credential resource ended".to_owned()));
            return;
        };
        let (owner, effects) = (owner.clone(), effects.clone());
        self.admit(
            Box::pin(async move {
                let result = owner
                    .get_api_key(&provider, include.unwrap_or(true), &*effects)
                    .await
                    .map_err(|error| error.to_string());
                let _ = reply.send(result);
            }),
            admission,
        );
    }
}

impl State {
    /// Native credentials own provider lookup, accepted state and persistence after authorization.
    pub(super) fn start_credential_login(
        &self,
        id: u64,
        (provider, controls): (String, crate::oauth_callbacks::Controls),
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
        admission: Admission,
    ) {
        let capability = self.registry.capability(id);
        let result = match capability.as_deref() {
            Some(Capability::Credentials(owner, _)) => controls
                .callbacks(&self.registry)
                .map(|callbacks| (owner.clone(), callbacks)),
            _ => Err("extension credential resource ended".into()),
        };
        self.admit(
            Box::pin(async move {
                let result = match result {
                    Ok((owner, callbacks)) => owner
                        .login(&provider, callbacks)
                        .await
                        .map_err(|error| error.to_string()),
                    Err(error) => Err(error),
                };
                let _ = reply.send(result);
            }),
            admission,
        );
    }
}

impl State {
    /// Keeps reverse input on the same pending-work owner as native authorization.
    pub(super) fn oauth_interaction(&self, request: crate::oauth_interaction::Request) {
        if let Some((work, admission)) = request.answer() {
            self.admit(work, admission);
        }
    }
}
