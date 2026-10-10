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
        super::fixture_rows(include_str!("fixtures/account.json"), &["token"]).unwrap();
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
    /// Named consumers sharing this one full query.
    cases: Vec<String>,
    /// Supplied descriptor changes.
    #[serde(deserialize_with = "descriptor")]
    model: crate::Model,
    /// Conversation, or the unchanged controlled context.
    #[serde(
        default = "super::controlled_context",
        deserialize_with = "conversation"
    )]
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
        let rows = body_cases("defaults").unwrap();
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
    /// Raw requested effort.
    reasoning_effort: Option<String>,
    /// Summary selection, null and omission both default.
    reasoning_summary: Option<String>,
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
pub(super) fn present<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Option<serde_json::Value>, D::Error> {
    serde_json::Value::deserialize(decoder).map(Some)
}

impl From<RequestOptions> for super::super::OpenAICodexResponsesOptions {
    fn from(input: RequestOptions) -> Self {
        use super::super::OpenAICodexReasoningSummary as Summary;
        use super::super::OpenAICodexTextVerbosity as Verbosity;
        use crate::providers::responses::openai_responses::OpenAIResponsesServiceTier as Tier;
        let effort = input.reasoning_effort.map(|name| match name.as_str() {
            "none" => crate::ModelThinkingLevel::Off,
            "minimal" => crate::ModelThinkingLevel::Minimal,
            "low" => crate::ModelThinkingLevel::Low,
            "medium" => crate::ModelThinkingLevel::Medium,
            "high" => crate::ModelThinkingLevel::High,
            "xhigh" => crate::ModelThinkingLevel::Xhigh,
            _ => panic!("invalid fixture effort"),
        });
        let summary = input.reasoning_summary.map(|name| match name.as_str() {
            "auto" => Summary::Auto,
            "concise" => Summary::Concise,
            "detailed" => Summary::Detailed,
            "off" => Summary::Off,
            "on" => Summary::On,
            _ => panic!("invalid fixture summary"),
        });
        Self {
            reasoning_effort: effort,
            reasoning_summary: summary,
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
        let row: BodyCase = body_cases("defaults").unwrap().remove(0);
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
        super::fixture_rows(include_str!("fixtures/endpoints.json"), &["base"]).unwrap();
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
        super::fixture_rows(include_str!("fixtures/headers.json"), &["input"]).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        for row in rows {
            let base: BodyCase = body_cases("defaults").unwrap().remove(0);
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
        let rows = body_cases("history").unwrap();
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

/// Assert complete wire documents from the independent request corpus.
async fn assert_bodies(text: &str) {
    let rows = body_cases(text).unwrap();
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
}

#[test]
fn maestro_response_sessions_map_reasoning_from_descriptor() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(assert_bodies("reasoning"));
}

#[test]
fn maestro_response_sessions_keep_raw_minimal_without_mapping() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(assert_bodies("minimal"));
}

#[test]
fn maestro_response_sessions_default_reasoning_summary() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(assert_bodies("summary"));
}

/// Status/body classification observed at the request setup boundary.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetryCase {
    /// HTTP status.
    status: u16,
    /// Raw body text.
    text: String,
    /// Retry classification.
    expected: bool,
}

#[test]
fn maestro_response_sessions_distinguish_http_failures() {
    let rows: Vec<RetryCase> =
        super::fixture_rows(include_str!("fixtures/retries.json"), &["status", "text"]).unwrap();
    for row in rows {
        assert_eq!(
            super::super::http::is_retryable_error(row.status, &row.text).unwrap(),
            row.expected,
            "{} {:?}",
            row.status,
            row.text
        );
    }
}

/// Error body with a controlled clock and expected friendly selection.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ErrorCase {
    /// HTTP status.
    status: u16,
    /// Whole raw envelope.
    raw: String,
    /// Transport status text.
    status_text: String,
    /// Fixed clock milliseconds.
    now: f64,
    /// Both selected texts, including optional friendly text.
    expected: ErrorTexts,
}

/// The two authored error renderings.
#[derive(Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ErrorTexts {
    /// Server message or raw/status fallback.
    message: String,
    /// Usage-specific friendly form.
    friendly_message: Option<String>,
}

