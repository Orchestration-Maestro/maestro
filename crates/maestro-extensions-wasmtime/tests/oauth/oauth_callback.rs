//! Built-in authorization runs through the guest using only controlled HTTP endpoints.
use maestro_models::{
    BoxFuture, Fetch, OAuthCallbacks, OAuthCredentials, OAuthError, OAuthProviderHandle,
    OAuthProviderInterface,
};
use serde_json::{Value, json};
use std::{
    borrow::Cow,
    sync::{Arc, Mutex},
};

/// Selects only transport; every operation still belongs to the original native provider.
struct Controlled {
    /// Original built-in implementation, retained before its registry replacement.
    provider: OAuthProviderHandle,
    /// Real HTTP client redirected to the disposable local peer.
    fetch: Fetch,
}
impl OAuthProviderInterface for Controlled {
    fn id(&self) -> &str {
        self.provider.id()
    }
    fn name(&self) -> &str {
        self.provider.name()
    }
    fn uses_callback_server(&self) -> Option<bool> {
        self.provider.uses_callback_server()
    }
    fn login(
        &self,
        callbacks: OAuthCallbacks,
        _: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        self.provider.login(callbacks, Some(self.fetch.clone()))
    }
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
        _: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        self.provider
            .refresh_token(credentials, Some(self.fetch.clone()))
    }
    fn get_api_key<'a>(
        &self,
        credentials: &'a OAuthCredentials,
    ) -> Result<Cow<'a, str>, OAuthError> {
        self.provider.get_api_key(credentials)
    }
    fn modify_models<'m>(
        &self,
        models: &'m [maestro_models::Model],
        credentials: &OAuthCredentials,
    ) -> Option<Result<Vec<Cow<'m, maestro_models::Model>>, OAuthError>> {
        self.provider.modify_models(models, credentials)
    }
}

#[test]
fn oauth_builtin_logins_use_controlled_endpoints_and_original_algorithms() {
    if !maestro_test_process::selection().is_child() {
        let output = maestro_test_process::TestProcess::current(
            "oauth_builtin_logins_use_controlled_endpoints_and_original_algorithms",
        )
        .unwrap()
        .case("builtin")
        .env("MAESTRO_OAUTH_CALLBACK_HOST", "127.0.0.1")
        .output()
        .unwrap();
        assert!(output.status.success(), "{output:?}");
        return;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        for id in ["github-copilot", "anthropic", "openai-codex"] {
            builtin(id).await;
        }
    });
}

/// Invokes a compiled guest against one native algorithm, then restores registry membership.
async fn builtin(id: &'static str) {
    let original = maestro_models::get_oauth_provider(id).unwrap();
    let policies = Arc::new(Mutex::new(Vec::new()));
    let observed = policies.clone();
    let count = if id == "github-copilot" {
        3 + maestro_models::get_models(id).len()
    } else {
        1
    };
    let server = super::refresh::Server::serve(count, move |headers, body| {
        response(id, headers, body, &observed)
    });
    maestro_models::register_oauth_provider(Arc::new(Controlled {
        provider: original.clone(),
        fetch: transport(id, server.url.clone()),
    }));
    let bus = super::super::components::TracedBus::new();
    let (_owner, runner, _) =
        super::super::retained_components::loaded_on(bus.controller.for_extension()).await;
    let result = super::call(&runner, &format!("builtin {id}")).await;
    assert_eq!(result["id"], id);
    assert_eq!(result["name"], original.name());
    assert_eq!(result["server"], json!(original.uses_callback_server()));
    assert_eq!(result["key"], result["credentials"]["access"]);
    let expires = result["credentials"]["expires"].as_f64().unwrap();
    assert!(expires.is_finite());
    assert!(expires > 0.0);
    if id == "github-copilot" {
        device(&result, &policies);
    } else {
        subscription(&result, id);
    }
    server.finish();
    maestro_models::register_oauth_provider(original);
}

/// Rejects any unrecognized service origin before sending through the real HTTP client.
fn transport(id: &'static str, origin: String) -> Fetch {
    Arc::new(move |mut request| {
        let prefixes: &[&str] = match id {
            "anthropic" => &["https://platform.claude.com"],
            "openai-codex" => &["https://auth.openai.com"],
            _ => &[
                "https://github.com",
                "https://api.github.com",
                "https://api.controlled.example",
            ],
        };
        let path = prefixes
            .iter()
            .find_map(|prefix| request.url.strip_prefix(prefix))
            .unwrap();
        assert!(path.starts_with('/'));
        request.url = format!("{origin}{path}");
        maestro_models::default_fetch()(request)
    })
}

/// Synthetic account claim: no signed token or real credential is used.
const ACCOUNT_TOKEN: &str = "x.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiZ3Vlc3QtYWNjb3VudCJ9fQ.x";

