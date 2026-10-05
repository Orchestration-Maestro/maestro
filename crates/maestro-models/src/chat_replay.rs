//! Wire replay of already-projected messages.
use crate::*;
use serde_json::{Value, json};
pub(crate) fn messages(model: &Model, context: &Context, d: &ChatDialect) -> Vec<Value> {
    let mut messages = Vec::new();
    if let Some(prompt) = &context.system_prompt {
        messages.push(json!({"role":if d.developer_role && model.capabilities.reasoning{"developer"}else{"system"},"content":prompt}));
    }
    let mut images = Vec::new();
    for (position, message) in context.messages.iter().enumerate() {
        match message {
            Message::User(user) => {
                let parts=user.content.iter().map(|p|match p{InputContent::Text(t)=>json!({"type":"text","text":t.text}),InputContent::Image(i)=>json!({"type":"image_url","image_url":{"url":format!("data:{};base64,{}",i.mime_type,i.data)}})}).collect::<Vec<_>>();
                if !parts.is_empty() {
                    messages.push(json!({"role":"user","content":parts}));
                }
            }
            Message::Assistant(a) => {
                let mut parts = Vec::new();
                let mut text = String::new();
                let mut thinking = Vec::new();
                let mut signature = None;
                let mut tools = Vec::new();
                let mut details = Vec::new();
                for c in &a.content {
                    match c {
                        AssistantContent::Text(t) if !crate::scalar::trim(&t.text).is_empty() => {
                            text.push_str(&t.text);
                            parts.push(json!({"type":"text","text":t.text}));
                        }
                        AssistantContent::Thinking(ThinkingContent::Readable {
                            text,
                            signature: s,
                        }) if !crate::scalar::trim(text).is_empty() => {
                            thinking.push(text.clone());
                            if signature.is_none() {
                                signature = s.clone().filter(|s| !s.is_empty());
                            }
                        }
                        AssistantContent::ToolCall(t) => {
                            tools.push(json!({"id":t.id,"type":"function","function":{"name":t.name,"arguments":t.arguments().map(|a|crate::chat_json::compact(&Value::Object(a.clone()))).unwrap_or_else(||"{}".into())}}));
                            if let Some(raw) = &t.replay_metadata
                                && let Ok(value) = serde_json::from_str::<Value>(raw)
                            {
                                details.push(value);
                            }
                        }
                        _ => {}
                    }
                }
                let has_thinking = !thinking.is_empty();
                if d.thinking_as_text && has_thinking {
                    parts.insert(0, json!({"type":"text","text":thinking.join("\n\n")}));
                }
                let mut out = json!({"role":"assistant","content":if d.thinking_as_text && has_thinking{Value::Array(parts)}else if text.is_empty(){if d.assistant_after_tool_result{json!("")}else{Value::Null}}else{json!(text)}});
                if !d.thinking_as_text
                    && let Some(s) = signature
                {
                    out[s] = json!(thinking.join("\n"));
                }
                if !tools.is_empty() {
                    out["tool_calls"] = json!(tools);
                }
                if !details.is_empty() {
                    out["reasoning_details"] = json!(details);
                }
                if d.empty_reasoning_content
                    && model.capabilities.reasoning
                    && out.get("reasoning_content").is_none()
                {
                    out["reasoning_content"] = json!("");
                }
                if !text.is_empty()
                    || out["content"].as_array().is_some_and(|a| !a.is_empty())
                    || out.get("tool_calls").is_some()
                {
                    messages.push(out);
                }
            }
            Message::ToolResult(result) => {
                let text=result.content.iter().filter_map(|p|match p{InputContent::Text(t)=>Some(t.text.as_str()),InputContent::Image(i)=>{images.push(json!({"type":"image_url","image_url":{"url":format!("data:{};base64,{}",i.mime_type,i.data)}}));None}}).collect::<Vec<_>>().join("\n");
                let mut out = json!({"role":"tool","tool_call_id":result.tool_call_id,"content":if text.is_empty(){"(see attached image)"}else{&text}});
                if d.tool_result_name && !result.tool_name.is_empty() {
                    out["name"] = json!(result.tool_name);
                }
                messages.push(out);
                if !matches!(
                    context.messages.get(position + 1),
                    Some(Message::ToolResult(_))
                ) {
                    if d.assistant_after_tool_result
                        && (!images.is_empty()
                            || matches!(context.messages.get(position + 1), Some(Message::User(_))))
                    {
                        messages.push(json!({"role":"assistant","content":"I have processed the tool results."}));
                    }
                    if !images.is_empty() {
                        let mut parts = vec![
                            json!({"type":"text","text":"Attached image(s) from tool result:"}),
                        ];
                        parts.append(&mut images);
                        messages.push(json!({"role":"user","content":parts}));
                    }
                }
            }
        }
    }
    messages
}
