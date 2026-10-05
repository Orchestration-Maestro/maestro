//! Encode already-projected request data once.
use crate::*;
use serde_json::{Value, json};
pub(crate) fn encode(
    model: &Model,
    context: &Context,
    options: &ProviderOptions,
    dialect: &ChatDialect,
) -> Result<ChatHttpRequest, Failure> {
    if options.thinking_budget.is_some() {
        return Err(Failure::UnsupportedOperation);
    }
    let messages = crate::chat_replay::messages(model, context, dialect);
    let mut payload = json!({"model":model.identity.model,"messages":messages,"stream":true});
    if !context.tools.is_empty() || context.messages.iter().any(|m|matches!(m,Message::ToolResult(_)) || matches!(m,Message::Assistant(a) if a.content.iter().any(|c|matches!(c,AssistantContent::ToolCall(_))))) {
 payload["tools"]=Value::Array(context.tools.iter().map(|t|{let mut function=json!({"name":t.name,"description":t.description,"parameters":t.parameters});if dialect.strict_tools{function["strict"]=json!(false);} json!({"type":"function","function":function})}).collect());
 }
    if model.capabilities.reasoning
        && let Some(format) = dialect.thinking_format
    {
        let enabled = options.thinking != ThinkingLevel::Off;
        match format {
            ChatThinkingFormat::Effort => {
                if let Some(e) = &options.effort {
                    payload["reasoning_effort"] = json!(e);
                }
            }
            ChatThinkingFormat::NestedEffort => {
                if let Some(e) = &options.effort {
                    payload["reasoning"] = json!({"effort":e});
                } else if options.thinking == ThinkingLevel::Off
                    && !matches!(
                        model
                            .capabilities
                            .thinking_level_map
                            .get(&ThinkingLevel::Off),
                        Some(None)
                    )
                {
                    payload["reasoning"] = json!({"effort":"none"});
                }
            }
            ChatThinkingFormat::Toggle => payload["enable_thinking"] = json!(enabled),
            ChatThinkingFormat::TemplateToggle => {
                payload["chat_template_kwargs"] =
                    json!({"enable_thinking":enabled,"preserve_thinking":true})
            }
            ChatThinkingFormat::TypedToggle => {
                payload["thinking"] = json!({"type":if enabled{"enabled"}else{"disabled"}});
                if enabled && let Some(e) = &options.effort {
                    payload["reasoning_effort"] = json!(e);
                }
            }
        }
    }
    if let Some(choice) = &options.tool_choice {
        payload["tool_choice"] = match choice {
            ToolChoice::Auto => json!("auto"),
            ToolChoice::None => json!("none"),
            ToolChoice::Required => json!("required"),
            ToolChoice::Function { name } => json!({"type":"function","function":{"name":name}}),
        };
    }
    if let Some(p) = &dialect.provider_routing {
        payload["provider"] = json!(p);
    }
    if let Some(p) = &dialect.provider_options {
        payload["providerOptions"] = json!(p);
    }
    let cache = matches!(options.cache_preference.as_deref(), Some("short" | "long"));
    let long = options.cache_preference.as_deref() == Some("long") && dialect.long_cache_retention;
    if cache
        && (dialect.prompt_cache_key || long)
        && let Some(session) = options.session_affinity.as_ref().filter(|s| !s.is_empty())
    {
        payload["prompt_cache_key"] = json!(session);
    }
    if cache && dialect.cache_control.is_some() {
        let marker = if long {
            json!({"type":"ephemeral","ttl":"1h"})
        } else {
            json!({"type":"ephemeral"})
        };
        if let Some(messages) = payload["messages"].as_array_mut() {
            if let Some(instruction) = messages
                .first_mut()
                .filter(|m| matches!(m["role"].as_str(), Some("system" | "developer")))
                && let Some(text) = instruction["content"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(String::from)
            {
                instruction["content"] =
                    json!([{"type":"text","text":text,"cache_control":marker}]);
            }
            for message in messages.iter_mut().rev() {
                if !matches!(message["role"].as_str(), Some("user" | "assistant")) {
                    continue;
                }
                if let Some(text) = message["content"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                {
                    message["content"] = json!([{"type":"text","text":text}]);
                }
                if let Some(parts) = message["content"].as_array_mut()
                    && let Some(part) = parts.iter_mut().rev().find(|p| {
                        p["type"] == "text" && p["text"].as_str().is_some_and(|s| !s.is_empty())
                    })
                {
                    part["cache_control"] = marker.clone();
                    break;
                }
            }
        }
        if let Some(tool) = payload["tools"].as_array_mut().and_then(|a| a.last_mut()) {
            tool["cache_control"] = marker;
        }
    }
    if long {
        payload["prompt_cache_retention"] = json!("24h");
    }
    let mut generated =
        std::collections::BTreeMap::from([("content-type".into(), "application/json".into())]);
    if cache
        && dialect.session_affinity_headers
        && let Some(session) = options.session_affinity.as_ref().filter(|s| !s.is_empty())
    {
        for name in ["session_id", "x-client-request-id", "x-session-affinity"] {
            generated.insert(name.into(), session.clone());
        }
    }
    if let RequestAuth::Secret { secret, .. } = &options.auth {
        generated.insert(
            dialect.auth_header.clone(),
            format!("{}{}", dialect.auth_prefix, secret.expose()),
        );
    }
    if dialect.store {
        payload["store"] = json!(false);
    }
    if dialect.usage_in_stream {
        payload["stream_options"] = json!({"include_usage":true});
    }
    if let Some(output) = options.output_limit {
        payload[match dialect.output_field {
            ChatOutputField::MaxTokens => "max_tokens",
            ChatOutputField::MaxCompletionTokens => "max_completion_tokens",
        }] = json!(output);
    }
    if let Some(t) = options.temperature {
        payload["temperature"] = json!(t);
    }
    if dialect.tool_stream && !context.tools.is_empty() {
        payload["tool_stream"] = json!(true);
    }
    let joined = format!(
        "{}/chat/completions",
        model.endpoint.strip_suffix('/').unwrap_or(&model.endpoint)
    );
    let url = reqwest::Url::parse(&joined).map_err(|_| Failure::AdapterFailed)?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(Failure::UnsupportedOperation);
    }
    Ok(ChatHttpRequest {
        url: url.to_string(),
        headers: crate::dispatch::headers(&[&generated, &options.headers])?,
        body: crate::chat_json::compact(&payload).into_bytes(),
    })
}
