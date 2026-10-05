//! Private provider credential ownership behind replaceable adapters.
//! Stored data is sensitive plaintext, not a vault. Metadata never resolves secrets.
//! Constructors and changes read synchronously; selected requests delegate blocking work.
//! See the feature page for absence/failure, helper effects, ownership and current format.
//!
//! A synthetic memory store supplies one selected scripted model request:
//! ```
//! use maestro_credentials::*;
//! use maestro_models::*;
//! use std::{future::Future, pin::Pin, sync::Arc};
//! struct Literal;
//! impl SecretResolver for Literal {
//!     fn environment(&self, _: &str) -> Option<SecretString> { None }
//!     fn resolve(&self, value: SecretString, _: Cancellation)
//!         -> Pin<Box<dyn Future<Output = Result<Option<SecretString>, CredentialError>> + Send + '_>>
//!     { Box::pin(async move { Ok(Some(value)) }) }
//! }
//! let credentials = Arc::new(Credentials::new(
//!     Arc::new(MemoryCredentialStorage::new(None)),
//!     CredentialOptions { environment_names: Default::default(), fallback: None,
//!         secrets: Arc::new(Literal), now: Arc::new(|| 100) },
//! )?);
//! credentials.set("synthetic", Credential::ApiKey {
//!     value: SecretString::new("SYNTHETIC_KEY".into()),
//! }, &Cancellation::new())?;
//! let model = Model { identity: ModelIdentity { provider: "synthetic".into(),
//!     model: "example".into(), operation: "chat".into() }, protocol: "script".into(),
//!     rates: None, capabilities: Default::default(), input: vec!["text".into()],
//!     headers: Default::default() };
//! let adapter = Arc::new(ScriptedProvider::new(vec![Script::Steps(vec![
//!     ScriptStep::Update(ProviderUpdate::Done { reason: StopReason::Stop }),
//! ])]));
//! let mut models = Models::new(Arc::new(|| 100));
//! models.register(model.clone(), adapter.clone())?;
//! let result = block_on(models.complete(model,
//!     Context { system_prompt: None, messages: vec![], tools: vec![] },
//!     StreamOptions { auth_resolver: Some(credentials), ..Default::default() }));
//! assert_eq!(result.failure, None);
//! assert_eq!(adapter.calls().len(), 1);
//! # fn block_on<F: Future>(future: F) -> F::Output {
//! #     struct Notify(std::thread::Thread);
//! #     impl std::task::Wake for Notify {
//! #         fn wake(self: Arc<Self>) { self.0.unpark(); }
//! #         fn wake_by_ref(self: &Arc<Self>) { self.0.unpark(); }
//! #     }
//! #     let waker = std::task::Waker::from(Arc::new(Notify(std::thread::current())));
//! #     let mut cx = std::task::Context::from_waker(&waker);
//! #     let mut future = std::pin::pin!(future);
//! #     loop { match future.as_mut().poll(&mut cx) {
//! #         std::task::Poll::Ready(value) => return value,
//! #         std::task::Poll::Pending => std::thread::park(),
//! #     }}
//! # }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![doc = include_str!("../../../docs/credentials.md")]

mod credentials;
mod file;
mod secrets;
mod storage;
mod types;
mod worker;

pub use credentials::Credentials;
pub use file::FileCredentialStorage;
pub use secrets::{NativeSecretResolver, SecretResolver, reset_secret_helper_cache};
pub use storage::{
    CredentialStorage, CredentialTransaction, MemoryCredentialStorage, ReadOnlyCredentialStorage,
};
pub use types::{Credential, CredentialError, CredentialOptions};
