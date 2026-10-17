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

/// Disposable exclusively created files owned by one test transaction.
struct Files(std::path::PathBuf);

impl Files {
    /// Create a unique directory without overwriting an existing path.
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "maestro-cloud-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// Write one new fixture file, preserving any pre-existing file.
    fn write(&self, name: &str, bytes: &[u8]) -> String {
        use std::io::Write;
        let path = self.0.join(name);
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
        path.to_str().unwrap().to_owned()
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

/// Retained native content client with a real credential provider.
fn client(
    credentials: google_cloud_auth::credentials::Credentials,
    headers: indexmap::IndexMap<String, String>,
) -> super::super::ApiClient {
    super::super::ApiClient {
        base_url: "https://content.invalid".into(),
        api_version: "v1".into(),
        headers,
        authentication: super::super::api_client::Authentication::Native(super::NodeAuth {
            credentials: tokio::sync::OnceCell::new_with(Some(credentials)),
            ..Default::default()
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

/// File-backed workforce credentials with a controlled endpoint and distinguishable quota.
fn workforce(endpoint: &str, quota: &str) -> serde_json::Value {
    serde_json::json!({"type":"external_account_authorized_user","client_id":"controlled-id","client_secret":"controlled-secret","refresh_token":"controlled-refresh","token_url":endpoint,"quota_project_id":quota})
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
