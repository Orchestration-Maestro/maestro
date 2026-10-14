//! Direct Bedrock request construction and configuration through the SDK with a controlled sender.

use crate::bedrock_cases as cases;
use crate::bedrock_routes as routes;
use crate::bedrock_rows as rows;
use crate::chat;
use crate::child_process;

use cases::{Case, Expect, run_cases};
use chat::TestResult;
use routes::{Refusal, Route, run_refusals, run_routes};

const EU: &str = "https://bedrock-runtime.eu-central-1.amazonaws.com";

/// Run every row the test owns in the request fixture.
fn check_rows(test: &str) -> TestResult {
    rows::check_rows(
        include_str!("fixtures/cloud_conversation_requests.json"),
        test,
    )
}

#[test]
fn payload_hook_observes_sdk_input_before_serialization() -> TestResult {
    check_rows("payload_hook_observes_sdk_input_before_serialization")
}

#[test]
fn payload_edits_and_replacements_reach_the_sdk() -> TestResult {
    check_rows("payload_edits_and_replacements_reach_the_sdk")
}

#[test]
fn hook_added_request_fields_survive_without_parallel_options() -> TestResult {
    check_rows("hook_added_request_fields_survive_without_parallel_options")
}

#[test]
fn selected_request_fields_keep_presence_and_supported_values() -> TestResult {
    check_rows("selected_request_fields_keep_presence_and_supported_values")
}

#[test]
fn sdk_selection_drops_unknown_structure_members() -> TestResult {
    check_rows("sdk_selection_drops_unknown_structure_members")
}

#[test]
fn wire_numbers_keep_double_precision() -> TestResult {
    check_rows("wire_numbers_keep_double_precision")
}

#[test]
fn text_request_keeps_empty_whitespace_unicode_and_order() -> TestResult {
    check_rows("text_request_keeps_empty_whitespace_unicode_and_order")
}

#[test]
fn payload_failure_precedes_sdk_resolution() -> TestResult {
    check_rows("payload_failure_precedes_sdk_resolution")
}

#[test]
fn route_failure_wins_over_later_body_conversion() -> TestResult {
    check_rows("route_failure_wins_over_later_body_conversion")
}

#[test]
fn bearer_and_skip_auth_use_the_selected_authentication() -> TestResult {
    const TEST: &str = "bearer_and_skip_auth_use_the_selected_authentication";
    let cases = [
        Case {
            bearer: Some(""),
            env: &[("AWS_BEARER_TOKEN_BEDROCK", "environment")],
            ..Case::new(
                "empty option falls back",
                EU,
                Expect::Authorized("Bearer environment"),
            )
        },
        Case {
            bearer: Some("explicit"),
            env: &[("AWS_BEDROCK_SKIP_AUTH", "true")],
            ..Case::new(
                "other skip spellings do not skip",
                EU,
                Expect::Authorized("Bearer explicit"),
            )
        },
    ];
    let is_case =
        child_process::child_case().is_some_and(|id| cases.iter().any(|case| case.id == id));
    if is_case {
        return run_cases(TEST, &cases);
    }
    check_rows(TEST)?;
    if child_process::child_case().is_none() {
        run_cases(TEST, &cases)?;
    }
    Ok(())
}

/// Cases where the region comes from options, the environment or the endpoint.
fn region_cases() -> Vec<Case> {
    const ALL: &[(&str, &str)] = &[
        ("AWS_REGION", "us-west-1"),
        ("AWS_DEFAULT_REGION", "ap-northeast-1"),
    ];
    const KEY: &str = "controlled-default";
    vec![
        Case::new(
            "derived from the endpoint",
            EU,
            Expect::signed(EU, "eu-central-1", KEY),
        ),
        Case {
            region: Some("eu-west-2"),
            env: ALL,
            ..Case::new("option wins", EU, Expect::standard("eu-west-2", KEY))
        },
        Case {
            env: ALL,
            ..Case::new(
                "AWS_REGION wins over AWS_DEFAULT_REGION",
                EU,
                Expect::standard("us-west-1", KEY),
            )
        },
        Case {
            env: &[("AWS_DEFAULT_REGION", "ap-northeast-1")],
            ..Case::new(
                "AWS_DEFAULT_REGION",
                EU,
                Expect::standard("ap-northeast-1", KEY),
            )
        },
        Case {
            region: Some(""),
            env: &[("AWS_REGION", ""), ("AWS_DEFAULT_REGION", "ap-northeast-1")],
            ..Case::new(
                "empty values fall through",
                EU,
                Expect::standard("ap-northeast-1", KEY),
            )
        },
        Case {
            env: &[("AWS_REGION", "us-east-2")],
            ..Case::new(
                "standard endpoint is not pinned",
                "https://bedrock-runtime.us-east-1.amazonaws.com",
                Expect::standard("us-east-2", KEY),
            )
        },
    ]
}

