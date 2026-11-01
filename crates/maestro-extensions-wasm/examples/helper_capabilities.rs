//! An author fixture that observes helpers through the installed host.
// maestro-component: MAESTRO_HELPER_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{CommandOptions, ExtensionAPI, ExtensionFuture, ExtensionResult};
use serde_json::Value;
use std::rc::Rc;
#[path = "helper_capabilities/conversation.rs"]
mod conversation;
#[path = "helper_capabilities/frontmatter.rs"]
mod frontmatter;
#[path = "helper_capabilities/fuzzy.rs"]
mod fuzzy;
#[path = "helper_capabilities/keys.rs"]
mod keys;
#[path = "helper_capabilities/paths.rs"]
mod paths;
#[path = "helper_capabilities/text.rs"]
mod text;
#[path = "helper_capabilities/truncate.rs"]
mod truncate;

/// Registers one observation command without choosing a native adapter.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let helpers = api.clone();
        api.register_command(
            "helpers",
            CommandOptions {
                description: None,
                get_argument_completions: None,
                handler: Rc::new(move |request, _| {
                    let api = helpers.clone();
                    Box::pin(async move {
                        let request: Value =
                            serde_json::from_str(&request).map_err(|e| e.to_string())?;
                        let value = observe(&api, &request)?;
                        Err(value.to_string())
                    })
                }),
            },
        )
    })
}

/// Returns the complete observation as the command's catchable result.
fn observe(api: &ExtensionAPI, request: &Value) -> ExtensionResult<Value> {
    if request["operation"]
        .as_str()
        .is_some_and(|op| op.starts_with("k."))
    {
        return keys::observe(api, request);
    }
    if request["operation"]
        .as_str()
        .is_some_and(|op| op.starts_with("f."))
    {
        return fuzzy::observe(api, request);
    }
    if request["operation"]
        .as_str()
        .is_some_and(|op| op.starts_with("fm."))
    {
        return frontmatter::observe(api, request);
    }
    if request["operation"]
        .as_str()
        .is_some_and(|op| op.starts_with("tr."))
    {
        return truncate::observe(api, request);
    }
    if request["operation"]
        .as_str()
        .is_some_and(|op| op.starts_with("p."))
    {
        return paths::observe(api, request);
    }
    if request["operation"]
        .as_str()
        .is_some_and(|op| op.starts_with("msg.") || op.starts_with("co."))
    {
        return conversation::observe(api, request);
    }
    if request["operation"] == "config.getAgentDir" {
        return Ok(serde_json::json!(
            maestro_extensions_wasm::config::get_agent_dir(api)?
        ));
    }
    if request["operation"] == "config.VERSION" {
        return Ok(serde_json::json!(maestro_extensions_wasm::config::version(
            api
        )?));
    }
    if request["operation"] == "composition" {
        return composed(api, request);
    }
    let args = &request["args"];
    text::observe(api, request["operation"].as_str().unwrap_or_default(), args)
}

/// Reads a supplied cell count without narrowing it silently.
fn number(value: &Value) -> ExtensionResult<usize> {
    usize::try_from(value.as_u64().ok_or("missing cell count")?).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

/// Exercises every helper family through this fixture's one installed API.
fn composed(api: &ExtensionAPI, request: &Value) -> ExtensionResult<Value> {
    let mut result = text::observe(api, "composition", &request["args"])?;
    let fields = result.as_object_mut().ok_or("missing composition record")?;
    fields.insert(
        "background".into(),
        text::observe(api, "u.applyBackgroundToLine", &serde_json::json!(["x", 4]))?,
    );
    fields.insert(
        "fuzzy".into(),
        fuzzy::observe(
            api,
            &serde_json::json!({"operation":"f.fuzzyFilter","args":[""]}),
        )?,
    );
    fields.insert("metadata".into(), frontmatter::observe(api, &serde_json::json!({"operation":"fm.parseFrontmatter","args":["---\nx: .nan\ny: .inf\nz: -.inf\n---\nbody"]}))?);
    let cwd = request["cwd"]
        .as_str()
        .ok_or("missing composition directory")?;
    fields.insert(
        "path".into(),
        paths::observe(
            api,
            &serde_json::json!({"operation":"p.resolveReadPath","args":["café.txt",cwd]}),
        )?,
    );
    fields.insert(
        "key".into(),
        serde_json::json!(maestro_extensions_wasm::Key::ctrl(api, "x")?),
    );
    fields.insert(
        "version".into(),
        serde_json::json!(maestro_extensions_wasm::config::version(api)?),
    );
    composed_content(api, fields)?;
    Ok(result)
}

/// Observes transcript and output helpers within the same composition.
fn composed_content(
    api: &ExtensionAPI,
    fields: &mut serde_json::Map<String, Value>,
) -> ExtensionResult<()> {
    let messages = serde_json::json!([{"role":"user","content":"hello","timestamp":1.0}]);
    fields.insert(
        "transcript".into(),
        conversation::observe(
            api,
            &serde_json::json!({"operation":"co.serializeConversation","args":[messages]}),
        )?,
    );
    fields.insert(
        "messages".into(),
        conversation::observe(
            api,
            &serde_json::json!({"operation":"msg.convertToLlm","args":[messages]}),
        )?,
    );
    fields.insert(
        "truncation".into(),
        truncate::observe(
            api,
            &serde_json::json!({"operation":"tr.truncateHead","args":["",{}]}),
        )?,
    );
    fields.insert(
        "wrapped".into(),
        text::observe(api, "u.wrapTextWithAnsi", &serde_json::json!(["abcdef", 3]))?,
    );
    fields.insert(
        "slice".into(),
        text::observe(
            api,
            "u.sliceWithWidth",
            &serde_json::json!(["abc", 1, 1, false]),
        )?,
    );
    Ok(())
}
