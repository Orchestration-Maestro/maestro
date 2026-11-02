//! Retained catalog and credential requests use their original local owners.
use super::{Admission, Capability, State};
impl State {
    /// Selects and retains the original catalog while its provider-key operation settles.
    pub(super) fn start_provider_key(
        &self,
        id: u64,
        provider: String,
        reply: tokio::sync::oneshot::Sender<Result<Option<String>, String>>,
        admission: Admission,
    ) {
        let capability = self.registry.capability(id);
        let Some(Capability::Catalog(catalog)) = capability.as_deref() else {
            let _ = reply.send(Err("extension catalog resource ended".to_owned()));
            return;
        };
        let catalog = catalog.clone();
        self.admit(
            Box::pin(async move {
                let result = catalog
                    .get_api_key_for_provider(&provider)
                    .await
                    .map_err(|error| error.to_string());
                let _ = reply.send(result);
            }),
            admission,
        );
    }
}

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
    /// Dispatches a credential request outside the capability-table borrow.
    pub(super) fn credentials(&self, id: u64, request: crate::credential_reads::Request) {
        let capability = self.registry.capability(id);
        if let Some(Capability::Credentials(owner, _)) = capability.as_deref() {
            request.answer(owner, &self.registry);
        } else {
            request.reject("extension credential resource ended");
        }
    }
    /// Dispatches a catalog request outside the capability-table borrow.
    pub(super) fn catalog(&self, id: u64, request: crate::catalog_reads::Request) {
        let capability = self.registry.capability(id);
        if let Some(Capability::Catalog(catalog)) = capability.as_deref() {
            request.answer(catalog, &self.registry);
        } else {
            request.reject("extension catalog resource ended");
        }
    }
}

impl State {
    /// Reads one original retained tree node outside the registry borrow.
    pub(super) fn node(&self, id: u64, request: crate::tree_reads::Request) {
        let capability = self.registry.capability(id);
        if let Some(Capability::Node(node)) = capability.as_deref() {
            node.answer(request, &self.registry);
        } else {
            request.reject("extension tree node resource ended");
        }
    }
}

impl State {
    /// Starts an OAuth login or refresh, or serves an OAuth provider or interaction request.
    pub(super) fn oauth(&self, message: super::Message) {
        match message {
            super::Message::OAuthLogin(id, controls, reply, admission) => {
                self.start_oauth_login(id, controls, reply, admission);
            }
            super::Message::OAuthRefresh(id, data, reply, admission) => {
                self.start_oauth_refresh(id, data, reply, admission);
            }
            super::Message::OAuthProvider(id, request) => self.oauth_provider(id, request),
            super::Message::OAuthInteraction(request) => self.oauth_interaction(request),
            _ => {}
        }
    }
    /// Calls an OAuth implementation outside the capability-table borrow.
    pub(super) fn oauth_provider(&self, id: u64, request: crate::oauth_reads::Request) {
        let capability = self.registry.capability(id);
        if let Some(Capability::OAuthProvider(provider)) = capability.as_deref() {
            request.answer(provider);
        } else {
            request.reject();
        }
    }
}

impl State {
    /// Keeps the selected implementation through refresh completion without another registry lookup.
    pub(super) fn start_oauth_refresh(
        &self,
        id: u64,
        data: String,
        reply: tokio::sync::oneshot::Sender<Result<String, String>>,
        admission: Admission,
    ) {
        let capability = self.registry.capability(id);
        let Some(Capability::OAuthProvider(provider)) = capability.as_deref() else {
            let _ = reply.send(Err("extension OAuth provider resource ended".into()));
            return;
        };
        let provider = provider.clone();
        self.admit(
            Box::pin(async move {
                let result = async {
                    let credentials = crate::writer_reads::decode(&data)?;
                    let refreshed = provider
                        .refresh_token(credentials, None)
                        .await
                        .map_err(|error| error.to_string())?;
                    serde_json::to_string(&refreshed).map_err(|error| error.to_string())
                }
                .await;
                let _ = reply.send(result);
            }),
            admission,
        );
    }
}

impl State {
    /// Leaves token exchange and interaction order with the selected native implementation.
    pub(super) fn start_oauth_login(
        &self,
        id: u64,
        controls: crate::oauth_callbacks::Controls,
        reply: tokio::sync::oneshot::Sender<Result<String, String>>,
        admission: Admission,
    ) {
        let capability = self.registry.capability(id);
        let result = match capability.as_deref() {
            Some(Capability::OAuthProvider(provider)) => controls
                .callbacks(&self.registry)
                .map(|callbacks| (provider.clone(), callbacks)),
            _ => Err("extension OAuth provider resource ended".into()),
        };
        self.admit(
            Box::pin(async move {
                let result = match result {
                    Ok((provider, callbacks)) => provider
                        .login(callbacks, None)
                        .await
                        .map_err(|error| error.to_string())
                        .and_then(|credentials| {
                            serde_json::to_string(&credentials).map_err(|error| error.to_string())
                        }),
                    Err(error) => Err(error),
                };
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
