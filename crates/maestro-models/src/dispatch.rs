//! Owned lazy authentication setup before the adapter source exists.
use crate::{
    Context, Failure, Model, Provider, ProviderOptions, ProviderStream, ProviderUpdate,
    StreamOptions,
};
use std::{future::Future, pin::Pin, sync::Arc};

type Setup = Pin<Box<dyn Future<Output = Result<Box<dyn ProviderStream>, Failure>> + Send>>;
struct Dispatch {
    setup: Option<Setup>,
    source: Option<Box<dyn ProviderStream>>,
}
pub(crate) fn source(
    provider: Arc<dyn Provider>,
    model: Model,
    context: Context,
    options: StreamOptions,
) -> Result<Box<dyn ProviderStream>, Failure> {
    if options.cancellation.is_cancelled() {
        return Err(Failure::Cancelled);
    }
    if let Some(auth) = options.auth {
        return provider.stream(
            model,
            context,
            ProviderOptions {
                cancellation: options.cancellation,
                auth: crate::auth::validate(auth)?,
                headers: options.headers,
            },
        );
    }
    let resolver = options
        .auth_resolver
        .ok_or(Failure::MissingAuthentication)?;
    Ok(Box::new(Dispatch {
        source: None,
        setup: Some(Box::pin(async move {
            if options.cancellation.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            let resolution = resolver.resolve(
                model.identity.provider.clone(),
                options.cancellation.clone(),
            );
            if options.cancellation.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            let result = resolution.await;
            if options.cancellation.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            let auth = crate::auth::validate(result.map_err(|failure| match failure {
                Failure::MissingAuthentication | Failure::Cancelled => failure,
                _ => Failure::AuthenticationFailed,
            })?)?;
            if options.cancellation.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            let result = provider.stream(
                model,
                context,
                ProviderOptions {
                    cancellation: options.cancellation.clone(),
                    auth,
                    headers: options.headers,
                },
            );
            if options.cancellation.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            result
        })),
    }))
}
impl ProviderStream for Dispatch {
    fn next(&mut self) -> Pin<Box<dyn Future<Output = Option<ProviderUpdate>> + Send + '_>> {
        Box::pin(async move {
            if let Some(setup) = self.setup.as_mut() {
                let result = setup.await;
                self.setup = None;
                match result {
                    Ok(source) => self.source = Some(source),
                    Err(failure) => return Some(ProviderUpdate::Error { failure }),
                }
            }
            match self.source.as_mut() {
                Some(source) => source.next().await,
                None => None,
            }
        })
    }
}

pub(crate) fn headers(
    layers: &[&std::collections::BTreeMap<String, String>],
) -> Result<std::collections::BTreeMap<String, String>, Failure> {
    let mut effective = std::collections::BTreeMap::new();
    for layer in layers {
        let mut names = std::collections::BTreeSet::new();
        for (name, value) in *layer {
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
                || value.bytes().any(|b| (b < 32 && b != b'\t') || b == 127)
            {
                return Err(Failure::InvalidRequestHeaders);
            }
            let name = name.to_ascii_lowercase();
            if !names.insert(name.clone()) {
                return Err(Failure::InvalidRequestHeaders);
            }
            effective.insert(name, value.clone());
        }
    }
    Ok(effective)
}
