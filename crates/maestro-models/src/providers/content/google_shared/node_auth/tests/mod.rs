//! Authentication witnesses through content preparation.

mod adc;
mod certificates;
mod headers;
mod metadata;
mod refresh;

/// A complete controlled JSON response at the production HTTP boundary.
fn response(value: &serde_json::Value) -> crate::HttpResponse {
    let bytes = serde_json::to_vec(value).unwrap();
    crate::HttpResponse {
        status: 200,
        status_text: String::new(),
        headers: std::collections::BTreeMap::new(),
        body: Box::pin(futures_util::stream::once(std::future::ready(Ok(bytes)))),
    }
}

/// The controlled client key as PEM text.
fn client_key_pem() -> String {
    use base64::Engine as _;
    let der =
        include_bytes!("../../../../../../tests/fixtures/cloud-authentication/client-key.der");
    format!(
        "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
        base64::engine::general_purpose::STANDARD.encode(der)
    )
}

use crate::tests::content_credentials::{Files, workforce};

/// Retained native content client with a real credential provider.
fn client(
    credentials: google_cloud_auth::credentials::Credentials,
    headers: indexmap::IndexMap<String, String>,
) -> super::super::ApiClient {
    super::super::ApiClient {
        base_url: "https://content.invalid".into(),
        resource: super::super::api_client::Resource::default(),
        api_version: "v1".into(),
        headers,
        authentication: super::super::api_client::Authentication::Native(super::NodeAuth {
            credentials: tokio::sync::OnceCell::new_with(Some(credentials)),
        }),
    }
}

/// Prepare a minimal projected content request through its public operation.
async fn prepare(client: &super::super::ApiClient) -> super::super::PreparedContentRequest {
    let projected = super::super::project_content_request(
        serde_json::json!({"model":"model","contents":"hello"}),
    )
    .unwrap();
    client.prepare_request(projected, None).await.unwrap()
}

/// Drive a focused native operation without mutating process environment.
fn run<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

/// Fetch recording each requested URL and answering through `reply`.
fn recording(
    reply: impl Fn(&str) -> Result<crate::HttpResponse, crate::FetchError> + Send + Sync + 'static,
) -> (crate::Fetch, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    let urls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = urls.clone();
    let fetch: crate::Fetch = std::sync::Arc::new(move |request| {
        seen.lock().unwrap().push(request.url.clone());
        Box::pin(std::future::ready(reply(&request.url)))
    });
    (fetch, urls)
}

/// A successful metadata response naming the expected flavor.
fn google_response() -> crate::HttpResponse {
    let mut response = response(&serde_json::json!({}));
    response
        .headers
        .insert("metadata-flavor".into(), "Google".into());
    response
}

/// Drive a paused-clock runtime so retry backoff never sleeps for real.
fn run_paused<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap()
        .block_on(future)
}
