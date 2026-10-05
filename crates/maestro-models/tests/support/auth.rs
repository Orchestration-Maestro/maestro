#![allow(dead_code)]
use super::conformance::*;
use maestro_models::*;
use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

pub fn local() -> StreamOptions {
    StreamOptions {
        auth: Some(RequestAuth::ConfiguredWithoutSecret { source: None }),
        ..Default::default()
    }
}
pub fn secret(value: &str, source: Option<&str>) -> RequestAuth {
    RequestAuth::Secret {
        secret: SecretString::new(value.into()),
        source: source.map(str::to_owned),
    }
}
pub fn assert_auth(auth: &RequestAuth, value: Option<&str>, label: Option<&str>) {
    match auth {
        RequestAuth::Secret { secret, source } => {
            assert_eq!(Some(secret.expose()), value);
            assert_eq!(source.as_deref(), label);
        }
        RequestAuth::ConfiguredWithoutSecret { source } => {
            assert_eq!(value, None);
            assert_eq!(source.as_deref(), label);
        }
    }
}
pub struct Resolver {
    pub status: AuthStatus,
    pub statuses: AtomicUsize,
    pub calls: Mutex<Vec<(String, Cancellation)>>,
    pub results: Mutex<VecDeque<Result<RequestAuth, Failure>>>,
    pub gate: Option<Gate>,
    pub cancel_on_create: bool,
    pub cancel_on_poll: bool,
}
impl Resolver {
    pub fn new(results: Vec<Result<RequestAuth, Failure>>) -> Self {
        Self {
            status: AuthStatus {
                configured: true,
                source: Some("metadata".into()),
            },
            statuses: AtomicUsize::new(0),
            calls: Mutex::new(Vec::new()),
            results: Mutex::new(results.into()),
            gate: None,
            cancel_on_create: false,
            cancel_on_poll: false,
        }
    }
    pub fn count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}
impl AuthResolver for Resolver {
    fn status(&self, _: &str) -> AuthStatus {
        self.statuses.fetch_add(1, Ordering::SeqCst);
        self.status.clone()
    }
    fn resolve(
        &self,
        provider: String,
        cancellation: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<RequestAuth, Failure>> + Send + '_>> {
        self.calls
            .lock()
            .unwrap()
            .push((provider, cancellation.clone()));
        let result = self
            .results
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected resolution");
        let wait = self.gate.as_ref().map(Gate::wait);
        if self.cancel_on_create {
            cancellation.cancel();
        }
        Box::pin(async move {
            if self.cancel_on_poll {
                cancellation.cancel();
            }
            if let Some(wait) = wait {
                wait.await;
            }
            result
        })
    }
}
pub fn resolving(resolver: Arc<dyn AuthResolver>) -> StreamOptions {
    StreamOptions {
        auth_resolver: Some(resolver),
        ..Default::default()
    }
}

pub struct Adapter {
    pub exchange: Option<Arc<dyn TokenExchange>>,
    pub description: ProviderDescription,
    pub calls: Mutex<Vec<(Model, Context, ProviderOptions)>>,
    pub supports_chat: bool,
    pub cancel_capability: Option<Cancellation>,
    pub cancel_description: Option<Cancellation>,
}
impl Default for Adapter {
    fn default() -> Self {
        Self {
            exchange: None,
            description: ProviderDescription::default(),
            calls: Mutex::new(Vec::new()),
            supports_chat: true,
            cancel_capability: None,
            cancel_description: None,
        }
    }
}
impl Provider for Adapter {
    fn supports(&self, operation: &str) -> bool {
        if let Some(c) = &self.cancel_capability {
            c.cancel();
        }
        self.supports_chat && operation == "chat"
    }
    fn token_exchange(&self) -> Option<Arc<dyn TokenExchange>> {
        self.exchange.clone()
    }
    fn description(&self) -> ProviderDescription {
        if let Some(c) = &self.cancel_description {
            c.cancel();
        }
        self.description.clone()
    }
    fn stream(
        &self,
        model: Model,
        context: Context,
        options: ProviderOptions,
    ) -> Result<Box<dyn ProviderStream>, Failure> {
        self.calls.lock().unwrap().push((model, context, options));
        Ok(Box::new(Response(Some(done()))))
    }
}
struct Response(Option<ProviderUpdate>);
impl ProviderStream for Response {
    fn next(&mut self) -> Pin<Box<dyn Future<Output = Option<ProviderUpdate>> + Send + '_>> {
        Box::pin(async { self.0.take() })
    }
}
pub fn headers(values: &[(&str, &str)]) -> std::collections::BTreeMap<String, String> {
    values
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

pub struct Exchange {
    pub calls: AtomicUsize,
    pub gate: Option<Gate>,
    pub failure: bool,
}
impl TokenExchange for Exchange {
    fn exchange(
        &self,
        state: SecretString,
        cancellation: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<TokenExchangeResult, Failure>> + Send + '_>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let wait = self.gate.as_ref().map(Gate::wait);
        Box::pin(async move {
            assert_eq!(state.expose(), "STATE_SENTINEL");
            if let Some(mut wait) = wait {
                let mut cancelled = Box::pin(cancellation.cancelled());
                std::future::poll_fn(|cx| {
                    if cancelled.as_mut().poll(cx).is_ready() {
                        return std::task::Poll::Ready(Err(Failure::Cancelled));
                    }
                    wait.as_mut().poll(cx).map(|_| Ok(()))
                })
                .await?;
            }
            if cancellation.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            if self.failure {
                return Err(Failure::AuthenticationFailed);
            }
            Ok(TokenExchangeResult {
                auth: secret("ROTATED_AUTH", Some("exchange")),
                state: SecretString::new("ROTATED_STATE".into()),
                expires_at: Some(123),
            })
        })
    }
}
