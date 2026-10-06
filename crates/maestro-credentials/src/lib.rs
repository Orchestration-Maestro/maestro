//! Private provider credential ownership behind replaceable adapters.
//! Stored data is sensitive plaintext, not a vault. Metadata never resolves secrets.
//! Constructors and changes read synchronously; selected requests delegate blocking work.
//! See the feature page for absence/failure, helper effects, ownership and current format.
//!
//! A synthetic memory store supplies one selected controlled model request:
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
//! use std::sync::RwLock;
//! let descriptor = Model {
//!     id: "synthetic".into(), name: "Synthetic".into(), api: "synthetic".into(),
//!     provider: "local".into(), base_url: String::new(), reasoning: false,
//!     thinking_level_map: None, input: vec!["text".into()],
//!     cost: TokenRates { input: 0.0, output: 0.0, cache_read: 0.0, cache_write: 0.0 },
//!     context_window: 0.0, max_tokens: 0.0, headers: None, compat: None,
//! };
//! fn produce(model: Model) -> Result<AssistantMessageEventStream, ThrownValue> {
//!     let message = Arc::new(RwLock::new(AssistantMessage {
//!         content: vec![], api: model.api, provider: model.provider, model: model.id,
//!         response_model: None, response_id: None, diagnostics: None,
//!         usage: Usage { input: 0.0, output: 0.0, cache_read: 0.0, cache_write: 0.0,
//!             total_tokens: 0.0, cost: UsageCost { input: 0.0, output: 0.0,
//!                 cache_read: 0.0, cache_write: 0.0, total: 0.0 } },
//!         stop_reason: StopReason::Stop, error_message: None, timestamp: 0.0,
//!     }));
//!     let stream = create_assistant_message_event_stream();
//!     stream.push(AssistantMessageEvent::Start { partial: message.clone() })?;
//!     stream.push(AssistantMessageEvent::Done { reason: StopReason::Stop, message })?;
//!     Ok(stream)
//! }
//! // Resolve the credential owner before calling the model adapter.
//! let resolved = block_on(credentials.resolve("synthetic".into(), Cancellation::new()))?;
//! let RequestAuth::Secret { secret, source } = resolved else { panic!() };
//! assert_eq!(source.as_deref(), Some("stored"));
//! let provider = ApiProvider {
//!     api: descriptor.api.clone(),
//!     stream: Arc::new(|model, _, options| {
//!         assert_eq!(options.unwrap().base.api_key.as_deref(), Some("SYNTHETIC_KEY"));
//!         produce(model)
//!     }),
//!     stream_simple: Arc::new(|model, _, _| produce(model)),
//! };
//! let stream = (provider.stream)(descriptor,
//!     Context { system_prompt: None, messages: vec![], tools: None },
//!     Some(ProviderStreamOptions { base: StreamOptions {
//!         api_key: Some(secret.expose().into()), ..Default::default()
//!     }, ..Default::default() })).unwrap();
//! let result = block_on(stream.result());
//! assert_eq!(result.read().unwrap().stop_reason, StopReason::Stop);
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
