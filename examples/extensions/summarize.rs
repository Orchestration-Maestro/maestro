//! An opt-in conversation summary shown in a custom Markdown view.
// maestro-component: MAESTRO_SUMMARIZE_COMPONENT
#![cfg(target_arch = "wasm32")]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    AssistantContent, CallbackEmitter, CommandOptions, Component, ComponentView, Container,
    Context, CustomFactoryOutcome, DynamicBorder, ExtensionAPI, ExtensionCommandContext,
    ExtensionComponent, ExtensionFuture, ExtensionResult, Markdown, Message, NotificationType,
    ProviderStreamOptions, ResolvedRequestAuth, SessionEntry, StreamOptions, Style, Text,
    TextContent, UserBlock, UserContent, UserMessage, complete, get_markdown_theme, matches_key,
};
use serde_json::Value;
use std::{
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

/// Trims only the whitespace scalars recognized by the host's shared text owner.
fn trim<'a>(api: &ExtensionAPI, mut text: &'a str) -> ExtensionResult<&'a str> {
    while let Some(c) = text.chars().next() {
        if !maestro_extensions_wasm::utils::is_whitespace_char(api, &c.to_string())? {
            break;
        }
        text = &text[c.len_utf8()..];
    }
    while let Some(c) = text.chars().next_back() {
        if !maestro_extensions_wasm::utils::is_whitespace_char(api, &c.to_string())? {
            break;
        }
        text = &text[..text.len() - c.len_utf8()];
    }
    Ok(text)
}

