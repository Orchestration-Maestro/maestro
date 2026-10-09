//! Request preparation witnesses.

use super::super::request::extract_account_id;
use serde::Deserialize;

/// One credential and its surviving account claim.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountCase {
    /// Complete encoded credential.
    token: String,
    /// Account or an extraction failure.
    account: Option<String>,
}

#[test]
fn maestro_response_sessions_extract_account_claim() {
    let rows: Vec<AccountCase> =
        serde_json::from_str(include_str!("fixtures/account.json")).unwrap();
    for row in rows {
        match row.account {
            Some(account) => assert_eq!(extract_account_id(&row.token).unwrap(), account),
            None => assert_eq!(
                extract_account_id(&row.token).unwrap_err().message,
                "Failed to extract accountId from token"
            ),
        }
    }
}

/// Recorded request selections and their complete wire payload.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BodyCase {
    /// Supplied descriptor.
    model: crate::Model,
    /// Conversation.
    context: crate::Context,
    /// Selected common request settings.
    options: RequestOptions,
    /// Complete ordered body.
    expected: serde_json::Value,
}

#[test]
fn maestro_response_sessions_build_request_defaults() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let rows: Vec<BodyCase> =
            serde_json::from_str(include_str!("fixtures/defaults.json")).unwrap();
        for row in rows {
            let options = row.options.into();
            let prepared = super::super::request::prepare_request(
                &std::sync::Arc::new(row.model),
                &row.context,
                &options,
                "maestro (browser)",
            )
            .await
            .unwrap();
            assert_eq!(
                prepared.url,
                "https://chatgpt.com/backend-api/codex/responses"
            );
            assert_eq!(prepared.headers["originator"], "maestro");
            assert_eq!(
                crate::providers::json_text::compact_json(&prepared.body).unwrap(),
                crate::providers::json_text::compact_json(&row.expected).unwrap()
            );
        }
    });
}

/// Common settings present in the request corpus.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequestOptions {
    /// Temperature, including zero.
    temperature: Option<f64>,
    /// Explicit null is different from omission.
    #[serde(default, deserialize_with = "present")]
    service_tier: Option<serde_json::Value>,
    /// Cache key, including empty.
    session_id: Option<String>,
    /// Verbosity spelling.
    text_verbosity: Option<String>,
    /// Common output limit, ignored by this provider.
    max_tokens: Option<f64>,
    /// Common metadata, ignored by this provider.
    metadata: Option<crate::JsonObject>,
    /// Common cache retention, ignored by this provider.
    cache_retention: Option<crate::CacheRetention>,
}

/// Preserve an explicitly present null in fixture selections.
fn present<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Option<serde_json::Value>, D::Error> {
    serde_json::Value::deserialize(decoder).map(Some)
}

impl From<RequestOptions> for super::super::OpenAICodexResponsesOptions {
    fn from(input: RequestOptions) -> Self {
        use super::super::OpenAICodexTextVerbosity as Verbosity;
        use crate::providers::responses::openai_responses::OpenAIResponsesServiceTier as Tier;
        Self {
            common: crate::StreamOptions {
                api_key: Some("a.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjX3Rlc3QifX0=.b".to_owned()),
                temperature: input.temperature,
                session_id: input.session_id,
                max_tokens: input.max_tokens,
                metadata: input.metadata,
                cache_retention: input.cache_retention,
                ..Default::default()
            },
            service_tier: input.service_tier.map(|tier| match tier.as_str() {
                None if tier.is_null() => crate::providers::nullable::Nullable::Null,
                Some("scale") => crate::providers::nullable::Nullable::Value(Tier::Scale),
                _ => panic!("unexpected fixture tier"),
            }),
            text_verbosity: input.text_verbosity.map(|value| match value.as_str() {
                "high" => Verbosity::High,
                _ => panic!("unexpected fixture verbosity"),
            }),
        }
    }
}

#[test]
fn maestro_response_sessions_select_text_verbosity() {
    use super::super::OpenAICodexTextVerbosity as Verbosity;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let row: BodyCase =
            serde_json::from_str::<Vec<BodyCase>>(include_str!("fixtures/defaults.json"))
                .unwrap()
                .remove(0);
        let model = std::sync::Arc::new(row.model);
        let mut options: super::super::OpenAICodexResponsesOptions = row.options.into();
        for (verbosity, expected) in [
            (None, "low"),
            (Some(Verbosity::Low), "low"),
            (Some(Verbosity::Medium), "medium"),
            (Some(Verbosity::High), "high"),
        ] {
            options.text_verbosity = verbosity;
            let prepared = super::super::request::prepare_request(
                &model,
                &row.context,
                &options,
                "maestro (browser)",
            )
            .await
            .unwrap();
            assert_eq!(prepared.body["text"]["verbosity"], expected);
        }
    });
}

/// One authored base and resolved endpoint.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EndpointCase {
    /// Optional authored path.
    base: Option<String>,
    /// Resolved lexical spelling.
    expected: String,
}

#[test]
fn maestro_response_sessions_resolve_sse_endpoint() {
    let rows: Vec<EndpointCase> =
        serde_json::from_str(include_str!("fixtures/endpoints.json")).unwrap();
    for row in rows {
        assert_eq!(
            super::super::request::resolve_codex_url(row.base.as_deref().unwrap_or_default()),
            row.expected
        );
    }
}

/// Recorded header layers.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HeaderInput {
    /// Initial model fields.
    init: indexmap::IndexMap<String, String>,
    /// Additional replacing fields.
    #[serde(default)]
    additional: indexmap::IndexMap<String, String>,
    /// Account claim.
    account: String,
    /// Credential.
    token: String,
    /// Optional affinity identity.
    session: Option<String>,
    /// Injected host identity.
    user_agent: String,
}

/// Effective headers at the transport seam.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HeaderCase {
    /// Authored layers.
    input: HeaderInput,
    /// Complete effective field values.
    expected: indexmap::IndexMap<String, String>,
}

#[test]
fn maestro_response_sessions_apply_sse_headers() {
    let rows: Vec<HeaderCase> =
        serde_json::from_str(include_str!("fixtures/headers.json")).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        for row in rows {
            let base: BodyCase =
                serde_json::from_str::<Vec<BodyCase>>(include_str!("fixtures/defaults.json"))
                    .unwrap()
                    .remove(0);
            let mut model = base.model;
            model.headers = Some(row.input.init);
            let options = super::super::OpenAICodexResponsesOptions {
                common: crate::StreamOptions {
                    api_key: Some(row.input.token),
                    headers: Some(row.input.additional),
                    session_id: row.input.session,
                    ..Default::default()
                },
                ..Default::default()
            };
            let prepared = super::super::request::prepare_request(
                &std::sync::Arc::new(model),
                &base.context,
                &options,
                &row.input.user_agent,
            )
            .await
            .unwrap();
            assert_eq!(prepared.headers["chatgpt-account-id"], row.input.account);
            assert_eq!(prepared.headers, row.expected);
        }
    });
}

#[test]
fn maestro_response_sessions_convert_history_and_tools() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let rows: Vec<BodyCase> =
            serde_json::from_str(include_str!("fixtures/history.json")).unwrap();
        for row in rows {
            let options = row.options.into();
            let prepared = super::super::request::prepare_request(
                &std::sync::Arc::new(row.model),
                &row.context,
                &options,
                "maestro (browser)",
            )
            .await
            .unwrap();
            assert_eq!(
                crate::providers::json_text::compact_json(&prepared.body).unwrap(),
                crate::providers::json_text::compact_json(&row.expected).unwrap()
            );
        }
    });
}
