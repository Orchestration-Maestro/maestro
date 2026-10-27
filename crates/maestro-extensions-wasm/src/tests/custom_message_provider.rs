//! Controlled authorization and streaming for the shipped message provider.
#[path = "../../../../examples/extensions/custom-provider-anthropic/pkce.rs"]
mod pkce;

#[test]
fn custom_message_provider_pkce_matches_rfc7636() {
    let bytes = [
        116, 24, 223, 180, 151, 153, 224, 37, 79, 250, 96, 125, 216, 173, 187, 186, 22, 212, 37,
        77, 105, 214, 191, 240, 91, 88, 5, 88, 83, 132, 141, 121,
    ];
    assert_eq!(
        pkce::generate(bytes),
        (
            "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into(),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".into()
        )
    );
}

#[path = "../../../../examples/extensions/custom-provider-anthropic/oauth.rs"]
mod oauth;

use super::{bus_callbacks::captured_api, controlled::Controlled};
use crate::{ExtensionResult, OAuthAuthInfo, OAuthLoginCallbacks, OAuthProviderInterface};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Supplies an authorization code bound to the published proof key.
fn callbacks() -> Rc<OAuthLoginCallbacks> {
    let published = Rc::new(RefCell::new(String::new()));
    let url = published.clone();
    Rc::new(OAuthLoginCallbacks {
        on_auth: Rc::new(move |info: OAuthAuthInfo| {
            assert!(info.url.starts_with("https://claude.ai/oauth/authorize?"));
            assert!(info.url.contains("code_challenge_method=S256"));
            assert!(
                info.url
                    .contains("code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM")
            );
            assert!(info.instructions.is_none());
            *url.borrow_mut() = info.url;
            Ok(())
        }),
        on_prompt: Rc::new(move |prompt| {
            assert_eq!(prompt.message, "Paste the authorization code:");
            let url = published.borrow();
            let state = url
                .split("state=")
                .nth(1)
                .unwrap()
                .split('&')
                .next()
                .unwrap();
            let code = format!("controlled-code#{state}");
            Box::pin(async move { Ok(code) })
        }),
        on_progress: None,
        on_manual_code_input: None,
        on_select: None,
        signal: None,
    })
}

/// Supplies the exact RFC verifier bytes, with no ambient entropy or clock reads.
fn authorization(host: &Controlled) -> Rc<dyn OAuthProviderInterface> {
    Rc::new(oauth::CustomAnthropic {
        io: captured_api(host).unwrap().native_io(),
        entropy: Rc::new(|| {
            Ok([
                116, 24, 223, 180, 151, 153, 224, 37, 79, 250, 96, 125, 216, 173, 187, 186, 22,
                212, 37, 77, 105, 214, 191, 240, 91, 88, 5, 88, 83, 132, 141, 121,
            ])
        }),
        now: Rc::new(|| 10_000.0),
    })
}

/// Installs a controlled token response without changing the host's HTTP implementation.
fn tokens(host: &Controlled, access: &str, refresh: &str, lifetime: u32) {
    host.observe().native_http.text = json!({
        "access_token": access, "refresh_token": refresh, "expires_in": lifetime
    })
    .to_string();
}

#[test]
fn custom_message_provider_authorizes_and_refreshes_through_host_http() -> ExtensionResult<()> {
    let host = Controlled::default();
    let provider = authorization(&host);
    tokens(&host, "access-first", "refresh-first", 3600);
    let credentials = super::block_on(provider.login(callbacks()))?;
    assert_eq!(credentials.access, "access-first");
    assert_eq!(credentials.refresh, "refresh-first");
    assert_eq!(json!(credentials.expires), json!(3_310_000.0));
    assert_eq!(provider.get_api_key(&credentials)?, "access-first");
    assert_eq!(provider.id(), "custom-anthropic");
    assert_eq!(provider.name(), "Custom Anthropic (Claude Pro/Max)");
    assert_eq!(provider.uses_callback_server(), None);
    assert!(provider.modify_models(&[], &credentials)?.is_none());
    tokens(&host, "access-next", "refresh-next", 7200);
    let refreshed = super::block_on(provider.refresh_token(credentials))?;
    assert_eq!(refreshed.access, "access-next");
    assert_eq!(refreshed.refresh, "refresh-next");
    assert_eq!(json!(refreshed.expires), json!(6_910_000.0));
    check_token_requests(&host);
    Ok(())
}

/// Checks both token exchanges at the host HTTP interface.
fn check_token_requests(host: &Controlled) {
    let observed = host.observed();
    assert_eq!(observed.native_http.requests.len(), 2);
    let requests = &observed.native_http.requests;
    for request in requests {
        assert_eq!(request.url, "https://console.anthropic.com/v1/oauth/token");
        assert_eq!(request.method.as_deref(), Some("POST"));
        assert_eq!(
            request.headers,
            [("Content-Type".into(), "application/json".into())]
        );
    }
    let login: Value = serde_json::from_slice(requests[0].body.as_ref().unwrap()).unwrap();
    assert_eq!(
        login,
        json!({
            "grant_type":"authorization_code", "client_id":"9d1c250a-e61b-44d9-88ed-5944d1962f5e",
            "code":"controlled-code", "state":"dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
            "redirect_uri":"https://console.anthropic.com/oauth/code/callback",
            "code_verifier":"dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
        })
    );
    let refresh: Value = serde_json::from_slice(requests[1].body.as_ref().unwrap()).unwrap();
    assert_eq!(
        refresh,
        json!({"grant_type":"refresh_token", "client_id":"9d1c250a-e61b-44d9-88ed-5944d1962f5e", "refresh_token":"refresh-first"})
    );
}

#[test]
fn custom_message_provider_rejects_mismatched_authorization_state() {
    let host = Controlled::default();
    let provider = authorization(&host);
    tokens(&host, "must-not-use", "must-not-use", 3600);
    let mut interaction = callbacks();
    Rc::get_mut(&mut interaction).unwrap().on_prompt =
        Rc::new(|_| Box::pin(async { Ok("controlled-code#unrelated-state".into()) }));
    let result = super::block_on(provider.login(interaction));
    assert_eq!(result.unwrap_err(), "Authorization state mismatch");
    assert!(host.observed().native_http.requests.is_empty());
}
