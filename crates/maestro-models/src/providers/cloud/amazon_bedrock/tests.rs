//! Unit tests for the proxy choice and the configuration decisions.
#![cfg(not(target_arch = "wasm32"))]

use url::Url;

use super::BedrockOptions;
use super::client::{Auth, Settings};
use super::proxy::Environment;
use crate::{Model, get_model};

/// One proxy case: name, destination, environment and the proxy expected, empty for none.
type ProxyRow = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
    &'static str,
);

/// One flag case: environment, whether proxying is on and whether HTTP/1 is required.
type FlagRow = (&'static [(&'static str, &'static str)], bool, bool);

/// An environment holding exactly `pairs`.
fn environment(pairs: &[(&str, &str)]) -> Environment {
    Environment(
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
    )
}

/// The proxy cases: every name is a variable of the proxy choice.
const PROXY_ROWS: &[ProxyRow] = &[
    (
        "HTTP_PROXY",
        "http://origin.example/",
        &[("HTTP_PROXY", "http://chosen.example:3128")],
        "http://chosen.example:3128",
    ),
    (
        "HTTPS_PROXY",
        "https://origin.example/",
        &[("HTTPS_PROXY", "http://chosen.example:3128")],
        "http://chosen.example:3128",
    ),
    (
        "NO_PROXY",
        "https://origin.example/",
        &[("NO_PROXY", "origin.example")],
        "",
    ),
    (
        "http_proxy",
        "http://origin.example/",
        &[("http_proxy", "http://chosen.example:3128")],
        "http://chosen.example:3128",
    ),
    (
        "https_proxy",
        "https://origin.example/",
        &[("https_proxy", "http://chosen.example:3128")],
        "http://chosen.example:3128",
    ),
    (
        "no_proxy",
        "https://origin.example/",
        &[("no_proxy", "origin.example")],
        "",
    ),
    (
        "lowercase_wins",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://upper.example:3128"),
            ("https_proxy", "http://lower.example:3128"),
        ],
        "http://lower.example:3128",
    ),
    (
        "bypass_origin.example",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", "origin.example"),
        ],
        "",
    ),
    (
        "bypass_decoy.example",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", "decoy.example"),
        ],
        "http://chosen.example:3128",
    ),
    (
        "bypass_origin.example:443",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", "origin.example:443"),
        ],
        "",
    ),
    (
        "bypass_origin.example:444",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", "origin.example:444"),
        ],
        "http://chosen.example:3128",
    ),
    (
        "bypass_.example",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", ".example"),
        ],
        "",
    ),
    (
        "bypass_*.example",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", "*.example"),
        ],
        "",
    ),
    (
        "bypass_*",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", "*"),
        ],
        "",
    ),
    (
        "lowercase_bypass_wins",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", "origin.example"),
            ("no_proxy", "decoy.example"),
        ],
        "http://chosen.example:3128",
    ),
    (
        "exact_not_subdomain",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", "example"),
        ],
        "http://chosen.example:3128",
    ),
    (
        "dot_not_bare",
        "https://example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", ".example"),
        ],
        "http://chosen.example:3128",
    ),
    (
        "star_suffix",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", "*example"),
        ],
        "",
    ),
    (
        "semicolon_not_delimiter",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", "decoy.example;origin.example"),
        ],
        "http://chosen.example:3128",
    ),
    (
        "space_delimiter",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", "decoy.example origin.example"),
        ],
        "",
    ),
    (
        "feff_delimiter",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", "decoy.example\u{feff}origin.example"),
        ],
        "",
    ),
    (
        "u0085_not_delimiter",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", "decoy.example\u{85}origin.example"),
        ],
        "http://chosen.example:3128",
    ),
    (
        "empty_entries",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", ", ,origin.example,,"),
        ],
        "",
    ),
    (
        "casefold",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", "ORIGIN.EXAMPLE"),
        ],
        "",
    ),
    (
        "proxy_missing_scheme",
        "https://origin.example/",
        &[("HTTPS_PROXY", "chosen.example:3128")],
        "https://chosen.example:3128",
    ),
    (
        "npm_protocol_wins",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://upper.example:3128"),
            ("npm_config_https_proxy", "http://npm.example:3128"),
        ],
        "http://npm.example:3128",
    ),
    (
        "npm_generic_fallback",
        "https://origin.example/",
        &[
            ("NO_PROXY", "decoy.example"),
            ("npm_config_proxy", "http://npm.example:3128"),
            ("all_proxy", "http://all.example:3128"),
        ],
        "http://npm.example:3128",
    ),
    (
        "all_fallback",
        "https://origin.example/",
        &[
            ("NO_PROXY", "decoy.example"),
            ("ALL_PROXY", "http://all.example:3128"),
        ],
        "http://all.example:3128",
    ),
    (
        "empty_lower_falls_back",
        "https://origin.example/",
        &[
            ("https_proxy", ""),
            ("HTTPS_PROXY", "http://upper.example:3128"),
        ],
        "http://upper.example:3128",
    ),
    (
        "npm_no_proxy_wins",
        "https://origin.example/",
        &[
            ("HTTPS_PROXY", "http://chosen.example:3128"),
            ("NO_PROXY", "decoy.example"),
            ("npm_config_no_proxy", "origin.example"),
        ],
        "",
    ),
];