/// Extracts user and assistant text, then assistant tool calls, in branch order.
fn conversation(api: &ExtensionAPI, entries: Vec<SessionEntry>) -> ExtensionResult<String> {
    let mut sections = Vec::new();
    for entry in entries {
        let entry = serde_json::to_value(entry).map_err(|error| error.to_string())?;
        if entry["type"] != "message" {
            continue;
        }
        let message = &entry["message"];
        let label = match message["role"].as_str() {
            Some("user") => "User",
            Some("assistant") => "Assistant",
            _ => continue,
        };
        let content = &message["content"];
        let blocks = content.as_array().map_or(&[][..], Vec::as_slice);
        let text = content.as_str().map_or_else(
            || {
                blocks
                    .iter()
                    .filter(|block| block["type"] == "text")
                    .filter_map(|block| block["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            str::to_owned,
        );
        let text = trim(api, &text)?;
        let mut lines = Vec::new();
        if !text.is_empty() {
            lines.push(format!("{label}: {text}"));
        }
        if label == "Assistant" {
            lines.extend(tool_lines(blocks)?);
        }
        if !lines.is_empty() {
            sections.push(lines.join("\n"));
        }
    }
    Ok(sections.join("\n\n"))
}

/// Formats tool arguments with the shared canonical JSON writer.
fn tool_lines(blocks: &[Value]) -> ExtensionResult<Vec<String>> {
    blocks
        .iter()
        .filter(|block| block["type"] == "toolCall")
        .filter_map(|block| {
            block["name"]
                .as_str()
                .map(|name| (name, &block["arguments"]))
        })
        .map(|(name, args)| {
            let empty = serde_json::json!({});
            let args = maestro_json::compact_json(if args.is_null() { &empty } else { args })
                .map_err(|error| error.to_string())?;
            Ok(format!("Tool {name} was called with args {args}"))
        })
        .collect()
}

/// Reports a notification only while an interactive UI is available.
fn notify(
    ctx: &ExtensionCommandContext,
    text: &str,
    kind: NotificationType,
) -> ExtensionResult<()> {
    if ctx.has_ui()? {
        ctx.ui()?.notify(text, Some(kind))?;
    }
    Ok(())
}

/// The retained host container, closed only by Enter or Escape.
struct SummaryView {
    /// Host widgets retain their own rendering and invalidation.
    container: Container,
    /// Host helper used to recognize terminal keys.
    api: ExtensionAPI,
    /// Completes the custom view.
    done: Rc<dyn Fn(()) -> ExtensionResult<()>>,
}
impl ComponentView for SummaryView {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.container.render(width)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.container.invalidate()
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        if matches_key(&self.api, data, "enter")? || matches_key(&self.api, data, "escape")? {
            (self.done)(())?;
        }
        Ok(())
    }
}

/// Shows the host's Markdown widget without emitting headless output.
async fn show(
    api: ExtensionAPI,
    ctx: ExtensionCommandContext,
    summary: String,
) -> ExtensionResult<()> {
    if !ctx.has_ui()? {
        return Ok(());
    }
    ctx.ui()?
        .custom(
            Rc::new(move |_, theme, _, done, _| {
                let container = Container::new()?;
                let border_theme = theme.clone();
                let border = DynamicBorder::new(Some(&Style::new(move |text| {
                    border_theme.fg("accent", text)
                })))?;
                container.add_child(&Component::from(&border))?;
                let title = Text::new(
                    &theme.fg("accent", &theme.bold("Conversation Summary")?)?,
                    1,
                    0,
                    None,
                )?;
                container.add_child(&Component::from(&title))?;
                let markdown = Markdown::new(&summary, 1, 1, &get_markdown_theme()?, None)?;
                container.add_child(&Component::from(&markdown))?;
                let hint = Text::new(&theme.fg("dim", "Press Enter or Esc to close")?, 1, 0, None)?;
                container.add_child(&Component::from(&hint))?;
                container.add_child(&Component::from(&border))?;
                Ok(CustomFactoryOutcome::Ready(ExtensionComponent {
                    component: Component::new(SummaryView {
                        container,
                        api: api.clone(),
                        done,
                    }),
                    dispose: None,
                    set_expanded: None,
                    width: None,
                }))
            }),
            None,
        )
        .await?;
    Ok(())
}

/// Reads the selected branch, authenticates the fixed model and completes one summary request.
async fn summarize(api: ExtensionAPI, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    let text = conversation(&api, ctx.session_manager()?.get_branch(None)?)?;
    if text.is_empty() {
        return notify(
            &ctx,
            "No conversation text found",
            NotificationType::Warning,
        );
    }
    notify(&ctx, "Preparing summary...", NotificationType::Info)?;
    let registry = ctx.model_registry()?;
    let Some(model) = registry.find("openai", "gpt-5.2")? else {
        return notify(
            &ctx,
            "Model openai/gpt-5.2 not found",
            NotificationType::Warning,
        );
    };
    let (api_key, headers, base_url) = match registry.get_api_key_and_headers(model.clone()).await?
    {
        ResolvedRequestAuth::Error { error } => {
            return notify(&ctx, &error, NotificationType::Warning);
        }
        ResolvedRequestAuth::Ok {
            api_key,
            headers,
            base_url,
        } => (api_key, headers, base_url),
    };
    let Some(api_key) = api_key.filter(|key| !key.is_empty()) else {
        return notify(
            &ctx,
            "No API key for openai/gpt-5.2",
            NotificationType::Warning,
        );
    };
    let summary = generate(
        model,
        &text,
        ProviderStreamOptions {
            common: StreamOptions {
                api_key: Some(api_key),
                base_url,
                headers,
                ..Default::default()
            },
            extra: serde_json::Map::from_iter([(
                "reasoningEffort".into(),
                Value::String("high".into()),
            )]),
            objects: None,
        },
    )
    .await?;
    show(api, ctx, summary).await
}

/// Sends the authored prompt through the host's registered raw completion provider.
async fn generate(
    model: maestro_extensions_wasm::Model,
    text: &str,
    options: ProviderStreamOptions,
) -> ExtensionResult<String> {
    let timestamp = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs_f64()
        * 1000.0)
        .floor();
    let prompt = format!(
        "Summarize this conversation so I can resume it later.\nInclude goals, key decisions, progress, open questions, and next steps.\nKeep it concise and structured with headings.\n\n<conversation>\n{text}\n</conversation>"
    );
    let response = complete(
        model,
        Context {
            system_prompt: None,
            messages: vec![Message::User(UserMessage {
                content: UserContent::Blocks(vec![UserBlock::Text(TextContent {
                    text: prompt,
                    text_signature: None,
                })]),
                timestamp,
            })],
            tools: None,
        },
        Some(options),
    )
    .await?;
    let summary = response
        .read()
        .map_err(|error| error.to_string())?
        .content
        .iter()
        .filter_map(|block| match block {
            AssistantContent::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(summary)
}

/// Registers `/summarize` on the supplied extension host.
///
/// # Errors
/// Returns the host's registration rejection unchanged.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let registered = api.clone();
        api.register_command(
            "summarize",
            CommandOptions {
                description: Some("Summarize the current conversation in a custom UI".into()),
                get_argument_completions: None,
                handler: Rc::new(move |_, ctx| Box::pin(summarize(registered.clone(), ctx))),
            },
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
