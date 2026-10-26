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
