//! A controlled endpoint used by the original native device-account refresh algorithm.
use maestro_models::{
    BoxFuture, Fetch, Model, OAuthCallbacks, OAuthCredentials, OAuthError, OAuthProviderInterface,
};
use std::{
    borrow::Cow,
    io::{Read, Write},
    net::TcpListener,
};

/// Native provider delegates operations to the existing device-account implementation.
pub(super) struct Provider(pub(super) String);
impl OAuthProviderInterface for Provider {
    fn id(&self) -> &'static str {
        "component-refresh"
    }
    fn name(&self) -> &'static str {
        "Controlled native refresh"
    }
    fn login(
        &self,
        callbacks: OAuthCallbacks,
        _fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        maestro_models::GITHUB_COPILOT_OAUTH_PROVIDER.login(callbacks, None)
    }
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
        _fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        assert_eq!(
            serde_json::to_value(&credentials).unwrap(),
            serde_json::json!({
                "refresh":"literal-refresh", "access":"old-access", "expires":42.5,
                "enterpriseUrl":"company.ghe.com", "extensionData":{"nested":[null,false,-0.0]}
            })
        );
        assert!(
            credentials.extra["extensionData"]["nested"][2]
                .as_f64()
                .unwrap()
                .is_sign_negative()
        );
        let url = self.0.clone();
        let fetch: Fetch = std::sync::Arc::new(move |mut request| {
            assert_eq!(
                request.url,
                "https://api.company.ghe.com/copilot_internal/v2/token"
            );
            assert_eq!(request.headers["authorization"], "Bearer literal-refresh");
            request.url.clone_from(&url);
            maestro_models::default_fetch()(request)
        });
        Box::pin(async move {
            let extra = credentials.extra["extensionData"].clone();
            let mut result = maestro_models::GITHUB_COPILOT_OAUTH_PROVIDER
                .refresh_token(credentials, Some(fetch))
                .await?;
            result.extra.insert("extensionData".into(), extra);
            Ok(result)
        })
    }
    fn get_api_key<'a>(
        &self,
        credentials: &'a OAuthCredentials,
    ) -> Result<Cow<'a, str>, OAuthError> {
        maestro_models::GITHUB_COPILOT_OAUTH_PROVIDER.get_api_key(credentials)
    }
    fn modify_models<'m>(
        &self,
        models: &'m [Model],
        credentials: &OAuthCredentials,
    ) -> Option<Result<Vec<Cow<'m, Model>>, OAuthError>> {
        maestro_models::GITHUB_COPILOT_OAUTH_PROVIDER.modify_models(models, credentials)
    }
}

/// One finite peer serving a synthetic token through the production HTTP client.
pub(super) struct Server {
    /// Local endpoint selected by the test transport.
    pub(super) url: String,
    /// Joins the actual HTTP response rather than waiting on a timer.
    thread: std::thread::JoinHandle<()>,
}
impl Server {
    /// Accepts exactly one request with no service credentials.
    pub(super) fn new() -> Self {
        let mut server = Self::serve(1, |headers, body| {
            assert!(headers.starts_with("GET /token HTTP/1.1\r\n"));
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("authorization: bearer literal-refresh\r\n")
            );
            assert!(body.is_empty());
            r#"{"token":"proxy-ep=proxy.controlled.example;native-token","expires_at":4102444800}"#
                .into()
        });
        server.url.push_str("/token");
        server
    }
    /// Serves the finite requests made by the original native authorization algorithms.
    pub(super) fn serve(
        count: usize,
        response: impl Fn(&str, &[u8]) -> String + Send + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let thread = std::thread::spawn(move || {
            for _ in 0..count {
                let (mut peer, _) = listener.accept().unwrap();
                let (headers, request) = read_request(&mut peer);
                let body = response(&headers, &request);
                write!(peer, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                peer.flush().unwrap();
            }
        });
        Self { url, thread }
    }
    /// Observes the completed token response.
    pub(super) fn finish(self) {
        self.thread.join().unwrap();
    }
}

/// Reads the actual content-length-framed request, including authorization-code bodies.
fn read_request(peer: &mut std::net::TcpStream) -> (String, Vec<u8>) {
    let mut request = Vec::new();
    let mut byte = [0];
    while !request.ends_with(b"\r\n\r\n") {
        peer.read_exact(&mut byte).unwrap();
        request.push(byte[0]);
    }
    let headers = String::from_utf8(request).unwrap();
    let length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().unwrap())
        })
        .unwrap_or(0);
    let mut body = vec![0; length];
    peer.read_exact(&mut body).unwrap();
    (headers, body)
}
