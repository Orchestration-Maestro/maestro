//! Current-format credential policy and detached metadata.
use crate::{Credential, CredentialError, CredentialOptions, CredentialStorage, worker::Work};
use maestro_models::{AuthResolver, AuthStatus, Cancellation, Failure, RequestAuth, SecretString};
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

/// Credential policy owner; synchronous construction/updates, asynchronous requests.
/// Metadata is cached presence, not proof of live validity. Secrets are never listed.
pub struct Credentials {
    storage: Arc<dyn CredentialStorage>,
    options: CredentialOptions,
    snapshot: Mutex<BTreeSet<String>>,
    mutation: Mutex<()>,
    runtime: Mutex<BTreeMap<String, RequestAuth>>,
}
fn parse(bytes: Option<SecretString>) -> Result<Map<String, Value>, CredentialError> {
    match bytes {
        None => Ok(Map::new()),
        Some(bytes) => serde_json::from_str::<Value>(bytes.expose())
            .map_err(|_| CredentialError::Malformed)?
            .as_object()
            .cloned()
            .ok_or(CredentialError::Malformed),
    }
}
fn read(
    storage: &dyn CredentialStorage,
    cancellation: &Cancellation,
) -> Result<Map<String, Value>, CredentialError> {
    let mut result = None;
    storage.transact(cancellation, &mut |bytes| {
        result = Some(parse(bytes)?);
        Ok(None)
    })?;
    result.ok_or(CredentialError::Storage)
}
fn failure(error: CredentialError) -> Failure {
    if error == CredentialError::Cancelled {
        Failure::Cancelled
    } else {
        Failure::AuthenticationFailed
    }
}
impl Credentials {
    /// Read the initial current-format store without helpers or fallback work.
    pub fn new(
        storage: Arc<dyn CredentialStorage>,
        options: CredentialOptions,
    ) -> Result<Self, CredentialError> {
        let data = read(storage.as_ref(), &Cancellation::new())?;
        Ok(Self {
            storage,
            options,
            snapshot: Mutex::new(data.keys().cloned().collect()),
            mutation: Mutex::new(()),
            runtime: Mutex::new(BTreeMap::new()),
        })
    }
    /// Synchronously reload valid bytes; failure preserves the last metadata snapshot.
    pub fn reload(&self, cancellation: &Cancellation) -> Result<(), CredentialError> {
        let _mutation = self.mutation.lock().map_err(|_| CredentialError::Storage)?;
        let data = read(self.storage.as_ref(), cancellation)?;
        *self.snapshot.lock().map_err(|_| CredentialError::Storage)? =
            data.keys().cloned().collect();
        Ok(())
    }
    /// Remove only local stored data, not remote grants, runtime input or ambient values.
    pub fn remove(
        &self,
        provider: &str,
        cancellation: &Cancellation,
    ) -> Result<(), CredentialError> {
        let _mutation = self.mutation.lock().map_err(|_| CredentialError::Storage)?;
        let mut next = None;
        self.storage.transact(cancellation, &mut |bytes| {
            let mut data = parse(bytes)?;
            data.remove(provider);
            next = Some(data.keys().cloned().collect());
            Ok(Some(SecretString::new(Value::Object(data).to_string())))
        })?;
        *self.snapshot.lock().map_err(|_| CredentialError::Storage)? =
            next.ok_or(CredentialError::Storage)?;
        Ok(())
    }
    /// Return detached stored-provider names only; never resolve or expose credentials.
    pub fn list(&self) -> Vec<String> {
        self.snapshot
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .cloned()
            .collect()
    }
    /// Set/clear already-resolved nonpersistent input; present invalid auth never falls through.
    pub fn set_runtime_auth(&self, provider: String, auth: Option<RequestAuth>) {
        let mut runtime = self.runtime.lock().unwrap_or_else(|p| p.into_inner());
        match auth {
            Some(auth) => {
                runtime.insert(provider, auth);
            }
            None => {
                runtime.remove(&provider);
            }
        }
    }
    /// Re-read/change only the selected provider under serialized storage ownership.
    /// Publish metadata only after successful persistence; malformed roots are preserved.
    pub fn set(
        &self,
        provider: &str,
        credential: Credential,
        cancellation: &Cancellation,
    ) -> Result<(), CredentialError> {
        let _mutation = self.mutation.lock().map_err(|_| CredentialError::Storage)?;
        let value = match credential {
            Credential::ApiKey { value } => json!({"type":"api_key", "key":value.expose()}),
            Credential::Refreshable(token) => {
                let auth = match token.auth {
                    RequestAuth::Secret { secret, .. } => json!({"secret":secret.expose()}),
                    RequestAuth::ConfiguredWithoutSecret { .. } => json!({"without_secret":true}),
                };
                json!({"type":"refreshable", "auth":auth, "state":token.state.expose(), "expires_at":token.expires_at})
            }
        };
        let mut next = None;
        self.storage.transact(cancellation, &mut |bytes| {
            let mut data = parse(bytes)?;
            data.insert(provider.into(), value.clone());
            let bytes = SecretString::new(Value::Object(data.clone()).to_string());
            next = Some(data.keys().cloned().collect());
            Ok(Some(bytes))
        })?;
        *self.snapshot.lock().map_err(|_| CredentialError::Storage)? =
            next.ok_or(CredentialError::Storage)?;
        Ok(())
    }
}
impl AuthResolver for Credentials {
    fn status(&self, provider: &str) -> AuthStatus {
        if self
            .runtime
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains_key(provider)
        {
            return AuthStatus {
                configured: true,
                source: Some("runtime".into()),
            };
        }
        let configured = self
            .snapshot
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(provider);
        if configured {
            return AuthStatus {
                configured: true,
                source: Some("stored".into()),
            };
        }
        if self
            .options
            .environment_names
            .get(provider)
            .is_some_and(|names| !names.is_empty())
        {
            return AuthStatus {
                configured: false,
                source: Some("environment".into()),
            };
        }
        if let Some(fallback) = &self.options.fallback {
            return AuthStatus {
                configured: fallback.status(provider).configured,
                source: Some("fallback".into()),
            };
        }
        AuthStatus {
            configured: false,
            source: None,
        }
    }
    fn resolve(
        &self,
        provider: String,
        cancellation: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<RequestAuth, Failure>> + Send + '_>> {
        Box::pin(async move {
            if cancellation.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            let runtime = self
                .runtime
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .get(&provider)
                .cloned();
            if let Some(auth) = runtime {
                return labeled(auth, "runtime");
            }
            let storage = self.storage.clone();
            let signal = cancellation.clone();
            let data = Work::start(move || read(storage.as_ref(), &signal))
                .map_err(failure)?
                .wait(cancellation.clone())
                .await
                .map_err(failure)?;
            let Some(record) = data.get(&provider) else {
                if let Some(names) = self.options.environment_names.get(&provider) {
                    for name in names {
                        if cancellation.is_cancelled() {
                            return Err(Failure::Cancelled);
                        }
                        if let Some(value) = self
                            .options
                            .secrets
                            .environment(name)
                            .filter(|s| !s.expose().is_empty())
                        {
                            return labeled(
                                RequestAuth::Secret {
                                    secret: value,
                                    source: None,
                                },
                                "environment",
                            );
                        }
                    }
                }
                if let Some(fallback) = &self.options.fallback {
                    let result = crate::worker::cancellable(&cancellation, async {
                        Ok(fallback.resolve(provider, cancellation.clone()).await)
                    })
                    .await
                    .map_err(failure)?;
                    return labeled(
                        result.map_err(|error| match error {
                            Failure::MissingAuthentication | Failure::Cancelled => error,
                            _ => Failure::AuthenticationFailed,
                        })?,
                        "fallback",
                    );
                }
                return Err(Failure::MissingAuthentication);
            };
            if record.get("type").and_then(Value::as_str) == Some("refreshable") {
                let auth = record
                    .get("auth")
                    .and_then(Value::as_object)
                    .ok_or(Failure::AuthenticationFailed)?;
                let request = match (auth.get("secret"), auth.get("without_secret")) {
                    (Some(Value::String(value)), None) => RequestAuth::Secret {
                        secret: SecretString::new(value.clone()),
                        source: None,
                    },
                    (None, Some(Value::Bool(true))) => {
                        RequestAuth::ConfiguredWithoutSecret { source: None }
                    }
                    _ => return Err(Failure::AuthenticationFailed),
                };
                record
                    .get("state")
                    .and_then(Value::as_str)
                    .ok_or(Failure::AuthenticationFailed)?;
                let expiry = match record.get("expires_at") {
                    Some(Value::Null) => None,
                    Some(value) => Some(value.as_u64().ok_or(Failure::AuthenticationFailed)?),
                    None => return Err(Failure::AuthenticationFailed),
                };
                if !expiry.is_some_and(|expiry| (self.options.now)() < expiry) {
                    return Err(Failure::MissingAuthentication);
                }
                return labeled(request, "stored");
            }
            if record.get("type").and_then(Value::as_str) != Some("api_key") {
                return Err(Failure::AuthenticationFailed);
            }
            let value = record
                .get("key")
                .and_then(Value::as_str)
                .ok_or(Failure::AuthenticationFailed)?;
            let value = crate::worker::cancellable(
                &cancellation,
                self.options
                    .secrets
                    .resolve(SecretString::new(value.into()), cancellation.clone()),
            )
            .await
            .map_err(failure)?
            .filter(|s| !s.expose().is_empty())
            .ok_or(Failure::MissingAuthentication)?;
            Ok(RequestAuth::Secret {
                secret: value,
                source: Some("stored".into()),
            })
        })
    }
}
fn labeled(auth: RequestAuth, label: &str) -> Result<RequestAuth, Failure> {
    match auth {
        RequestAuth::Secret { secret, .. } if secret.expose().is_empty() => {
            Err(Failure::MissingAuthentication)
        }
        RequestAuth::Secret { secret, .. } => Ok(RequestAuth::Secret {
            secret,
            source: Some(label.into()),
        }),
        RequestAuth::ConfiguredWithoutSecret { .. } => Ok(RequestAuth::ConfiguredWithoutSecret {
            source: Some(label.into()),
        }),
    }
}
