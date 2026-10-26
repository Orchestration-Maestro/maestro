//! Resource contribution and live command observations through the author facade.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CommandOptions, ExtensionAPI, ExtensionEvent, ExtensionEventResult, ExtensionFuture,
    FlagOptions, FlagType, FlagValue, Presence, ResourcesDiscoverResult, ToolDefinition,
};
use serde_json::json;
use std::{cell::Cell, rc::Rc};

/// Register versioned resources and one independently counted observation command.
pub fn factory(api: ExtensionAPI, version: &'static str) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.register_flag(
            "resource-version",
            FlagOptions {
                description: Some(version.into()),
                type_: FlagType::String,
                default: Some(FlagValue::String(version.into())),
            },
        )?;
        api.register_tool(ToolDefinition {
            render_call: None,
            render_result: None,
            metadata: serde_json::from_value(json!({"name":format!("resource-{version}"),"label":"Resource","description":"Read the resource version","parameters":{"type":"object"}})).map_err(|error| error.to_string())?,
            prepare_arguments: None,
            execute: Rc::new(move |_, _, _, _, _| Box::pin(async move {
                serde_json::from_value(json!({"content":[{"type":"text","text":version}],"details":null})).map_err(|error| error.to_string())
            })),
        })?;
        resources(&api, version)?;
        observe(&api, version)
    })
}

/// Resolve all three resource kinds beside the actual loaded artifact.
fn resources(api: &ExtensionAPI, version: &'static str) -> Result<(), String> {
    api.on(
        "resources_discover",
        Rc::new(move |event, _| {
            Box::pin(async move {
                let ExtensionEvent::ResourcesDiscover(_) = event else {
                    return Err("unexpected resource event".into());
                };
                let location =
                    std::env::var("MAESTRO_EXTENSION_PATH").map_err(|error| error.to_string())?;
                let base = location
                    .strip_suffix("index.wasm")
                    .ok_or("unexpected artifact name")?;
                let path = |name| Presence::Present(vec![format!("{base}{version}/{name}")]);
                Ok(Some(ExtensionEventResult::ResourcesDiscover(
                    ResourcesDiscoverResult {
                        skill_paths: path("SKILL.md"),
                        prompt_paths: path("dynamic.md"),
                        theme_paths: path("dynamic.json"),
                    },
                )))
            })
        }),
    )
}

/// Report current application capabilities through the terminal's existing title operation.
fn observe(api: &ExtensionAPI, version: &'static str) -> Result<(), String> {
    let calls = Rc::new(Cell::new(0_u32));
    let reader = api.clone();
    api.register_command("observe-resources", CommandOptions {
        description: Some(version.into()), get_argument_completions: None,
        handler: Rc::new(move |args, context| {
            let (calls, reader) = (Rc::clone(&calls), reader.clone());
            Box::pin(async move {
                if args == "reload" { return context.reload().await; }
                calls.set(calls.get() + 1);
                let Some(FlagValue::String(flag)) = reader.get_flag("resource-version")? else {
                    return Err("resource flag is not a string".into());
                };
                let tools: Vec<_> = reader.get_all_tools()?.into_iter().map(|tool| tool.name).collect();
                let snapshot = json!({"version":version,"calls":calls.get(),"cwd":context.cwd()?,"commands":reader.get_commands()?,"tools":tools,"thinking":reader.get_thinking_level()?,"flag":flag});
                context.ui()?.set_title(&format!("ROOT-OBSERVE:{snapshot}"))
            })
        }),
    })
}