/// Checks the actual HTTP consumer's code-exchange inputs, without duplicating OAuth logic.
fn response(id: &str, headers: &str, body: &[u8], policies: &Mutex<Vec<String>>) -> String {
    if id == "github-copilot" {
        return device_response(headers, body, policies);
    }
    if id == "anthropic" {
        assert!(headers.starts_with("POST /v1/oauth/token HTTP/1.1\r\n"));
        let request: Value = serde_json::from_slice(body).unwrap();
        assert_eq!(request["grant_type"], "authorization_code");
        assert_eq!(request["code"], "manual-code");
        assert_eq!(request["redirect_uri"], "http://localhost:53692/callback");
        assert_eq!(request["state"], request["code_verifier"]);
        assert!(!request["state"].as_str().unwrap().is_empty());
    } else {
        assert!(headers.starts_with("POST /oauth/token HTTP/1.1\r\n"));
        let body = std::str::from_utf8(body).unwrap();
        assert!(body.contains("grant_type=authorization_code"));
        assert!(body.contains("code=manual-code"));
        assert!(body.contains("redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback"));
        assert!(body.contains("code_verifier="));
    }
    let access = if id == "anthropic" {
        "native-subscription-access"
    } else {
        ACCOUNT_TOKEN
    };
    json!({"access_token":access,"refresh_token":"native-refresh","expires_in":3600}).to_string()
}

/// Native device flow and model-policy requests all terminate on this peer.
fn device_response(headers: &str, body: &[u8], policies: &Mutex<Vec<String>>) -> String {
    let path = headers
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap();
    match path {
        "/login/device/code" => {
            assert_eq!(
                std::str::from_utf8(body).unwrap(),
                "client_id=Iv1.b507a08c87ecfe98&scope=read%3Auser"
            );
            json!({"device_code":"controlled-device","user_code":"DEVICE","verification_uri":"https://controlled.invalid/device","interval":0,"expires_in":600}).to_string()
        }
        "/login/oauth/access_token" => {
            assert!(
                std::str::from_utf8(body)
                    .unwrap()
                    .contains("device_code=controlled-device")
            );
            json!({"access_token":"native-github-refresh"}).to_string()
        }
        "/copilot_internal/v2/token" => {
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("authorization: bearer native-github-refresh\r\n")
            );
            json!({"token":"proxy-ep=proxy.controlled.example;native-token","expires_at":4_102_444_800_u64}).to_string()
        }
        _ => {
            assert!(path.starts_with("/models/") && path.ends_with("/policy"));
            assert_eq!(body, br#"{"state":"enabled"}"#);
            policies.lock().unwrap().push(path.into());
            "{}".into()
        }
    }
}

/// Service tokens select their existing endpoint modifier and every model policy.
fn device(result: &Value, policies: &Mutex<Vec<String>>) {
    assert_eq!(
        result["credentials"],
        json!({"refresh":"native-github-refresh","access":"proxy-ep=proxy.controlled.example;native-token","expires":4_102_444_500_000_f64})
    );
    assert_eq!(
        result["models"],
        json!({"baseUrls":["https://api.controlled.example","https://before.invalid"],"neighbor":true})
    );
    assert_eq!(
        result["events"],
        json!([
            ["prompt", {"message":"GitHub Enterprise URL/domain (blank for github.com)","placeholder":"company.ghe.com","allowEmpty":true}],
            ["auth", {"url":"https://controlled.invalid/device","instructions":"Enter code: DEVICE"}],
            ["progress", "Enabling models..."]
        ])
    );
    let mut expected = maestro_models::get_models("github-copilot")
        .into_iter()
        .map(|model| format!("/models/{}/policy", model.id))
        .collect::<Vec<_>>();
    expected.sort();
    let mut actual = policies.lock().unwrap().clone();
    actual.sort();
    assert_eq!(actual, expected);
}

/// Manual input passes through the guest callbacks to each original PKCE exchange.
fn subscription(result: &Value, id: &str) {
    assert_eq!(result["credentials"]["refresh"], "native-refresh");
    assert_eq!(result["models"], Value::Null);
    let events = result["events"].as_array().unwrap();
    assert_eq!(events[0][0], "auth");
    assert_eq!(events[1], json!(["manual"]));
    if id == "anthropic" {
        assert_eq!(events.len(), 3);
        assert!(
            events[0][1]["url"]
                .as_str()
                .unwrap()
                .starts_with("https://claude.ai/oauth/authorize?")
        );
        assert_eq!(
            events[2],
            json!(["progress", "Exchanging authorization code for tokens..."])
        );
        assert_eq!(
            result["credentials"]["access"],
            "native-subscription-access"
        );
    } else {
        assert_eq!(events.len(), 2);
        assert!(
            events[0][1]["url"]
                .as_str()
                .unwrap()
                .starts_with("https://auth.openai.com/oauth/authorize?")
        );
        assert_eq!(result["credentials"]["access"], ACCOUNT_TOKEN);
        assert_eq!(result["credentials"]["accountId"], "guest-account");
    }
}