#[test]
fn maestro_response_sessions_render_friendly_http_errors() {
    assert_conditional_clock();
    let rows: Vec<ErrorCase> = super::fixture_rows(
        include_str!("fixtures/errors.json"),
        &["status", "raw", "statusText", "now"],
    )
    .unwrap();
    for row in rows {
        let actual = super::super::http::parse_error_response(
            row.status,
            &row.raw,
            &row.status_text,
            || row.now,
        );
        assert_eq!(
            ErrorTexts {
                message: actual.message,
                friendly_message: actual.friendly_message
            },
            row.expected,
            "{} {:?}",
            row.status,
            row.raw
        );
    }
}

/// Requested and echoed tiers, with null and omission selecting no named tier.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TierCase {
    /// Echoed tier.
    response: Option<String>,
    /// Requested tier.
    request: Option<String>,
    /// Resolved named tier.
    expected: Option<String>,
}

#[test]
fn maestro_response_sessions_resolve_echoed_tier() {
    let rows: Vec<TierCase> = super::fixture_rows(
        include_str!("fixtures/tiers.json"),
        &["response", "request"],
    )
    .unwrap();
    for row in rows {
        assert_eq!(
            super::super::events::resolve_codex_service_tier(
                row.response.as_deref(),
                row.request.as_deref()
            ),
            row.expected
        );
    }
}

/// All cost categories and a deliberately stale total distinguish pricing from a no-op.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PriceCase {
    /// Exact model ID.
    id: String,
    /// Requested pricing tier.
    tier: Option<String>,
    /// Complete input costs.
    #[serde(deserialize_with = "super::consumed")]
    cost: crate::UsageCost,
    /// Independently recorded resulting costs.
    #[serde(deserialize_with = "super::consumed")]
    expected: crate::UsageCost,
}

#[test]
fn maestro_response_sessions_price_all_cost_categories() {
    let rows: Vec<PriceCase> = super::fixture_rows(
        include_str!("fixtures/prices.json"),
        &["id", "tier", "cost"],
    )
    .unwrap();
    for row in rows {
        let mut usage = crate::Usage {
            cost: row.cost,
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 0.0,
        };
        super::super::events::apply_service_tier_pricing(&mut usage, row.tier.as_deref(), &row.id);
        assert_eq!(usage.cost, row.expected);
    }
}

#[test]
fn maestro_response_sessions_validate_headers_before_fetch() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        for (name, value, replaced, expected) in [
            ("Bad Header", "x", false, "Header bad header:"),
            (
                "good",
                "a\nb",
                false,
                "Header good holds NUL or a line break.",
            ),
            (
                "good",
                "Ā",
                false,
                "Header good holds U+0100, which is not a single byte.",
            ),
            (
                "x-test",
                "a\nb",
                true,
                "Header x-test holds NUL or a line break.",
            ),
        ] {
            assert_invalid_header(name, value, replaced, expected).await;
        }
    });
}

/// Observe payload preparation before each rejected header layer.
async fn assert_invalid_header(name: &str, value: &str, replaced: bool, expected: &str) {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let (mut model, context, mut options) = super::invocation();
    Arc::make_mut(&mut model).headers = Some(indexmap::IndexMap::from([(
        name.to_owned(),
        value.to_owned(),
    )]));
    if replaced {
        options.common.headers = Some(indexmap::IndexMap::from([(
            name.to_owned(),
            "good".to_owned(),
        )]));
    }
    let payloads = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&payloads);
    options.common.on_payload = Some(Arc::new(move |payload, _| {
        observed.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(payload) })
    }));
    options.common.fetch = Some(Arc::new(|_| {
        panic!("invalid headers must fail before Fetch")
    }));
    let error =
        super::super::request::prepare_request(&model, &context, &options, "maestro (browser)")
            .await
            .err()
            .unwrap();
    assert!(
        error.diagnostic().message.starts_with(expected),
        "{}",
        error.diagnostic().message
    );
    assert_eq!(payloads.load(Ordering::SeqCst), 1);
}

/// Only changing descriptor fields are recorded; unchanged fields come from one typed fixture.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DescriptorInput {
    /// Model identity when different from the baseline.
    id: Option<String>,
    /// Capability flag when different from the baseline.
    reasoning: Option<bool>,
    /// Descriptor effort mapping, including an empty map.
    thinking_level_map: Option<crate::ThinkingLevelMap>,
}

