//! Author provider registrations exercising every optional record field.
use indexmap::IndexMap;
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionResult, ModelCompat, ModelCost, ModelInput, ModelThinkingLevel,
    ProviderConfig, ProviderModelConfig,
};
use serde_json::json;

#[path = "../../examples/context_bridge/credentials/state.rs"]
mod credential_state;
#[path = "../../examples/context_bridge/providers.rs"]
mod providers;

/// Runs one named provider scenario.
pub fn dispatch(api: &ExtensionAPI, args: &str) -> Option<ExtensionResult<()>> {
    match args {
        "provider-shared-register" => Some(register_shared(api)),
        "provider records" => Some(records(api)),
        "provider nonfinite" => Some(nonfinite(api)),
        _ => None,
    }
}

/// Registers absent, empty and fully supplied provider records.
fn records(api: &ExtensionAPI) -> ExtensionResult<()> {
    api.register_provider("absent", ProviderConfig::default())?;
    api.register_provider(
        "empty",
        ProviderConfig {
            name: Some(String::new()),
            base_url: Some(String::new()),
            api_key: Some(String::new()),
            api: Some(String::new()),
            headers: Some(IndexMap::new()),
            auth_header: Some(false),
            models: Some(Vec::new()),
            ..ProviderConfig::default()
        },
    )?;
    let headers = IndexMap::from([
        ("z".to_owned(), "1".to_owned()),
        ("a".to_owned(), String::new()),
    ]);
    let mut first = model("first", vec![]);
    first.api = Some("openai-responses".into());
    first.base_url = Some(String::new());
    first.headers = Some(headers.clone());
    first.thinking_level_map = Some(IndexMap::from([
        (ModelThinkingLevel::High, None),
        (ModelThinkingLevel::Low, Some(String::new())),
    ]));
    first.compat = Some(ModelCompat(
        json!({"open": {"nested": [1, null]}})
            .as_object()
            .cloned()
            .unwrap_or_default(),
    ));
    let repeated = model(
        "repeated",
        vec![ModelInput::Text, ModelInput::Image, ModelInput::Text],
    );
    api.register_provider(
        "full",
        ProviderConfig {
            name: Some(" Full ".into()),
            base_url: Some("https://full.invalid".into()),
            api_key: Some("KEY".into()),
            api: Some("openai-completions".into()),
            headers: Some(headers),
            auth_header: Some(true),
            models: Some(vec![first, repeated]),
            ..ProviderConfig::default()
        },
    )
}

/// Reports the error of a nonfinite authored number, then removes a provider.
fn nonfinite(api: &ExtensionAPI) -> ExtensionResult<()> {
    let mut invalid = model("invalid", vec![ModelInput::Text]);
    invalid.cost.output = f64::INFINITY;
    let config = ProviderConfig {
        models: Some(vec![invalid]),
        ..ProviderConfig::default()
    };
    let error = api.register_provider("nonfinite", config).err();
    api.append_entry("provider-error", Some(json!(error)))?;
    api.unregister_provider("absent")
}

/// One model whose numbers cover zero, negative, fractional and signed-zero values.
fn model(id: &str, input: Vec<ModelInput>) -> ProviderModelConfig {
    ProviderModelConfig {
        id: id.into(),
        name: format!("{id} name"),
        api: None,
        base_url: None,
        reasoning: false,
        thinking_level_map: None,
        input,
        cost: ModelCost {
            input: 0.0,
            output: -1.5,
            cache_read: 0.25,
            cache_write: -0.0,
        },
        context_window: 128_000.0,
        max_tokens: -0.0,
        headers: None,
        compat: None,
    }
}

/// Installs the same author observers that run through the production component loader.
fn register_shared(api: &ExtensionAPI) -> ExtensionResult<()> {
    use maestro_extensions_wasm::CommandOptions;
    use std::rc::Rc;
    providers::factory(api)?;
    let host = api.clone();
    api.register_command(
        "provider-shared",
        CommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |args, context| {
                let api = host.clone();
                Box::pin(async move {
                    let input: serde_json::Value =
                        serde_json::from_str(&args).map_err(|error| error.to_string())?;
                    if let Some(control) = input.get("control") {
                        api.append_entry("catalog-control", Some(control.clone()))?;
                    }
                    if let Some(control) = input.get("readers") {
                        api.append_entry("reader-control", Some(control.clone()))?;
                    }
                    let request = input["request"].as_str().ok_or("missing shared request")?;
                    let answer = if let Some(request) = request.strip_prefix("credential callbacks")
                    {
                        credential_state::composition(&api, request.trim_start()).await
                    } else if request == "credential state" {
                        credential_state::control(&api).await
                    } else {
                        providers::command(&api, &context, request).await
                    };
                    api.append_entry(
                        "provider-answer",
                        Some(match answer {
                            Ok(value) => json!({"ok":value}),
                            Err(error) => json!({"error":error}),
                        }),
                    )
                })
            }),
        },
    )
}