/// Cases where the endpoint's shape decides the region or the route.
fn endpoint_cases() -> Vec<Case> {
    const KEY: &str = "controlled-default";
    let at = |id, base, url, region| Case::new(id, base, Expect::signed(url, region, KEY));
    vec![
        at(
            "FIPS host",
            "https://bedrock-runtime-fips.us-gov-west-1.amazonaws.com",
            "https://bedrock-runtime-fips.us-gov-west-1.amazonaws.com/model/",
            "us-gov-west-1",
        ),
        at(
            "China host",
            "https://bedrock-runtime.cn-north-1.amazonaws.com.cn",
            "https://bedrock-runtime.cn-north-1.amazonaws.com.cn/model/",
            "cn-north-1",
        ),
        at(
            "host case",
            "https://BEDROCK-Runtime.EU-West-1.AmazonAWS.com",
            "https://bedrock-runtime.eu-west-1.amazonaws.com/model/",
            "eu-west-1",
        ),
        at(
            "port",
            "https://bedrock-runtime.eu-west-1.amazonaws.com:8443",
            "https://bedrock-runtime.eu-west-1.amazonaws.com:8443/model/",
            "eu-west-1",
        ),
        Case {
            env: &[("AWS_REGION", "us-west-2")],
            ..at(
                "custom endpoint",
                "https://bedrock-vpc.example.com",
                "https://bedrock-vpc.example.com/model/",
                "us-west-2",
            )
        },
        at(
            "lookalike host is custom",
            "https://bedrock-runtime.eu-west-1.amazonaws.com.evil.example",
            "https://bedrock-runtime.eu-west-1.amazonaws.com.evil.example/model/",
            "us-east-1",
        ),
        Case::new("invalid endpoint", "bad url", Expect::Fails),
        Case::new("empty endpoint", "", Expect::Fails),
    ]
}

#[test]
fn endpoint_resolution_preserves_region_and_custom_routes() -> TestResult {
    let mut all = region_cases();
    all.extend(endpoint_cases());
    run_cases(
        "endpoint_resolution_preserves_region_and_custom_routes",
        &all,
    )
}

#[test]
fn profile_selection_defers_standard_endpoint_to_sdk() -> TestResult {
    const CUSTOM: &str = "https://custom.example/route";
    const WEST: &str = "https://bedrock-runtime.us-west-2.amazonaws.com/model/";
    run_cases(
        "profile_selection_defers_standard_endpoint_to_sdk",
        &[
            Case {
                profile: Some("selected"),
                ..Case::new(
                    "option profile",
                    EU,
                    Expect::signed(WEST, "us-west-2", "controlled-selected"),
                )
            },
            Case {
                env: &[("AWS_PROFILE", "selected")],
                ..Case::new(
                    "environment profile",
                    EU,
                    Expect::signed(WEST, "us-west-2", "controlled-selected"),
                )
            },
            Case {
                profile: Some("selected"),
                region: Some("eu-west-2"),
                ..Case::new(
                    "explicit region wins",
                    EU,
                    Expect::signed(
                        "https://bedrock-runtime.eu-west-2.amazonaws.com/model/",
                        "eu-west-2",
                        "controlled-selected",
                    ),
                )
            },
            Case {
                profile: Some("selected"),
                ..Case::new(
                    "custom endpoint with option profile",
                    CUSTOM,
                    Expect::signed(
                        "https://custom.example/route/model/",
                        "us-west-2",
                        "controlled-selected",
                    ),
                )
            },
            Case {
                env: &[("AWS_PROFILE", "selected")],
                ..Case::new(
                    "custom endpoint with environment profile",
                    CUSTOM,
                    Expect::signed(
                        "https://custom.example/route/model/",
                        "us-west-2",
                        "controlled-selected",
                    ),
                )
            },
        ],
    )
}

#[test]
fn native_credentials_sign_without_api_key() -> TestResult {
    run_cases(
        "native_credentials_sign_without_api_key",
        &[
            Case::new(
                "shared file",
                EU,
                Expect::signed(EU, "eu-central-1", "controlled-default"),
            ),
            Case {
                env: &[
                    ("AWS_ACCESS_KEY_ID", "environment-key"),
                    ("AWS_SECRET_ACCESS_KEY", "environment-secret"),
                ],
                ..Case::new(
                    "environment",
                    EU,
                    Expect::signed(EU, "eu-central-1", "environment-key"),
                )
            },
        ],
    )
}

