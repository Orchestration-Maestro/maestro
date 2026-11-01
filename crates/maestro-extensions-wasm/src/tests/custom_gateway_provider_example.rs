//! Controlled sign-in, token refresh and direct access for the shipped gateway provider.
#[path = "../../../../examples/extensions/custom-provider-gitlab-duo/models.rs"]
mod models;
#[path = "../../../../examples/extensions/custom-provider-gitlab-duo/oauth.rs"]
mod oauth;
#[path = "../../../../examples/extensions/custom-provider-gitlab-duo/provider.rs"]
mod provider;

use super::{bus_callbacks::captured_api, controlled::Controlled};
use crate::{
    OAuthAuthInfo, OAuthCredentials, OAuthLoginCallbacks, OAuthPrompt,
    OAuthProviderInterface,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// The documented greeting loads the component file without a package manifest.
#[test]
fn custom_gateway_provider_greeting_command_loads_the_component_file() {
    let guide = include_str!("../../../../examples/extensions/custom-provider-gitlab-duo.md");
    let command = guide
        .lines()
        .find(|line| line.starts_with("maestro -e "))
        .unwrap();
    let extension = command.split_whitespace().nth(2).unwrap();
    assert_eq!(extension, "./gateway/custom_provider_gitlab_duo.wasm");
}

/// The proof key the controlled host answers with (RFC 7636 appendix B).
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
/// The challenge that belongs to [`VERIFIER`].
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
/// The state those random bytes format to: bytes 0 through 15 with the version and variant set.
const STATE: &str = "00010203-0405-4607-8809-0a0b0c0d0e0f";
/// The bundled client's identifier.
const CLIENT_ID: &str = "da4edff2e6ebd2bc3208611e2768bc1c1dd7be791dc5ff26ca34ca9ee44f7d4b";
/// The redirect address as a form component.
const REDIRECT: &str = "http%3A%2F%2F127.0.0.1%3A8080%2Fcallback";
/// GitLab's token-issue time for the controlled responses, in seconds.
const CREATED_AT: u32 = 1_700_000_000;

/// A provider over the controlled host, with a clock the test moves and fixed entropy.
struct Fixture {
    /// The host that answers every request.
    host: Controlled,
    /// The provider under test.
    provider: provider::GitLabDuo,
    /// Current Unix epoch milliseconds.
    clock: Rc<Cell<f64>>,
}

/// Builds a provider that reaches the network only through the controlled host.
fn fixture() -> Fixture {
    let host = Controlled::default();
    let api = captured_api(&host).unwrap();
    host.observe().actions.value =
        json!({"oauth.generate_pkce": {"verifier": VERIFIER, "challenge": CHALLENGE}});
    let clock = Rc::new(Cell::new(1_000_000.0));
    let now = Rc::clone(&clock);
    let provider = provider::GitLabDuo {
        io: api.native_io(),
        api,
        now: Rc::new(move || now.get()),
        random: Rc::new(|| Ok(std::array::from_fn(|index| u8::try_from(index).unwrap()))),
        direct_access: Rc::default(),
    };
    Fixture {
        host,
        provider,
        clock,
    }
}

/// Sets the response every following request receives.
fn respond(host: &Controlled, status: u16, text: &str) {
    let mut observed = host.observe();
    observed.native_http.status = status;
    observed.native_http.text = text.to_owned();
}

/// A successful token response issued at [`CREATED_AT`].
fn tokens(access: &str, refresh: &str, lifetime: u32) -> String {
    json!({
        "access_token": access, "refresh_token": refresh,
        "expires_in": lifetime, "created_at": CREATED_AT
    })
    .to_string()
}

/// A successful direct access response.
fn granted(token: &str) -> String {
    json!({"token": token, "headers": {"x-gitlab-realm": "saas", "x-gitlab-instance-id": "abc"}})
        .to_string()
}

/// The text of the request at `index`, with its URL, method and headers.
fn request(host: &Controlled, index: usize) -> (String, Option<String>, Vec<(String, String)>, String) {
    let observed = host.observed();
    let sent = &observed.native_http.requests[index];
    let body = sent.body.clone().unwrap_or_default();
    (
        sent.url.clone(),
        sent.method.clone(),
        sent.headers.clone(),
        String::from_utf8(body).unwrap(),
    )
}

/// Interaction that records the authorization address and answers with a derived callback.
fn callbacks(
    published: &Rc<RefCell<Vec<String>>>,
    callback: impl Fn(&str) -> String + 'static,
) -> Rc<OAuthLoginCallbacks> {
    let urls = Rc::clone(published);
    let prompts = Rc::clone(published);
    Rc::new(OAuthLoginCallbacks {
        on_auth: Rc::new(move |info: OAuthAuthInfo| {
            assert!(info.instructions.is_none());
            urls.borrow_mut().push(info.url);
            Ok(())
        }),
        on_prompt: Rc::new(move |prompt: OAuthPrompt| {
            assert_eq!(prompt.message, "Paste the callback URL:");
            assert!(prompt.placeholder.is_none() && prompt.allow_empty.is_none());
            let answer = callback(STATE);
            prompts.borrow_mut().push(prompt.message);
            Box::pin(async move { Ok(answer) })
        }),
        on_progress: None,
        on_manual_code_input: None,
        on_select: None,
        signal: None,
    })
}

/// The callback address GitLab redirects to, carrying the supplied query.
fn redirected(query: &str) -> String {
    format!("http://127.0.0.1:8080/callback?{query}")
}

#[test]
fn custom_gateway_provider_models_keep_the_reference_table() {
    let rows: Vec<Value> = models::provider_models()
        .iter()
        .map(|model| serde_json::to_value(model).unwrap())
        .collect();
    let expected = [
        ("claude-opus-4-5-20251101", "Claude Opus 4.5", [15.0, 75.0, 1.5, 18.75], 200_000.0, 32_000.0),
        ("claude-sonnet-4-5-20250929", "Claude Sonnet 4.5", [3.0, 15.0, 0.3, 3.75], 200_000.0, 16_384.0),
        ("claude-haiku-4-5-20251001", "Claude Haiku 4.5", [1.0, 5.0, 0.1, 1.25], 200_000.0, 8_192.0),
        ("gpt-5.1-2025-11-13", "GPT-5.1", [2.5, 10.0, 0.0, 0.0], 128_000.0, 16_384.0),
        ("gpt-5-mini-2025-08-07", "GPT-5 Mini", [0.15, 0.6, 0.0, 0.0], 128_000.0, 16_384.0),
        ("gpt-5-codex", "GPT-5 Codex", [2.5, 10.0, 0.0, 0.0], 128_000.0, 16_384.0),
    ]
    .map(|(id, name, cost, window, max)| {
        json!({
            "id": id, "name": name, "reasoning": true, "input": ["text", "image"],
            "cost": {"input": cost[0], "output": cost[1], "cacheRead": cost[2], "cacheWrite": cost[3]},
            "contextWindow": window, "maxTokens": max
        })
    });
    assert_eq!(rows, expected);
}

#[test]
fn custom_gateway_provider_models_route_to_their_protocol() {
    for (id, api, base_url) in [
        ("claude-opus-4-5-20251101", "anthropic-messages", "https://cloud.gitlab.com/ai/v1/proxy/anthropic/"),
        ("claude-sonnet-4-5-20250929", "anthropic-messages", "https://cloud.gitlab.com/ai/v1/proxy/anthropic/"),
        ("claude-haiku-4-5-20251001", "anthropic-messages", "https://cloud.gitlab.com/ai/v1/proxy/anthropic/"),
        ("gpt-5.1-2025-11-13", "openai-responses", "https://cloud.gitlab.com/ai/v1/proxy/openai/v1"),
        ("gpt-5-mini-2025-08-07", "openai-responses", "https://cloud.gitlab.com/ai/v1/proxy/openai/v1"),
        ("gpt-5-codex", "openai-responses", "https://cloud.gitlab.com/ai/v1/proxy/openai/v1"),
    ] {
        let model = models::find(id).unwrap();
        assert_eq!(model.backend.api(), api, "{id}");
        assert_eq!(model.backend.base_url(), base_url, "{id}");
    }
    assert!(models::find("gpt-4").is_none());
}

#[test]
fn custom_gateway_provider_login_authorizes_with_a_proof_key_and_exchanges_the_code() {
    let Fixture { host, provider, .. } = fixture();
    respond(&host, 200, &tokens("access-1", "refresh-1", 7200));
    let published = Rc::default();
    let interaction = callbacks(&published, |state| redirected(&format!("code=a%2Fb+c%C3%A9&state={state}#done")));
    let credentials = super::block_on(provider.login(interaction)).unwrap();
    assert_eq!(
        *published.borrow(),
        [
            format!(
                "https://gitlab.com/oauth/authorize?client_id={CLIENT_ID}&redirect_uri={REDIRECT}\
                 &response_type=code&scope=api&code_challenge={CHALLENGE}\
                 &code_challenge_method=S256&state={STATE}"
            ),
            "Paste the callback URL:".to_owned()
        ]
    );
    let (url, method, headers, body) = request(&host, 0);
    assert_eq!(url, "https://gitlab.com/oauth/token");
    assert_eq!(method.as_deref(), Some("POST"));
    assert_eq!(
        headers,
        [("Content-Type".to_owned(), "application/x-www-form-urlencoded".to_owned())]
    );
    assert_eq!(
        body,
        format!(
            "client_id={CLIENT_ID}&grant_type=authorization_code&code=a%2Fb+c%C3%A9\
             &code_verifier={VERIFIER}&redirect_uri={REDIRECT}"
        )
    );
    assert_eq!(
        (credentials.access.as_str(), credentials.refresh.as_str()),
        ("access-1", "refresh-1")
    );
    assert_eq!(json!(credentials.expires), json!(1_700_006_900_000.0));
    assert_eq!(provider.get_api_key(&credentials).unwrap(), "access-1");
    assert_eq!((provider.id(), provider.name()), ("gitlab-duo", "GitLab Duo"));
    assert_eq!(provider.uses_callback_server(), None);
    assert!(provider.modify_models(&[], &credentials).unwrap().is_none());
}

#[test]
fn custom_gateway_provider_login_rejects_callbacks_without_a_matching_code() {
    for (callback, message) in [
        (redirected(&format!("state={STATE}")), "No authorization code found in callback URL"),
        (redirected(&format!("code=&state={STATE}")), "No authorization code found in callback URL"),
        ("not a url".to_owned(), "No authorization code found in callback URL"),
        (redirected("code=granted"), "Authorization state mismatch"),
        (redirected("code=granted&state=another"), "Authorization state mismatch"),
    ] {
        let Fixture { host, provider, .. } = fixture();
        respond(&host, 200, &tokens("never", "never", 1));
        let interaction = callbacks(&Rc::default(), move |_| callback.clone());
        let error = super::block_on(provider.login(interaction)).unwrap_err();
        assert_eq!(error, message);
        assert!(host.observed().native_http.requests.is_empty());
    }
}

#[test]
fn custom_gateway_provider_refresh_rotates_credentials_through_the_token_endpoint() {
    let Fixture { host, provider, .. } = fixture();
    respond(&host, 200, &tokens("access-2", "refresh-2", 3600));
    let stored = OAuthCredentials {
        refresh: "refresh/1 é".into(),
        access: "access-1".into(),
        expires: 1.0,
        extra: serde_json::Map::new(),
    };
    let refreshed = super::block_on(provider.refresh_token(stored)).unwrap();
    let (url, method, _, body) = request(&host, 0);
    assert_eq!((url.as_str(), method.as_deref()), ("https://gitlab.com/oauth/token", Some("POST")));
    assert_eq!(
        body,
        format!("client_id={CLIENT_ID}&grant_type=refresh_token&refresh_token=refresh%2F1+%C3%A9")
    );
    assert_eq!(
        (refreshed.access.as_str(), refreshed.refresh.as_str()),
        ("access-2", "refresh-2")
    );
    assert_eq!(json!(refreshed.expires), json!(1_700_003_300_000.0));
}

#[test]
fn custom_gateway_provider_token_failures_report_the_response_text() {
    let Fixture { host, provider, .. } = fixture();
    respond(&host, 400, "invalid_grant");
    let interaction = callbacks(&Rc::default(), |state| redirected(&format!("code=granted&state={state}")));
    assert_eq!(
        super::block_on(provider.login(interaction)).unwrap_err(),
        "Token exchange failed: invalid_grant"
    );
    let stored = OAuthCredentials {
        refresh: "r".into(),
        access: "a".into(),
        expires: 1.0,
        extra: serde_json::Map::new(),
    };
    assert_eq!(
        super::block_on(provider.refresh_token(stored.clone())).unwrap_err(),
        "Token refresh failed: invalid_grant"
    );
    host.observe().native_http.read_error = Some("body interrupted".into());
    assert_eq!(
        super::block_on(provider.refresh_token(stored)).unwrap_err(),
        "Token refresh failed: body interrupted"
    );
}

#[test]
fn custom_gateway_provider_direct_access_is_requested_once_per_twenty_five_minutes() {
    let Fixture { host, provider, clock } = fixture();
    respond(&host, 200, &granted("direct-1"));
    let first = super::block_on(provider.get_direct_access_token("glpat-user")).unwrap();
    assert_eq!(first.token, "direct-1");
    assert_eq!(first.headers["x-gitlab-realm"], "saas");
    let (url, method, headers, body) = request(&host, 0);
    assert_eq!(url, "https://gitlab.com/api/v4/ai/third_party_agents/direct_access");
    assert_eq!(method.as_deref(), Some("POST"));
    assert_eq!(
        headers,
        [
            ("Authorization".to_owned(), "Bearer glpat-user".to_owned()),
            ("Content-Type".to_owned(), "application/json".to_owned())
        ]
    );
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap(), json!({"feature_flags": {"DuoAgentPlatformNext": true}}));
    clock.set(1_000_000.0 + 25.0 * 60.0 * 1000.0 - 1.0);
    assert_eq!(super::block_on(provider.get_direct_access_token("glpat-user")).unwrap().token, "direct-1");
    assert_eq!(host.observed().native_http.requests.len(), 1);
    respond(&host, 200, &granted("direct-2"));
    clock.set(1_000_000.0 + 25.0 * 60.0 * 1000.0);
    assert_eq!(super::block_on(provider.get_direct_access_token("glpat-user")).unwrap().token, "direct-2");
    assert_eq!(host.observed().native_http.requests.len(), 2);
}

