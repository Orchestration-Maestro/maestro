//! Shared author functions cross both controlled transports before native qualification.
use super::scenario::Driver;
use serde_json::{Value, json};

/// A reply selected by the actual method and operands, consumed once when requested.
fn reply(method: &str, args: Value, value: Value) -> Value {
    Value::Object(
        [
            ("method".into(), json!(method)),
            ("args".into(), args),
            ("value".into(), value),
            ("once".into(), json!(true)),
        ]
        .into_iter()
        .collect(),
    )
}

/// Calls the author command after applying an acknowledged host control.
async fn ask(driver: &mut impl Driver, request: &str, control: Value) -> Result<Value, String> {
    let mut input = control;
    input["request"] = json!(request);
    driver
        .run_plain_command("provider-shared", &input.to_string())
        .await?;
    answer(driver)
}

/// Complete descriptors returned by the controlled catalog, not a second catalog algorithm.
fn model(id: &str) -> Value {
    json!({"id":id,"name":id,"provider":"author-provider","api":"openai-completions",
        "baseUrl":"https://provider.invalid","reasoning":false,"input":["text"],
        "cost":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0},"contextWindow":128.0,"maxTokens":16.0})
}

/// Exact catalog replies follow the same author calls used by the production host.
async fn providers(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command("capture", "provider-shared-register")
        .await?;
    let replies = provider_replies()?;
    assert_eq!(
        ask(
            driver,
            "provider catalog",
            json!({"control":{"registry":{"replies":replies}}})
        )
        .await?,
        json!({"ok":{
            "label":" Literal provider ","key":"literal-key","configured":true,"status":{"configured":true,"source":"models_json_key"},
            "oauth":false,"queued":["factory"],"before":["first"],"replaced":["second"],"refreshed":["second"],"removed":true,"error":null,
            "auth":{"ok":true,"apiKey":"literal-key","headers":{"x-test":"literal","Authorization":"Bearer literal-key"}}
        }})
    );
    let registrations = driver
        .transcript()
        .into_iter()
        .filter(|line| line.starts_with("provider "))
        .collect::<Vec<_>>();
    assert_eq!(
        registrations,
        [
            format!(
                "provider register author-provider {}",
                provider_config("first")
            ),
            "provider unregister author-provider".into()
        ]
    );
    Ok(())
}

#[test]
fn provider_author_component_uses_native_catalog_without_adapter_changes() -> Result<(), String> {
    on_both_adapters!(providers)
}

