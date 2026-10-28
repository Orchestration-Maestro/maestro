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

/// The variable naming the platform configuration home, set to `home`: Windows reads
/// `%APPDATA%`, where other hosts read `$HOME/.config`.
fn home_variable(home: &str) -> (&'static str, String) {
    if cfg!(windows) {
        ("APPDATA", maestro_path::join(&[home, ".config"]))
    } else {
        ("HOME", home.to_owned())
    }
}

/// `variables` as a child can hold them: Windows names ignore case, so there only the first
/// nonempty spelling of each name is set.
fn settable<'a>(variables: &[(&'a str, &'a str)]) -> Vec<(&'a str, &'a str)> {
    let mut kept: Vec<(&str, &str)> = Vec::new();
    for &(name, value) in variables {
        match kept
            .iter_mut()
            .find(|(held, _)| cfg!(windows) && held.eq_ignore_ascii_case(name))
        {
            Some(held) if held.1.is_empty() => *held = (name, value),
            Some(_) => {}
            None => kept.push((name, value)),
        }
    }
    kept
}

use crate::tests::content_credentials::{Files, workforce};
use crate::tests::runtime::{run, run_paused};

impl Files {
    /// Write one new fixture file, preserving any pre-existing file.
    fn write(&self, name: &str, bytes: &[u8]) -> String {
        use std::io::Write;
        let path = self.path(name);
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
        path
    }
}

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
    google_response_with(&serde_json::json!({}))
}

/// A successful metadata response carrying `value` and naming the expected flavor.
fn google_response_with(value: &serde_json::Value) -> crate::HttpResponse {
    let mut response = response(value);
    response
        .headers
        .insert("metadata-flavor".into(), "Google".into());
    response
}
