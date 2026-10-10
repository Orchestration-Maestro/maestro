//! Response-session behavior witnesses.

mod requests;

mod streams;

#[cfg(not(target_arch = "wasm32"))]
mod sockets;

#[cfg(not(target_arch = "wasm32"))]
mod socket_transport;

#[cfg(not(target_arch = "wasm32"))]
mod loopback;

#[cfg(not(target_arch = "wasm32"))]
mod cache_continuation;

#[cfg(not(target_arch = "wasm32"))]
mod socket_sessions;

#[cfg(not(target_arch = "wasm32"))]
mod socket_debug;

/// Serializes tests that touch session-keyed process state: cached sockets, counters and
/// fallback flags. A test takes it before spawning any peer or task and holds it until they finish.
#[cfg(not(target_arch = "wasm32"))]
fn exclusive() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Build the controlled request descriptor without consulting a catalog or environment.
fn invocation() -> (
    std::sync::Arc<crate::Model>,
    crate::Context,
    super::OpenAICodexResponsesOptions,
) {
    let model = controlled_model();
    let context = controlled_context();
    let options = super::OpenAICodexResponsesOptions {
        common: crate::StreamOptions {
            api_key: Some(fixture_text(
                "a.<fake-account-prefix>dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjX3Rlc3QifX0=.b",
            )),
            ..Default::default()
        },
        ..Default::default()
    };
    (std::sync::Arc::new(model), context, options)
}

/// One descriptor used for unchanged fields in the controlled request corpus.
fn controlled_model() -> crate::Model {
    crate::Model {
        id: "gpt-5.1-codex".to_owned(),
        name: "test".to_owned(),
        api: "openai-codex-responses".to_owned(),
        provider: "openai-codex".to_owned(),
        base_url: "https://chatgpt.com/backend-api".to_owned(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![crate::ModelInput::Text, crate::ModelInput::Image],
        cost: crate::ModelCost {
            input: 1.0,
            output: 2.0,
            cache_read: 3.0,
            cache_write: 4.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// One typed user message supplies the unchanged conversation input.
fn controlled_context() -> crate::Context {
    serde_json::from_value(
        serde_json::json!({"messages":[{"role":"user","content":"hello Ω","timestamp":1}]}),
    )
    .unwrap()
}

/// Reject extra members that typed fixture decoding would otherwise silently discard.
fn consumed_members(input: &serde_json::Value, decoded: &serde_json::Value) -> Result<(), String> {
    match input {
        serde_json::Value::Object(fields) => {
            for (name, value) in fields {
                let retained = decoded
                    .get(name)
                    .ok_or_else(|| format!("unconsumed fixture member: {name}"))?;
                consumed_members(value, retained)?;
            }
        }
        serde_json::Value::Array(values) => {
            let retained = decoded
                .as_array()
                .ok_or_else(|| "unconsumed fixture array".to_owned())?;
            if values.len() != retained.len() {
                return Err("unconsumed fixture array element".to_owned());
            }
            for (value, decoded) in values.iter().zip(retained) {
                consumed_members(value, decoded)?;
            }
        }
        _ => {
            let expected = crate::providers::json_text::compact_json(input)
                .map_err(|error| error.to_string())?;
            let actual = crate::providers::json_text::compact_json(decoded)
                .map_err(|error| error.to_string())?;
            if expected != actual {
                return Err("fixture member changed during decoding".to_owned());
            }
        }
    }
    Ok(())
}

/// A plain full-query uniqueness check runs before the actual fixture consumer.
fn unique_queries(text: &str, fields: &[&str]) -> Result<(), serde_json::Error> {
    use serde::de::Error as _;
    let rows: Vec<serde_json::Value> = serde_json::from_str(text)?;
    let mut queries = std::collections::HashSet::new();
    for row in rows {
        let mut query = serde_json::Map::new();
        for name in fields {
            if let Some(value) = row.get(*name) {
                query.insert((*name).to_owned(), value.clone());
            }
        }
        let query = crate::providers::json_text::compact_json(&serde_json::Value::Object(query))
            .map_err(serde_json::Error::custom)?;
        if !queries.insert(query) {
            return Err(serde_json::Error::custom("duplicate fixture query"));
        }
    }
    Ok(())
}

/// Decode a typed fixture boundary without silently discarding supplied nested members.
fn consumed<'de, T, D>(decoder: D) -> Result<T, D::Error>
where
    T: serde::de::DeserializeOwned + serde::Serialize,
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;
    let input = serde_json::Value::deserialize(decoder)?;
    let record: T = serde_json::from_value(input.clone()).map_err(serde::de::Error::custom)?;
    let retained = serde_json::to_value(&record).map_err(serde::de::Error::custom)?;
    consumed_members(&input, &retained).map_err(serde::de::Error::custom)?;
    Ok(record)
}

/// Load-time checking belongs to the same decoder used by the behavior witnesses.
fn fixture_rows<T: serde::de::DeserializeOwned>(
    text: &str,
    query: &[&str],
) -> Result<Vec<T>, serde_json::Error> {
    let text = fixture_text(text);
    unique_queries(&text, query)?;
    serde_json::from_str(&text)
}

/// Reconstruct synthetic account strings using the owning account fixture encoder.
fn fixture_text(text: &str) -> String {
    let prefix = crate::oauth::responses::openai_codex::tests::account_fixture_prefix();
    text.replace("<fake-account-prefix>", &prefix)
}