/// The same retained credential observer runs after its creating callback returns.
async fn credentials(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command("capture", "provider-shared-register")
        .await?;
    let stored = json!({"type":"api_key","key":"!checkpoint-credential"});
    let replies = credential_replies();
    assert_eq!(
        ask(
            driver,
            "credential owner",
            json!({"control":{"registry":{"replies":replies}}})
        )
        .await?,
        json!({"ok":{"key":"original effect key","stored":stored,"listed":["author-credential"],"has":true,"auth":true,"status":{"configured":true,"source":"stored"},"errors":[]}})
    );
    assert_eq!(
        ask(driver, "credential retained", json!({})).await?,
        json!({"ok":"original effect key"})
    );
    let replies = constructor_replies();
    assert_eq!(
        ask(
            driver,
            "credential constructors",
            json!({"readers":{"bind":{"replies":replies}}})
        )
        .await?,
        json!({"ok":{"key":"changed-after-construction","alias":"changed-after-construction","errors":null,"stored":[]}})
    );
    assert_eq!(
        ask(driver, "credential release", json!({})).await?,
        json!({"ok":"released"})
    );
    callbacks(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}

#[test]
fn credential_author_retains_callable_native_owner() -> Result<(), String> {
    on_both_adapters!(credentials)
}

/// Backend failure recovery and synchronous fallback values come from the author's callbacks.
async fn callbacks(driver: &mut impl Driver) -> Result<(), String> {
    let replies = callback_replies();
    assert_eq!(
        ask(
            driver,
            "credential callbacks",
            json!({"readers":{"bind":{"replies":replies}}})
        )
        .await?,
        json!({"ok":{
            "keys":["changed-after-construction","accepted","original"],"errors":["write-first"],"drained":[],
            "raw":r#"{"shared":{"type":"api_key","key":"original"}}"#,
            "fallback":{"has":true,"status":{"configured":false,"source":"fallback","label":"custom provider config"},"key":"fallback-key"}
        }})
    );
    assert_eq!(
        ask(driver, "credential callbacks retained", json!({})).await?,
        json!({"ok":"later-key"})
    );
    driver
        .command_held(
            "provider-shared",
            &json!({"request":"credential callbacks pending"}).to_string(),
        )
        .await?;
    assert_eq!(
        answer(driver)?,
        json!({"ok":{"key":"later-key","captures":[]}})
    );
    assert_eq!(
        ask(driver, "credential callbacks release", json!({})).await?,
        json!({"ok":"released"})
    );
    assert!(driver.release("credential-backend").await);
    assert!(driver.release("credential-fallback").await);
    assert_eq!(
        ask(driver, "credential callbacks counts", json!({})).await?,
        json!({"ok":["backend","fallback"]})
    );
    Ok(())
}

/// Ordered replies for the complete provider observer.
fn provider_replies() -> Result<Value, String> {
    let first = model("first");
    let wire_model = serde_json::to_string(
        &serde_json::from_value::<crate::Model>(first.clone())
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    Ok(json!([
        reply("getAll", json!([]), json!([model("factory")])),
        reply("getAll", json!([]), json!([first])),
        reply("find", json!(["author-provider", "first"]), first.clone()),
        reply(
            "getProviderDisplayName",
            json!(["author-provider"]),
            json!(" Literal provider ")
        ),
        reply(
            "getApiKeyForProvider",
            json!(["author-provider"]),
            json!("literal-key")
        ),
        reply("hasConfiguredAuth", json!([wire_model]), json!(true)),
        reply(
            "getProviderAuthStatus",
            json!(["author-provider"]),
            json!({"configured":true,"source":"models_json_key"})
        ),
        reply("isUsingOAuth", json!([wire_model]), json!(false)),
        reply(
            "auth",
            json!([first]),
            json!({"apiKey":"literal-key","headers":[["x-test","literal"],["Authorization","Bearer literal-key"]]})
        ),
        reply(
            "registerProvider",
            json!(["author-provider", provider_config("second")]),
            Value::Null
        ),
        reply("getAll", json!([]), json!([model("second")])),
        reply("refresh", json!([]), Value::Null),
        reply("getAll", json!([]), json!([model("second")])),
        reply("getError", json!([]), Value::Null),
        reply("getAll", json!([]), json!([]))
    ]))
}

/// The complete provider record expected at the import boundary.
fn provider_config(id: &str) -> String {
    json!({"name":" Literal provider ","baseUrl":"https://provider.invalid","apiKey":"literal-key","api":"openai-completions","headers":{"x-test":"literal"},"authHeader":true,"models":[{"id":id,"name":id,"reasoning":false,"input":["text"],"cost":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0},"contextWindow":128.0,"maxTokens":16.0}]}).to_string()
}

/// Scripted state and later reads for the retained credential observer.
fn credential_replies() -> Value {
    let provider = json!(["author-credential"]);
    let stored = json!({"type":"api_key","key":"!checkpoint-credential"});
    json!([
        reply(
            "set",
            json!(["author-credential", stored.to_string()]),
            Value::Null
        ),
        reply(
            "set_runtime_api_key",
            json!(["author-credential", "runtime-key"]),
            Value::Null
        ),
        reply(
            "getApiKeyForProvider",
            provider.clone(),
            json!("runtime-key")
        ),
        reply("remove_runtime_api_key", provider.clone(), Value::Null),
        reply(
            "getApiKeyForProvider",
            provider.clone(),
            json!("original effect key")
        ),
        reply("get", provider.clone(), stored.clone()),
        reply("list", json!([]), provider.clone()),
        reply("has", provider.clone(), json!(true)),
        reply("has_auth", provider.clone(), json!(true)),
        reply(
            "get_auth_status",
            provider.clone(),
            json!({"configured":true,"source":"stored"})
        ),
        reply("drain_errors", json!([]), json!([])),
        reply("get_all", json!([]), json!({"author-credential":stored})),
        reply("reload", json!([]), Value::Null),
        reply("remove", provider.clone(), Value::Null),
        reply("has", provider.clone(), json!(false)),
        reply("get", provider.clone(), Value::Null),
        reply(
            "set",
            json!(["author-credential", stored.to_string()]),
            Value::Null
        ),
        reply("logout", provider, Value::Null),
        reply("list", json!([]), json!([])),
        reply(
            "set",
            json!(["author-credential", stored.to_string()]),
            Value::Null
        ),
        reply(
            "get_api_key",
            json!(["author-credential", null]),
            json!("original effect key")
        )
    ])
}

/// Independent memory construction uses the supplied credential owner.
fn constructor_replies() -> Value {
    json!([
        reply("auth_storage_in_memory", json!(["{}"]), Value::Null),
        reply("in_memory_model_registry", json!([]), Value::Null),
        reply(
            "set_runtime_api_key",
            json!(["constructor-provider", "changed-after-construction"]),
            Value::Null
        ),
        reply(
            "getApiKeyForProvider",
            json!(["constructor-provider"]),
            json!("changed-after-construction")
        ),
        reply(
            "get_api_key",
            json!(["constructor-provider", null]),
            json!("changed-after-construction")
        ),
        reply("getError", json!([]), Value::Null),
        reply("list", json!([]), json!([]))
    ])
}

/// Native callback outcomes stay separate from scripted accepted credential state.
fn backend_replies() -> Vec<Value> {
    let raw = r#"{"shared":{"type":"api_key","key":"original"}}"#;
    let next = r#"{"shared":{"type":"api_key","key":"accepted"}}"#;
    vec![
        reply("auth_storage_from_backend", json!([]), Value::Null),
        reply("lock_update", json!([raw]), Value::Null),
        reply("backend_result", json!([]), Value::Null),
        reply("in_memory_model_registry", json!([]), Value::Null),
        reply(
            "set_runtime_api_key",
            json!(["shared", "changed-after-construction"]),
            Value::Null,
        ),
        reply(
            "getApiKeyForProvider",
            json!(["shared"]),
            json!("changed-after-construction"),
        ),
        reply("remove_runtime_api_key", json!(["shared"]), Value::Null),
        reply(
            "set",
            json!(["shared", r#"{"type":"api_key","key":"accepted"}"#]),
            Value::Null,
        ),
        reply("lock_update", json!([raw]), json!(next)),
        reply("backend_result", json!([]), json!("write-first")),
        reply("getApiKeyForProvider", json!(["shared"]), json!("accepted")),
        reply(
            "drain_errors",
            json!([]),
            json!([{"message":"write-first"}]),
        ),
        reply("reload", json!([]), Value::Null),
        reply("lock_update", json!([raw]), Value::Null),
        reply("backend_result", json!([]), Value::Null),
        reply("getApiKeyForProvider", json!(["shared"]), json!("original")),
    ]
}

/// Each fallback result is checked against the actual authored callback.
fn callback_replies() -> Value {
    let mut replies = backend_replies();
    replies.extend([
        reply("set_fallback_resolver", json!([]), Value::Null),
        reply(
            "fallback_result",
            json!(["shared-no-env"]),
            json!("fallback-key"),
        ),
        reply("has_auth", json!(["shared-no-env"]), json!(true)),
        reply(
            "fallback_result",
            json!(["shared-no-env"]),
            json!("fallback-key"),
        ),
        reply(
            "get_auth_status",
            json!(["shared-no-env"]),
            json!({"configured":false,"source":"fallback","label":"custom provider config"}),
        ),
        reply(
            "fallback_result",
            json!(["shared-no-env"]),
            json!("fallback-key"),
        ),
        reply(
            "get_api_key",
            json!(["shared-no-env", null]),
            json!("fallback-key"),
        ),
        reply(
            "set_runtime_api_key",
            json!(["shared", "later-key"]),
            Value::Null,
        ),
        reply("drain_errors", json!([]), json!([])),
        reply("get_api_key", json!(["shared", null]), json!("later-key")),
        reply("get_api_key", json!(["shared", null]), json!("later-key")),
    ]);
    Value::Array(replies)
}

/// The latest authored command result after a normal or deliberately held invocation.
fn answer(driver: &impl Driver) -> Result<Value, String> {
    let text = driver
        .transcript()
        .into_iter()
        .rev()
        .find_map(|line| {
            line.strip_prefix("entry provider-answer ")
                .map(str::to_owned)
        })
        .ok_or("missing provider answer")?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}