#[test]
fn proxy_choice_follows_the_variables_and_exclusions() -> Result<(), Box<dyn std::error::Error>> {
    for (id, destination, pairs, expected) in PROXY_ROWS {
        let destination = Url::parse(destination)?;
        let chosen = environment(pairs).proxy_for(&destination)?;
        let expected = (!expected.is_empty())
            .then(|| Url::parse(expected))
            .transpose()?;
        assert_eq!(chosen, expected, "{id}");
    }
    Ok(())
}

#[test]
fn selected_proxy_that_is_not_usable_is_an_error() -> Result<(), Box<dyn std::error::Error>> {
    let destination = Url::parse("http://origin.example/")?;
    for (id, proxy) in [("malformed", "http://"), ("unsupported", "ftp://p.example")] {
        let chosen = environment(&[("HTTP_PROXY", proxy)]).proxy_for(&destination);
        assert!(chosen.is_err(), "{id}");
    }
    let bypassed = environment(&[("HTTP_PROXY", "http://"), ("NO_PROXY", "origin.example")]);
    assert_eq!(bypassed.proxy_for(&destination), Ok(None));
    Ok(())
}

/// The model every settings decision is made for.
fn european_model() -> Result<Model, &'static str> {
    get_model(
        "amazon-bedrock",
        "eu.anthropic.claude-sonnet-4-5-20250929-v1:0",
    )
    .ok_or("the catalog lacks the EU profile")
}

#[test]
fn http1_follows_the_proxy_gate_and_the_force_flag() -> Result<(), Box<dyn std::error::Error>> {
    let model = european_model()?;
    let options = BedrockOptions::default();
    let cases: &[FlagRow] = &[
        (&[], false, false),
        (&[("HTTP_PROXY", "http://p")], true, true),
        (&[("HTTPS_PROXY", "http://p")], true, true),
        (&[("NO_PROXY", "x")], true, true),
        (&[("http_proxy", "http://p")], true, true),
        (&[("https_proxy", "http://p")], true, true),
        (&[("no_proxy", "x")], true, true),
        (&[("HTTP_PROXY", "")], false, false),
        (&[("ALL_PROXY", "http://p")], false, false),
        (&[("AWS_BEDROCK_FORCE_HTTP1", "1")], false, true),
        (&[("AWS_BEDROCK_FORCE_HTTP1", "true")], false, false),
        (
            &[("AWS_BEDROCK_FORCE_HTTP1", "1"), ("HTTP_PROXY", "http://p")],
            true,
            true,
        ),
    ];
    for (pairs, proxy, http1) in cases {
        let settings = Settings::resolve(&model, &options, &environment(pairs))
            .map_err(|failure| failure.message)?;
        assert_eq!(
            (settings.proxy, settings.http1),
            (*proxy, *http1),
            "{pairs:?}"
        );
    }
    Ok(())
}