#[test]
fn custom_gateway_provider_direct_access_follows_the_supplied_credential() {
    let Fixture { host, provider, .. } = fixture();
    for (index, credential) in ["account-a", "account-b", "account-a"].into_iter().enumerate() {
        let token = format!("direct-{index}");
        respond(&host, 200, &granted(&token));
        let granted = super::block_on(provider.get_direct_access_token(credential)).unwrap();
        assert_eq!(granted.token, token);
        assert_eq!(host.observed().native_http.requests.len(), index + 1);
        assert_eq!(request(&host, index).2[0].1, format!("Bearer {credential}"));
    }
}

#[test]
fn custom_gateway_provider_new_credentials_expire_the_direct_access_token() {
    let Fixture { host, provider, .. } = fixture();
    respond(&host, 200, &granted("direct-1"));
    super::block_on(provider.get_direct_access_token("glpat-user")).unwrap();
    respond(&host, 200, &tokens("access-2", "refresh-2", 3600));
    let stored = OAuthCredentials { refresh: "r".into(), access: "a".into(), expires: 1.0, extra: serde_json::Map::new() };
    super::block_on(provider.refresh_token(stored)).unwrap();
    respond(&host, 200, &granted("direct-2"));
    assert_eq!(super::block_on(provider.get_direct_access_token("glpat-user")).unwrap().token, "direct-2");
    assert_eq!(host.observed().native_http.requests.len(), 3);
    respond(&host, 400, "invalid_grant");
    let stored = OAuthCredentials { refresh: "r".into(), access: "a".into(), expires: 1.0, extra: serde_json::Map::new() };
    assert!(super::block_on(provider.refresh_token(stored)).is_err());
    assert_eq!(super::block_on(provider.get_direct_access_token("glpat-user")).unwrap().token, "direct-2");
    assert_eq!(host.observed().native_http.requests.len(), 4);
}

#[test]
fn custom_gateway_provider_direct_access_failures_name_the_cause() {
    let Fixture { host, provider, .. } = fixture();
    respond(&host, 403, "forbidden");
    assert_eq!(
        super::block_on(provider.get_direct_access_token("glpat-user")).unwrap_err(),
        "GitLab Duo access denied. Ensure GitLab Duo is enabled for your account. Error: forbidden"
    );
    respond(&host, 500, "boom");
    assert_eq!(
        super::block_on(provider.get_direct_access_token("glpat-user")).unwrap_err(),
        "Failed to get direct access token: 500 boom"
    );
    respond(&host, 502, "unread");
    host.observe().native_http.read_error = Some("body interrupted".into());
    assert_eq!(
        super::block_on(provider.get_direct_access_token("glpat-user")).unwrap_err(),
        "Failed to get direct access token: 502 body interrupted"
    );
}