/// Reject unused descriptor fields before applying the recorded changes.
fn descriptor<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<crate::Model, D::Error> {
    let input = DescriptorInput::deserialize(decoder)?;
    let mut model = super::controlled_model();
    if let Some(id) = input.id {
        model.id = id;
    }
    if let Some(reasoning) = input.reasoning {
        model.reasoning = reasoning;
    }
    model.thinking_level_map = input.thinking_level_map;
    Ok(model)
}

/// Decode the conversation and require every authored member to survive the typed input.
fn conversation<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<crate::Context, D::Error> {
    super::consumed(decoder)
}

/// Load one unique full-query corpus, then let each named witness select its associations.
fn body_cases(case: &str) -> Result<Vec<BodyCase>, serde_json::Error> {
    let text = include_str!("fixtures/requests.json");
    let rows: Vec<BodyCase> = serde_json::from_str(text)?;
    unique_body_queries(text, &rows)?;
    for row in &rows {
        let mut names = std::collections::HashSet::new();
        if row.cases.is_empty()
            || row.cases.iter().any(|name| {
                !matches!(
                    name.as_str(),
                    "defaults" | "history" | "reasoning" | "minimal" | "summary"
                ) || !names.insert(name)
            })
        {
            return Err(<serde_json::Error as serde::de::Error>::custom(
                "unconsumed or repeated fixture association",
            ));
        }
    }
    Ok(rows
        .into_iter()
        .filter(|row| row.cases.iter().any(|name| name == case))
        .collect())
}

/// Exercise each request corpus's real load boundary without repeating transformation assertions.
pub(super) fn audit_fixture_inputs() -> Result<(), serde_json::Error> {
    body_cases("defaults")?;
    super::fixture_rows::<AccountCase>(include_str!("fixtures/account.json"), &["token"])?;
    super::fixture_rows::<EndpointCase>(include_str!("fixtures/endpoints.json"), &["base"])?;
    super::fixture_rows::<HeaderCase>(include_str!("fixtures/headers.json"), &["input"])?;
    super::fixture_rows::<RetryCase>(include_str!("fixtures/retries.json"), &["status", "text"])?;
    super::fixture_rows::<ErrorCase>(
        include_str!("fixtures/errors.json"),
        &["status", "raw", "statusText", "now"],
    )?;
    super::fixture_rows::<TierCase>(
        include_str!("fixtures/tiers.json"),
        &["response", "request"],
    )?;
    super::fixture_rows::<PriceCase>(
        include_str!("fixtures/prices.json"),
        &["id", "tier", "cost"],
    )?;
    Ok(())
}

/// Compare full operation inputs after expanding the compact descriptor/context fields.
fn unique_body_queries(text: &str, rows: &[BodyCase]) -> Result<(), serde_json::Error> {
    use serde::de::Error as _;
    let authored: Vec<serde_json::Value> = serde_json::from_str(text)?;
    let mut queries = std::collections::HashSet::new();
    for (row, input) in rows.iter().zip(authored) {
        let query = serde_json::json!({"model": &row.model, "context": &row.context, "options": input["options"]});
        let key =
            crate::providers::json_text::compact_json(&query).map_err(serde_json::Error::custom)?;
        if !queries.insert(key) {
            return Err(serde_json::Error::custom("duplicate fixture query"));
        }
    }
    Ok(())
}

/// Friendly rendering reads its clock only after a usage code with a nonzero reset.
fn assert_conditional_clock() {
    for raw in [
        r#"{"error":{"code":"other","resets_at":1}}"#,
        r#"{"error":{"code":"usage_limit_reached","resets_at":0}}"#,
    ] {
        super::super::http::parse_error_response(400, raw, "", || {
            panic!("unselected clock must not be read")
        });
    }
    let reads = std::cell::Cell::new(0);
    let result = super::super::http::parse_error_response(
        400,
        r#"{"error":{"code":"usage_limit_reached","resets_at":60}}"#,
        "",
        || {
            reads.set(reads.get() + 1);
            0.0
        },
    );
    assert_eq!(reads.get(), 1);
    assert_eq!(
        result.friendly_message.as_deref(),
        Some("You have hit your ChatGPT usage limit. Try again in ~1 min.")
    );
}