#[test]
fn skip_flag_replaces_both_bearer_sources_and_only_the_exact_spelling_counts()
-> Result<(), Box<dyn std::error::Error>> {
    let model = european_model()?;
    let options = BedrockOptions {
        bearer_token: Some("explicit".into()),
        ..BedrockOptions::default()
    };
    let skipped = [
        ("AWS_BEDROCK_SKIP_AUTH", "1"),
        ("AWS_BEARER_TOKEN_BEDROCK", "ambient"),
    ];
    let settings = Settings::resolve(&model, &options, &environment(&skipped))
        .map_err(|failure| failure.message)?;
    assert_eq!(settings.auth, Auth::Placeholder);
    let settings = Settings::resolve(&model, &BedrockOptions::default(), &environment(&skipped))
        .map_err(|failure| failure.message)?;
    assert_eq!(settings.auth, Auth::Placeholder);
    let other = [
        ("AWS_BEDROCK_SKIP_AUTH", "true"),
        ("AWS_BEARER_TOKEN_BEDROCK", "ambient"),
    ];
    let settings = Settings::resolve(&model, &options, &environment(&other))
        .map_err(|failure| failure.message)?;
    assert_eq!(settings.auth, Auth::Bearer("explicit".into()));
    Ok(())
}

/// Environment compositions with selected URL spelling, before credential decoding.
type SocksEnvironmentRow = (
    &'static [(&'static str, &'static str)],
    &'static str,
    Option<&'static str>,
);

/// Selected proxy spellings for each environment composition.
const SOCKS_ENVIRONMENT_ROWS: &[SocksEnvironmentRow] = &[
    (
        &[
            ("HTTP_PROXY", "socks5h://bad%ZZ@proxy.example"),
            ("NO_PROXY", "origin.example"),
        ],
        "http://origin.example/path",
        None,
    ),
    (
        &[
            ("HTTP_PROXY", "socks5h://bad%ZZ@proxy.example"),
            ("NO_PROXY", "other.example"),
        ],
        "http://origin.example/path",
        Some("socks5h://bad%ZZ@proxy.example"),
    ),
    (
        &[
            ("HTTP_PROXY", "socks5://upper.example"),
            ("http_proxy", "socks://lower.example"),
        ],
        "http://origin.example/path",
        Some("socks://lower.example"),
    ),
    (
        &[("ALL_PROXY", "socks5h://proxy.example")],
        "http://origin.example/path",
        None,
    ),
    (
        &[
            ("NO_PROXY", "other.example"),
            ("ALL_PROXY", "socks5h://proxy.example"),
        ],
        "http://origin.example/path",
        Some("socks5h://proxy.example"),
    ),
    (
        &[
            ("HTTP_PROXY", "socks5://http.example"),
            ("HTTPS_PROXY", "socks://https.example"),
        ],
        "https://origin.example/path",
        Some("socks://https.example"),
    ),
];

#[test]
fn socks_environment_selection_precedes_url_decoding() {
    for &(pairs, destination, selected) in SOCKS_ENVIRONMENT_ROWS {
        let environment = environment(pairs);
        let chosen = if environment.names_proxy() {
            environment
                .proxy_for(&Url::parse(destination).unwrap())
                .unwrap()
        } else {
            None
        };
        assert!(
            chosen.as_ref().map(Url::as_str) == selected,
            "environment selection"
        );
        if selected.is_some_and(|url| url.contains("%ZZ")) {
            let fetch = crate::providers::http::configured_fetch(crate::providers::http::Route {
                proxy: Some(std::sync::Arc::new(move |_| Ok(chosen.clone()))),
                ..crate::providers::http::Route::default()
            });
            let request = crate::providers::http::HttpRequest {
                method: "GET".into(),
                url: destination.into(),
                headers: indexmap::IndexMap::new(),
                body: Vec::new(),
                signal: None,
            };
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(fetch(request));
            assert!(matches!(
                result,
                Err(crate::providers::http::FetchError::Connection(_))
            ));
        }
    }
}