#[test]
fn selected_profile_skips_environment_credentials() -> TestResult {
    const KEYS: &[(&str, &str)] = &[
        ("AWS_ACCESS_KEY_ID", "environment-key"),
        ("AWS_SECRET_ACCESS_KEY", "environment-secret"),
    ];
    const KEYS_AND_PROFILE: &[(&str, &str)] = &[
        ("AWS_ACCESS_KEY_ID", "environment-key"),
        ("AWS_SECRET_ACCESS_KEY", "environment-secret"),
        ("AWS_PROFILE", "selected"),
    ];
    const KEYS_AND_EMPTY_PROFILE: &[(&str, &str)] = &[
        ("AWS_ACCESS_KEY_ID", "environment-key"),
        ("AWS_SECRET_ACCESS_KEY", "environment-secret"),
        ("AWS_PROFILE", ""),
    ];
    run_cases(
        "selected_profile_skips_environment_credentials",
        &[
            Case {
                profile: Some("selected"),
                env: KEYS,
                ..Case::new(
                    "option profile",
                    EU,
                    Expect::standard("us-west-2", "controlled-selected"),
                )
            },
            Case {
                env: KEYS_AND_PROFILE,
                ..Case::new(
                    "environment profile",
                    EU,
                    Expect::standard("us-west-2", "controlled-selected"),
                )
            },
            Case {
                env: &[("AWS_PROFILE", "")],
                ..Case::new(
                    "empty environment profile uses the default profile",
                    EU,
                    Expect::signed(EU, "eu-central-1", "controlled-default"),
                )
            },
            Case {
                env: KEYS_AND_EMPTY_PROFILE,
                ..Case::new(
                    "empty environment profile keeps environment credentials",
                    EU,
                    Expect::signed(EU, "eu-central-1", "environment-key"),
                )
            },
        ],
    )
}

#[test]
fn catalog_regions_use_standard_hosts() -> TestResult {
    let named = maestro_models::get_model(
        "amazon-bedrock",
        "eu.anthropic.claude-sonnet-4-5-20250929-v1:0",
    )
    .ok_or("the catalog lacks the EU profile")?;
    assert_eq!(named.base_url, EU);
    let european: Vec<_> = maestro_models::get_models("amazon-bedrock")
        .into_iter()
        .filter(|model| model.id.starts_with("eu."))
        .collect();
    assert!(european.len() > 1, "expected several EU profiles");
    assert!(european.iter().all(|model| model.base_url == EU));
    Ok(())
}

#[test]
fn proxy_variables_and_bypass_reach_selected_route() -> TestResult {
    const BYPASS_OTHER: (&str, &str) = ("NO_PROXY", "example.org");
    run_routes(
        "proxy_variables_and_bypass_reach_selected_route",
        &[
            Route {
                id: "HTTP_PROXY",
                env: &[("HTTP_PROXY", "PROXY")],
                proxied: true,
            },
            Route {
                id: "http_proxy",
                env: &[("http_proxy", "PROXY")],
                proxied: true,
            },
            Route {
                id: "lowercase wins",
                env: &[
                    ("HTTP_PROXY", "http://127.0.0.1:1"),
                    ("http_proxy", "PROXY"),
                ],
                proxied: true,
            },
            Route {
                id: "excluded host",
                env: &[("HTTP_PROXY", "PROXY"), ("NO_PROXY", "127.0.0.1")],
                proxied: false,
            },
            Route {
                id: "other excluded host",
                env: &[("HTTP_PROXY", "PROXY"), BYPASS_OTHER],
                proxied: true,
            },
            Route {
                id: "no variable",
                env: &[],
                proxied: false,
            },
            Route {
                id: "ALL_PROXY alone",
                env: &[("ALL_PROXY", "PROXY")],
                proxied: false,
            },
            Route {
                id: "ALL_PROXY after the gate",
                env: &[BYPASS_OTHER, ("ALL_PROXY", "PROXY")],
                proxied: true,
            },
            Route {
                id: "npm protocol variable after the gate",
                env: &[BYPASS_OTHER, ("npm_config_http_proxy", "PROXY")],
                proxied: true,
            },
            Route {
                id: "HTTPS_PROXY for an http origin",
                env: &[("HTTPS_PROXY", "PROXY")],
                proxied: false,
            },
        ],
    )
}

#[test]
fn unusable_selected_proxy_fails_before_any_connection() -> TestResult {
    run_refusals(
        "unusable_selected_proxy_fails_before_any_connection",
        &[
            Refusal {
                id: "malformed",
                env: &[("HTTP_PROXY", "http://")],
                redirects: false,
            },
            Refusal {
                id: "unsupported scheme",
                env: &[("HTTP_PROXY", "ftp://proxy.example")],
                redirects: false,
            },
        ],
    )
}

#[test]
fn redirects_are_returned_without_being_followed() -> TestResult {
    run_refusals(
        "redirects_are_returned_without_being_followed",
        &[Refusal {
            id: "redirect",
            env: &[],
            redirects: true,
        }],
    )
}
