#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
//! Request authentication through the public registry.
use crate::fallback;
use crate::request_support;
use crate::support;
use maestro_credentials::ConfigValueOperations;
use std::sync::Arc;

struct Operations;
impl ConfigValueOperations for Operations {
    fn execute(&self, _: &str) -> Option<Vec<u8>> {
        panic!("unexpected helper")
    }
}
#[test]
fn empty_stored_keys_block_configured_fallback() {
    let operations = Arc::new(request_support::Operations::default());
    operations.command("never", Some("fallback"));
    let registry = support::registry_with(
        r#"{"providers":{"p":{"apiKey":"!never","baseUrl":"url","api":"test","models":[{"id":"m"}]}}}"#,
        Arc::clone(&operations) as maestro_catalog::ConfigValues,
    );
    registry.auth_storage().set(
        "p",
        maestro_credentials::AuthCredential::ApiKey(maestro_credentials::ApiKeyCredential {
            env: None,
            key: String::new(),
        }),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let key = runtime
        .block_on(registry.get_api_key_for_provider("p"))
        .unwrap();
    assert!(key.as_deref().eq(&Some("")), "present empty key must win");
    success(request(&registry), Some(""), &[]);
    assert!(operations.calls.lock().unwrap().is_empty());
}
#[test]
fn request_headers_merge_exact_names_in_layer_order() {
    let registry = support::registry_with(
        r#"{"providers":{"p":{"apiKey":"configured-key","baseUrl":"url","api":"test","authHeader":true,"headers":{"X":"provider","authorization":"lower","Authorization":"old","P":"provider"},"models":[{"id":"m","headers":{"X":"model","M":"model"}}]}}}"#,
        Arc::new(Operations),
    );
    let model = registry.find("p", "m").unwrap();
    model.write().unwrap().headers = Some(
        [
            ("X".into(), "descriptor".into()),
            ("authorization".into(), "literal".into()),
        ]
        .into(),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let maestro_catalog::ResolvedRequestAuth::Ok { api_key, headers } =
        runtime.block_on(registry.get_api_key_and_headers(&model))
    else {
        panic!("expected success")
    };
    assert!(api_key.as_deref().eq(&Some("configured-key")));
    let headers = headers.unwrap();
    assert!(headers.get("X").map(String::as_str).eq(&Some("model")));
    assert!(
        headers
            .get("Authorization")
            .map(String::as_str)
            .eq(&Some("Bearer configured-key"))
    );
    assert!(
        headers
            .get("authorization")
            .map(String::as_str)
            .eq(&Some("lower"))
    );
    assert_eq!(
        headers.keys().map(String::as_str).collect::<Vec<_>>(),
        ["X", "authorization", "Authorization", "P", "M"]
    );
    header_absence_and_literals();
}

use request_support::{document, error, request, run, success};
#[test]
fn native_helpers_preserve_shell_output_and_failure() {
    let native = Arc::new(maestro_credentials::ProcessConfigValueOperations::new(
        || Err(std::io::Error::other("unused shell selector")),
    ));

    for (index, (input, expected, failure)) in NATIVE_CASES.iter().copied().enumerate() {
        let registry =
            support::registry_with(input, Arc::clone(&native) as maestro_catalog::ConfigValues);
        let key = run(registry.get_api_key_for_provider("p")).unwrap();
        assert!(key.as_deref().eq(&expected), "native helper case {index}");
        match failure {
            None => success(request(&registry), expected, &[]),
            Some(failure) => error(request(&registry), failure),
        }
    }
}

#[test]
fn configured_environment_is_read_on_each_call() {
    let values = [None, Some(""), Some("first"), Some("second"), Some(" ")];
    let Some(values) = maestro_test_process::run_per_environment(
        std::thread::current().name().unwrap(),
        values,
        |value| value.map(|value| ("ENV", value)).into_iter().collect(),
    )
    .unwrap() else {
        return;
    };
    let registry = support::registry(&document(
        r#""apiKey":"ENV","headers":{"X":"ENV"}"#,
        r#", "headers":{"M":"ENV"}"#,
    ));
    for value in values {
        let expected = value.filter(|v| !v.is_empty()).unwrap_or("ENV");
        let key = run(registry.get_api_key_for_provider("p")).unwrap();
        assert!(key.as_deref().eq(&Some(expected)));
        success(
            request(&registry),
            Some(expected),
            &[("X", expected), ("M", expected)],
        );
        let status = registry.get_provider_auth_status("p").unwrap();
        assert!(status.configured);
        assert_eq!(
            status.source,
            Some(if expected == "ENV" {
                maestro_credentials::AuthSource::ModelsJsonKey
            } else {
                maestro_credentials::AuthSource::Environment
            })
        );
        assert_eq!(
            status.label.as_deref(),
            (expected != "ENV").then_some("ENV")
        );
    }
}

#[test]
fn request_keys_preserve_lower_layer_precedence() {
    if !maestro_test_process::selection().is_child() {
        let output = maestro_test_process::TestProcess::current(
            "request_keys_preserve_lower_layer_precedence",
        )
        .unwrap()
        .case("*")
        .env("OPENAI_API_KEY", "ambient-key")
        .output()
        .unwrap();
        assert!(output.status.success(), "key subprocess failed");

        return;
    }
    let registry = support::registry(&document(r#""apiKey":"configured""#, ""));
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = calls.clone();
    registry.auth_storage().set_fallback_resolver(move |_| {
        seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Some("fallback".into()))
    });
    for (stored, runtime, expected) in [
        (None, None, "configured"),
        (Some("stored"), None, "stored"),
        (Some("stored"), Some("runtime"), "runtime"),
        (Some("stored"), Some(""), "stored"),
        (Some("!failed"), Some(""), "configured"),
    ] {
        if let Some(key) = stored {
            registry.auth_storage().set(
                "p",
                maestro_credentials::AuthCredential::ApiKey(
                    maestro_credentials::ApiKeyCredential {
                        env: None,
                        key: key.into(),
                    },
                ),
            );
        }
        if let Some(key) = runtime {
            registry.auth_storage().set_runtime_api_key("p", key);
        }
        let key = run(registry.get_api_key_for_provider("p")).unwrap();
        assert!(key.as_deref().eq(&Some(expected)));
        success(request(&registry), Some(expected), &[]);
    }
    let key = run(registry.get_api_key_for_provider("openai")).unwrap();
    assert!(key.as_deref().eq(&Some("ambient-key")));
    assert!(
        run(registry.get_api_key_for_provider("unknown-keys"))
            .unwrap()
            .is_none()
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    registry.auth_storage().set_fallback_resolver(|_| Ok(None));
}

#[test]
fn authorization_requires_nonempty_resolved_key() {
    for (configured, auth_header, expected) in [
        (false, false, None),
        (false, true, None),
        (true, false, Some("")),
        (true, true, Some("")),
    ] {
        let registry = support::registry(&document(
            &format!(
                r#""apiKey":"configured","authHeader":{auth_header},"headers":{{"X":"kept"}}"#
            ),
            "",
        ));
        if configured {
            registry.auth_storage().set(
                "p",
                maestro_credentials::AuthCredential::ApiKey(
                    maestro_credentials::ApiKeyCredential {
                        env: None,
                        key: String::new(),
                    },
                ),
            );
        } else {
            // A header-only provider retains a descriptor supplied independently.
            let file = support::registry(&serde_json::json!({"providers":{"p":{"headers":{"X":"kept"},"authHeader":auth_header}}}).to_string());
            let model = registry.find("p", "m").unwrap();
            let result = run(file.get_api_key_and_headers(&model));
            if auth_header {
                error(result, "No API key found for \"p\"");
            } else {
                success(result, expected, &[("X", "kept")]);
            }
            continue;
        }
        let result = request(&registry);
        if auth_header {
            error(result, "No API key found for \"p\"");
        } else {
            success(result, expected, &[("X", "kept")]);
        }
    }
}

#[test]
fn overwritten_header_failures_prevent_later_effects() {
    for (fields, model_fields, diagnostic, calls) in [
        (
            r#""apiKey":"!key","headers":{"X":"!later"}"#,
            "",
            "API key for provider \"p\"",
            vec!["key"],
        ),
        (
            r#""apiKey":"key","headers":{"X":"!provider"}"#,
            r#", "headers":{"X":"!later"}"#,
            "provider \"p\" header \"X\"",
            vec!["provider"],
        ),
        (
            r#""apiKey":"key","headers":{"X":"provider"}"#,
            r#", "headers":{"X":"!model","Y":"!later"}"#,
            "model \"p/m\" header \"X\"",
            vec!["model"],
        ),
        (
            r#""apiKey":"key","authHeader":true,"headers":{"Authorization":"!authorization"}"#,
            r#", "headers":{"X":"!later"}"#,
            "provider \"p\" header \"Authorization\"",
            vec!["authorization"],
        ),
    ] {
        let operations = Arc::new(request_support::Operations::default());
        operations.command("later", Some("success"));
        let registry = support::registry_with(
            &document(fields, model_fields),
            Arc::clone(&operations) as maestro_catalog::ConfigValues,
        );
        error(
            request(&registry),
            &format!(
                "Failed to resolve {diagnostic} from shell command: {}",
                calls[0]
            ),
        );
        assert_eq!(*operations.calls.lock().unwrap(), calls);
    }
}

#[test]
fn header_resolution_preserves_record_enumeration_order() {
    let operations = Arc::new(request_support::Operations::default());
    let registry = support::registry_with(
        &document(
            r#""apiKey":"key","headers":{"10":"!ten","2":"!two","01":"one","Z":"z","A":"a"}"#,
            "",
        ),
        Arc::clone(&operations) as maestro_catalog::ConfigValues,
    );
    error(
        request(&registry),
        "Failed to resolve provider \"p\" header \"2\" from shell command: two",
    );
    assert_eq!(*operations.calls.lock().unwrap(), ["two"]);
    operations.command("two", Some("two"));
    operations.command("ten", Some("ten"));
    success(
        request(&registry),
        Some("key"),
        &[
            ("2", "two"),
            ("10", "ten"),
            ("01", "one"),
            ("Z", "z"),
            ("A", "a"),
        ],
    );
    numeric_header_boundaries_and_merge();
}

#[test]
fn admitted_definitions_replace_override_header_storage() {
    for model_fields in ["", r#", "headers":{}"#, r#", "headers":{"X":"definition"}"#] {
        let registry = support::registry(&document(
            r#""apiKey":"key","modelOverrides":{"m":{"headers":{"X":"override"}}}"#,
            model_fields,
        ));
        let expected = if model_fields.contains("definition") {
            vec![("X", "definition")]
        } else {
            vec![]
        };
        success(request(&registry), Some("key"), &expected);
    }
}

#[test]
fn pair_keys_stay_distinct_through_reload_and_removal() {
    let text = support::Text(Arc::new(std::sync::Mutex::new(Some(r#"{"providers":{"a:b":{"api":"test","baseUrl":"url","apiKey":"key-ab","models":[{"id":"c","headers":{"X":"ab-c"}}]},"a":{"api":"test","baseUrl":"url","apiKey":"key-a","models":[{"id":"b:c","headers":{"X":"a-bc"}}]}}}"#.into()))));
    let registry = support::text_registry("models-input.json", &text, crate::support::values());
    let first = registry.find("a:b", "c").unwrap();
    let second = registry.find("a", "b:c").unwrap();
    success(
        run(registry.get_api_key_and_headers(&first)),
        Some("key-ab"),
        &[("X", "ab-c")],
    );
    success(
        run(registry.get_api_key_and_headers(&second)),
        Some("key-a"),
        &[("X", "a-bc")],
    );
    *text.0.lock().unwrap() = Some(r#"{"providers":{"a:b":{"api":"test","baseUrl":"url","apiKey":"key-ab","models":[{"id":"c","headers":{"X":"ab-c"}}]},"a":{"api":"test","baseUrl":"url","apiKey":"key-a","models":[{"id":"b:c"}]}}}"#.into());
    registry.refresh().unwrap();
    success(
        run(registry.get_api_key_and_headers(&first)),
        Some("key-ab"),
        &[("X", "ab-c")],
    );
    success(
        run(registry.get_api_key_and_headers(&second)),
        Some("key-a"),
        &[],
    );
}

#[test]
fn reload_clears_replaces_and_removes_request_configuration() {
    let text = support::Text(Arc::new(std::sync::Mutex::new(Some(document(
        r#""apiKey":"first","authHeader":true,"headers":{"X":"first"}"#,
        "",
    )))));
    let registry = support::text_registry("models-input.json", &text, crate::support::values());
    let model = registry.find("p", "m").unwrap();
    success(
        run(registry.get_api_key_and_headers(&model)),
        Some("first"),
        &[("X", "first"), ("Authorization", "Bearer first")],
    );
    *text.0.lock().unwrap() = Some(document(
        r#""apiKey":"second","headers":{"Y":"second"}"#,
        "",
    ));
    registry.refresh().unwrap();
    success(
        run(registry.get_api_key_and_headers(&model)),
        Some("second"),
        &[("Y", "second")],
    );
    for input in [Some("{".into()), None] {
        *text.0.lock().unwrap() = input;
        registry.refresh().unwrap();
        success(run(registry.get_api_key_and_headers(&model)), None, &[]);
    }
}

#[test]
fn configured_commands_repeat_across_calls_and_registries() {
    let input = document(r#""apiKey":"!fresh","authHeader":true"#, "");
    let operations = Arc::new(request_support::Operations::default());
    let registry = support::registry_with(
        &input,
        Arc::clone(&operations) as maestro_catalog::ConfigValues,
    );
    for (key, header) in [("key-1", "Bearer key-1"), ("key-2", "Bearer key-2")] {
        operations.command("fresh", Some(key));
        success(request(&registry), Some(key), &[("Authorization", header)]);
    }
    let second = support::registry_with(
        &input,
        Arc::clone(&operations) as maestro_catalog::ConfigValues,
    );
    operations.command("fresh", Some("key-3"));
    assert!(
        run(second.get_api_key_for_provider("p"))
            .unwrap()
            .as_deref()
            .eq(&Some("key-3"))
    );
    assert_eq!(
        *operations.calls.lock().unwrap(),
        ["fresh", "fresh", "fresh"]
    );
    operations.command("other", Some("independent"));
    let other = support::registry_with(
        &document(r#""apiKey":"!other""#, ""),
        Arc::clone(&operations) as maestro_catalog::ConfigValues,
    );
    assert!(
        run(other.get_api_key_for_provider("p"))
            .unwrap()
            .as_deref()
            .eq(&Some("independent"))
    );
}

#[test]
fn failed_configured_commands_are_retried() {
    let operations = Arc::new(request_support::Operations::default());
    let registry = support::registry_with(
        &document(r#""apiKey":"!retry""#, ""),
        Arc::clone(&operations) as maestro_catalog::ConfigValues,
    );
    for _ in 0..2 {
        assert!(
            run(registry.get_api_key_for_provider("p"))
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(*operations.calls.lock().unwrap(), ["retry", "retry"]);
}

#[test]
fn clear_key_cache_alias_clears_shared_credential_results() {
    let native = maestro_credentials::ProcessConfigValueOperations::new(|| {
        Err(std::io::Error::other("unused shell selector"))
    });
    let file = support::File::new(b"first");
    let command = format!("!cat '{}'", file.path());
    assert!(
        maestro_credentials::resolve_config_value(&command, &native)
            .as_deref()
            .eq(&Some("first"))
    );
    std::fs::write(file.path(), "second").unwrap();
    assert!(
        maestro_credentials::resolve_config_value(&command, &native)
            .as_deref()
            .eq(&Some("first"))
    );
    maestro_catalog::clear_api_key_cache();
    assert!(
        maestro_credentials::resolve_config_value(&command, &native)
            .as_deref()
            .eq(&Some("second"))
    );
}

#[test]
fn header_helpers_resolve_again_for_each_request() {
    let operations = Arc::new(request_support::Operations::default());
    let registry = support::registry_with(
        &document(
            r#""apiKey":"key","headers":{"P":"!provider-fresh"}"#,
            r#", "headers":{"M":"!model-fresh"}"#,
        ),
        Arc::clone(&operations) as maestro_catalog::ConfigValues,
    );
    for (provider, model) in [("provider-1", "model-1"), ("provider-2", "model-2")] {
        operations.command("provider-fresh", Some(provider));
        operations.command("model-fresh", Some(model));
        success(
            request(&registry),
            Some("key"),
            &[("P", provider), ("M", model)],
        );
    }
    assert_eq!(
        *operations.calls.lock().unwrap(),
        [
            "provider-fresh",
            "model-fresh",
            "provider-fresh",
            "model-fresh"
        ]
    );
}

#[test]
fn header_names_preserve_literal_identity() {
    let registry = support::registry(&document(
        r#""apiKey":"key","headers":{"__proto__":"proto","constructor":"constructor","toString":"text"}"#,
        "",
    ));
    success(
        request(&registry),
        Some("key"),
        &[
            ("__proto__", "proto"),
            ("constructor", "constructor"),
            ("toString", "text"),
        ],
    );
}

#[test]
fn selected_request_fields_keep_last_member_values() {
    let registry = support::registry(&document(
        r#""apiKey":"discarded","apiKey":"last","authHeader":true,"authHeader":false,"headers":{"X":"discarded","\u0058":"provider-last"}"#,
        r#", "headers":{"Y":"discarded","\u0059":"model-last"}"#,
    ));
    success(
        request(&registry),
        Some("last"),
        &[("X", "provider-last"), ("Y", "model-last")],
    );
    let registry = support::registry(&document(
        r#""apiKey":"old","apiKey":"new","authHeader":false,"authHeader":true,"headers":{"X":"old","\\u0058":"new"}"#,
        r#", "headers":{"Y":"old","Y":"new"}"#,
    ));
    success(
        request(&registry),
        Some("new"),
        &[
            ("X", "old"),
            ("\\u0058", "new"),
            ("Y", "new"),
            ("Authorization", "Bearer new"),
        ],
    );
}

#[test]
fn stored_commands_keep_lower_layer_cache_policy() {
    if !maestro_test_process::selection().is_child() {
        let output = maestro_test_process::TestProcess::current(
            "stored_commands_keep_lower_layer_cache_policy",
        )
        .unwrap()
        .case("*")
        .output()
        .unwrap();
        assert!(output.status.success(), "cache subprocess failed");

        return;
    }
    let operations = Arc::new(request_support::Operations::default());
    let registry = support::registry_with(
        &document(r#""apiKey":"!configured-cache-policy""#, ""),
        Arc::clone(&operations) as maestro_catalog::ConfigValues,
    );
    registry.auth_storage().set(
        "p",
        maestro_credentials::AuthCredential::ApiKey(maestro_credentials::ApiKeyCredential {
            env: None,
            key: "!stored-cache-policy".into(),
        }),
    );
    operations.command("stored-cache-policy", Some("first"));
    success(request(&registry), Some("first"), &[]);
    operations.command("stored-cache-policy", Some("second"));
    success(request(&registry), Some("first"), &[]);
    assert_eq!(*operations.calls.lock().unwrap(), ["stored-cache-policy"]);
    maestro_catalog::clear_api_key_cache();
    success(request(&registry), Some("second"), &[]);
    assert_eq!(
        *operations.calls.lock().unwrap(),
        ["stored-cache-policy", "stored-cache-policy"]
    );
}

#[test]
fn builtin_override_headers_resolve_without_changing_descriptors() {
    let registry = support::registry(
        r#"{"providers":{"openai":{"headers":{"P":"provider"},"modelOverrides":{"gpt-4o":{"headers":{"M":"model"}}}}}}"#,
    );
    let model = registry.find("openai", "gpt-4o").unwrap();
    assert!(model.read().unwrap().headers.is_none());
    registry
        .auth_storage()
        .set_runtime_api_key("openai", "runtime");
    success(
        run(registry.get_api_key_and_headers(&model)),
        Some("runtime"),
        &[("P", "provider"), ("M", "model")],
    );
    assert!(model.read().unwrap().headers.is_none());
    for (fields, expected) in [
        (r#""headers":{"P":"provider"}"#, vec![("P", "provider")]),
        (
            r#""baseUrl":"overridden","headers":{"P":"provider"}"#,
            vec![("P", "provider")],
        ),
        (
            r#""modelOverrides":{"gpt-4o":{"headers":{"M":"model"}}}"#,
            vec![("M", "model")],
        ),
    ] {
        let registry = support::registry(&format!(r#"{{"providers":{{"openai":{{{fields}}}}}}}"#));
        registry
            .auth_storage()
            .set_runtime_api_key("openai", "runtime");
        let model = registry.find("openai", "gpt-4o").unwrap();
        success(
            run(registry.get_api_key_and_headers(&model)),
            Some("runtime"),
            &expected,
        );
        assert!(model.read().unwrap().headers.is_none());
    }
}

/// Checked field-domain inputs with original source diagnostics.
const FIELD_CASES: &[FieldCase] = &[
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}]}}}",
        Some(
            "Failed to load models.json: Provider p: \"apiKey\" is required when defining custom models.\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"apiKey\": null}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.apiKey: must be string\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"apiKey\": 17}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.apiKey: must be string\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"apiKey\": \"\"}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.apiKey: must not have fewer than 1 characters\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"apiKey\": \"k\"}}}",
        None,
        Some("k"),
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"apiKey\": []}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.apiKey: must be string\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"apiKey\": {}}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.apiKey: must be string\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"apiKey\": false}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.apiKey: must be string\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"headers\": null}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.headers: must be object\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"headers\": 17}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.headers: must be object\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"headers\": {}}}}",
        Some(
            "Failed to load models.json: Provider p: \"apiKey\" is required when defining custom models.\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"headers\": {\"X\": \"v\"}}}}",
        Some(
            "Failed to load models.json: Provider p: \"apiKey\" is required when defining custom models.\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"headers\": []}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.headers: must be object\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"headers\": \"not-a-map-or-bool\"}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.headers: must be object\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"headers\": false}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.headers: must be object\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"headers\": {\"X\": null}}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.headers.X: must be string\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"headers\": {\"X\": 17}}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.headers.X: must be string\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"headers\": {\"X\": []}}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.headers.X: must be string\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"headers\": {\"X\": {}}}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.headers.X: must be string\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"authHeader\": null}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.authHeader: must be boolean\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"authHeader\": 17}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.authHeader: must be boolean\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"authHeader\": false}}}",
        Some(
            "Failed to load models.json: Provider p: \"apiKey\" is required when defining custom models.\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"authHeader\": true}}}",
        Some(
            "Failed to load models.json: Provider p: \"apiKey\" is required when defining custom models.\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"authHeader\": []}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.authHeader: must be boolean\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"authHeader\": \"not-a-map-or-bool\"}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.authHeader: must be boolean\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"authHeader\": {}}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.authHeader: must be boolean\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}]}}}",
        None,
        Some("configured-key"),
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\", \"headers\": null}]}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.models.0.headers: must be object\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\", \"headers\": 17}]}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.models.0.headers: must be object\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\", \"headers\": {}}]}}}",
        None,
        Some("configured-key"),
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\", \"headers\": {\"X\": \"v\"}}]}}}",
        None,
        Some("configured-key"),
        &[("X", "v")],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\", \"headers\": []}]}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.models.0.headers: must be object\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\", \"headers\": \"not-a-map-or-bool\"}]}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.models.0.headers: must be object\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\", \"headers\": false}]}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.models.0.headers: must be object\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\", \"headers\": {\"X\": null}}]}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.models.0.headers.X: must be string\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\", \"headers\": {\"X\": 17}}]}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.models.0.headers.X: must be string\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\", \"headers\": {\"X\": []}}]}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.models.0.headers.X: must be string\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\", \"headers\": {\"X\": {}}}]}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.models.0.headers.X: must be string\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"modelOverrides\": {\"m\": {}}}}}",
        None,
        Some("configured-key"),
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"modelOverrides\": {\"m\": {\"headers\": null}}}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.modelOverrides.m.headers: must be object\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"modelOverrides\": {\"m\": {\"headers\": 17}}}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.modelOverrides.m.headers: must be object\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"modelOverrides\": {\"m\": {\"headers\": {}}}}}}",
        None,
        Some("configured-key"),
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"modelOverrides\": {\"m\": {\"headers\": {\"X\": \"v\"}}}}}}",
        None,
        Some("configured-key"),
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"modelOverrides\": {\"m\": {\"headers\": []}}}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.modelOverrides.m.headers: must be object\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"modelOverrides\": {\"m\": {\"headers\": \"not-a-map-or-bool\"}}}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.modelOverrides.m.headers: must be object\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"modelOverrides\": {\"m\": {\"headers\": false}}}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.modelOverrides.m.headers: must be object\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"modelOverrides\": {\"m\": {\"headers\": {\"X\": null}}}}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.modelOverrides.m.headers.X: must be string\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"modelOverrides\": {\"m\": {\"headers\": {\"X\": 17}}}}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.modelOverrides.m.headers.X: must be string\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"modelOverrides\": {\"m\": {\"headers\": {\"X\": []}}}}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.modelOverrides.m.headers.X: must be string\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"configured-key\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}], \"modelOverrides\": {\"m\": {\"headers\": {\"X\": {}}}}}}}",
        Some(
            "Invalid models.json schema:\n  - providers.p.modelOverrides.m.headers.X: must be string\n\nFile: models-input.json",
        ),
        None,
        &[],
    ),
];
#[test]
fn request_fields_preserve_checked_input_domains() {
    for (input, diagnostic, key, headers) in FIELD_CASES {
        let registry = maestro_catalog::ModelRegistry::with_operations(
            support::auth(),
            "models-input.json",
            support::Text(Arc::new(std::sync::Mutex::new(Some((*input).into())))),
            crate::support::values(),
        )
        .unwrap();
        assert_eq!(registry.get_error().as_deref(), *diagnostic);
        if diagnostic.is_none() {
            success(request(&registry), *key, headers);
        }
    }
}
#[test]
fn request_result_catches_storage_errors_while_key_lookup_propagates() {
    if dynamic::isolated("request_result_catches_storage_errors_while_key_lookup_propagates") {
        return;
    }
    let id = "catalog-extraction-failure";
    let mut provider = request_support::OAuth::new(id, "controlled");
    provider.fail_key = true;
    let _registration = provider.register();
    let registry =
        maestro_catalog::ModelRegistry::in_memory(support::auth(), crate::support::values())
            .unwrap();
    let config = dynamic::config(&["m"]);
    config.write().unwrap().api_key = Some("!never".into());
    registry.register_provider(id, config).unwrap();
    let model = registry.find(id, "m").unwrap();
    registry
        .auth_storage()
        .set(id, request_support::tokens(f64::MAX));
    let failure: maestro_catalog::CatalogError =
        run(registry.get_api_key_for_provider(id)).unwrap_err();
    dynamic::assert_account_failure(&failure, "extraction failed");
    error(
        run(registry.get_api_key_and_headers(&model)),
        "extraction failed",
    );
}
#[test]
fn request_reads_shared_model_after_await_without_retaining_locks() {
    use std::future::Future;
    if dynamic::isolated("request_reads_shared_model_after_await_without_retaining_locks") {
        return;
    }
    let id = "catalog-read-phases";
    let (entered_send, mut entered) = tokio::sync::oneshot::channel();
    let (resume_send, resume) = tokio::sync::oneshot::channel();
    let channels = std::sync::Mutex::new(Some((entered_send, resume)));
    let mut provider = request_support::OAuth::new(id, "controlled");
    provider.refresh = Some(Box::new(move |mut credentials| {
        let (entered, resume) = channels.lock().unwrap().take().unwrap();
        Box::pin(async move {
            entered.send(()).unwrap();
            resume.await.unwrap();
            credentials.access = "fresh-old".into();
            credentials.expires = f64::MAX;
            Ok(credentials)
        })
    }));
    let _registration = provider.register();
    let registry = support::registry(
        r#"{"providers":{"catalog-read-phases":{"apiKey":"old-key","api":"test","baseUrl":"url","headers":{"P":"old-provider"},"models":[{"id":"old","headers":{"M":"old-model"}}]},"next":{"apiKey":"new-key","api":"test","baseUrl":"url","headers":{"P":"new-provider"},"models":[{"id":"new","headers":{"M":"new-model"}}]}}}"#,
    );
    registry
        .auth_storage()
        .set(id, request_support::tokens(0.0));
    let model = registry.find(id, "old").unwrap();
    model.write().unwrap().headers = Some([("D".into(), "old-descriptor".into())].into());
    let mut request = Box::pin(registry.get_api_key_and_headers(&model));
    let mut resume = Some(resume_send);
    let mut changed = false;
    let result = run(std::future::poll_fn(|context| {
        let poll = request.as_mut().poll(context);
        if !changed && entered.try_recv().is_ok() {
            let mut value = model.write().unwrap();
            value.provider = "next".into();
            value.id = "new".into();
            value.headers = Some([("D".into(), "new-descriptor".into())].into());
            drop(value);
            changed = true;
            resume.take().unwrap().send(()).unwrap();
        }
        poll
    }));
    assert!(changed, "refresh barrier was reached");
    success(
        result,
        Some("fresh-old"),
        &[
            ("D", "new-descriptor"),
            ("P", "old-provider"),
            ("M", "new-model"),
        ],
    );
}
#[test]
fn oauth_reload_failure_keeps_new_request_configuration() {
    if dynamic::isolated("oauth_reload_failure_keeps_new_request_configuration") {
        return;
    }
    let id = "catalog-request-reload";
    let provider = request_support::OAuth::new(id, "controlled");
    let fail = provider.fail_transform.clone();
    let provider = Arc::new(provider);
    let auth = support::auth();
    auth.set(id, request_support::tokens(f64::MAX));
    let text = support::Text(Arc::new(std::sync::Mutex::new(Some(document(
        r#""apiKey":"first","headers":{"X":"first"}"#,
        "",
    )))));
    let registry = maestro_catalog::ModelRegistry::with_operations(
        auth,
        "models-input.json",
        text.clone(),
        crate::support::values(),
    )
    .unwrap();
    let config = dynamic::config(&["dynamic"]);
    config.write().unwrap().callbacks.oauth = Some(provider);
    registry.register_provider(id, config).unwrap();
    let list = registry.get_all();
    let model = registry.find("p", "m").unwrap();
    *text.0.lock().unwrap() = Some(document(
        r#""apiKey":"second","headers":{"Y":"second"}"#,
        "",
    ));
    fail.store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        registry.refresh().unwrap_err().to_string(),
        "transform failed"
    );
    assert!(!Arc::ptr_eq(&list, &registry.get_all()));
    success(
        run(registry.get_api_key_and_headers(&model)),
        Some("second"),
        &[("Y", "second")],
    );
}

/// Native shell input and output observations.
const NATIVE_CASES: &[(&str, Option<&str>, Option<&str>)] = &[
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"literal-value\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}]}}}",
        Some("literal-value"),
        None,
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"!printf 'command-key'\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}]}}}",
        Some("command-key"),
        None,
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"!printf '  spaced-key  \\\\n'\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}]}}}",
        Some("spaced-key"),
        None,
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"!printf 'line1\\\\nline2'\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}]}}}",
        Some("line1\nline2"),
        None,
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"!printf '﻿ key ﻿'\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}]}}}",
        Some("key"),
        None,
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"!printf 'key'\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}]}}}",
        Some("key"),
        None,
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"!exit 1\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}]}}}",
        None,
        Some("Failed to resolve API key for provider \"p\" from shell command: exit 1"),
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"!maestro_missing_helper_474\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}]}}}",
        None,
        Some(
            "Failed to resolve API key for provider \"p\" from shell command: maestro_missing_helper_474",
        ),
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"!printf ''\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}]}}}",
        None,
        Some("Failed to resolve API key for provider \"p\" from shell command: printf ''"),
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"!printf ' \\\\n\\\\t'\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}]}}}",
        None,
        Some("Failed to resolve API key for provider \"p\" from shell command: printf ' \\n\\t'"),
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"!printf 'hello world' | tr ' ' '-'\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}]}}}",
        Some("hello-world"),
        None,
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \" !printf literal\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}]}}}",
        Some(" !printf literal"),
        None,
    ),
    (
        "{\"providers\": {\"p\": {\"baseUrl\": \"https://fixture.invalid\", \"apiKey\": \"!\", \"api\": \"openai-completions\", \"models\": [{\"id\": \"m\"}]}}}",
        None,
        Some("Failed to resolve API key for provider \"p\" from shell command: "),
    ),
];

/// Inline checked input, winning error and admitted request values.
type FieldCase = (
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
    &'static [(&'static str, &'static str)],
);

/// Empty maps vanish while descriptor values are literal, not expressions.
fn header_absence_and_literals() {
    for fields in [r#""apiKey":"key""#, r#""apiKey":"key","headers":{}"#] {
        let registry = support::registry(&document(fields, ""));
        success(request(&registry), Some("key"), &[]);
    }
    let registry = support::registry(&document(r#""apiKey":"key","headers":{"X":""}"#, ""));
    success(request(&registry), Some("key"), &[("X", "")]);
    let registry = support::registry(&document(r#""apiKey":"key""#, ""));
    let model = registry.find("p", "m").unwrap();
    model.write().unwrap().headers = Some(
        [
            ("X".into(), "!not-executed".into()),
            ("Y".into(), "ENV".into()),
        ]
        .into(),
    );
    success(
        request(&registry),
        Some("key"),
        &[("X", "!not-executed"), ("Y", "ENV")],
    );
}

/// Integer-name boundaries apply to merged descriptor and request records too.
fn numeric_header_boundaries_and_merge() {
    let registry = support::registry(
        r#"{"providers":{"p":{"headers":{"4294967295":"not-index","00":"double-zero","4294967294":"last-index","02":"leading-zero","2":"two","-1":"negative","0":"zero"}}}}"#,
    );
    let model = Arc::new(std::sync::RwLock::new(
        maestro_models::get_model("openai", "gpt-4o").unwrap(),
    ));
    model.write().unwrap().provider = "p".into();
    success(
        run(registry.get_api_key_and_headers(&model)),
        None,
        &[
            ("0", "zero"),
            ("2", "two"),
            ("4294967294", "last-index"),
            ("4294967295", "not-index"),
            ("00", "double-zero"),
            ("02", "leading-zero"),
            ("-1", "negative"),
        ],
    );
    let registry = support::registry(&document(
        r#""apiKey":"key","headers":{"10":"provider","2":"provider","P":"provider"}"#,
        r#", "headers":{"2":"model","M":"model"}"#,
    ));
    let model = registry.find("p", "m").unwrap();
    model.write().unwrap().headers = Some(
        [
            ("20".into(), "descriptor".into()),
            ("D".into(), "descriptor".into()),
        ]
        .into(),
    );
    success(
        request(&registry),
        Some("key"),
        &[
            ("2", "model"),
            ("10", "provider"),
            ("20", "descriptor"),
            ("D", "descriptor"),
            ("P", "provider"),
            ("M", "model"),
        ],
    );
}

use crate::dynamic;

#[test]
fn provider_request_lookup_avoids_auth_fallback() {
    for case in fallback::cases("provider_request_lookup_avoids_auth_fallback") {
        let registry = if case.input.include_config.unwrap() {
            support::registry(
                r#"{"providers":{"maestro-custom-fallback":{"api":"test","baseUrl":"url","apiKey":"","models":[{"id":"m"}]}}}"#,
            )
        } else {
            fallback::registry(false)
        };
        let calls = fallback::failing(&registry);
        let key =
            request_support::run(registry.get_api_key_for_provider("maestro-custom-fallback"))
                .unwrap();
        let actual =
            serde_json::json!({"branch":"ok", "value":key, "calls":*calls.lock().unwrap()});
        assert!(actual == case.expected, "recorded request lookup");
    }
}

/// The shared captured route neither inherits stdin nor replays successful output.
#[test]
fn captured_subprocess_preserves_descriptors_and_environment_order() {
    const NAME: &str = "captured_subprocess_preserves_descriptors_and_environment_order";
    let selection = maestro_test_process::selection();
    if selection.is_child() && selection.runs_case("inner") {
        use std::io::Read as _;
        let mut input = Vec::new();
        std::io::stdin().read_to_end(&mut input).unwrap();
        assert!(input.is_empty());
        assert!(!selection.runs_case("outer"));
        assert_eq!(std::env::var("MAESTRO_CAPTURE_VALUE").unwrap(), "supplied");
        assert!(std::env::var_os("MAESTRO_CAPTURE_REMOVE").is_none());
        println!("INNER-CAPTURE-STDOUT");
        eprintln!("INNER-CAPTURE-STDERR");
    } else if selection.is_child() && selection.runs_case("outer") {
        assert_eq!(
            std::env::var("MAESTRO_CAPTURE_REMOVE").unwrap(),
            "inherited"
        );
        crate::subprocess::run_child(
            maestro_test_process::TestProcess::current(NAME)
                .unwrap()
                .case("inner"),
            &[
                ("MAESTRO_CAPTURE_VALUE", Some("supplied")),
                ("MAESTRO_CAPTURE_REMOVE", None),
            ],
        );
    } else {
        assert!(!selection.is_child(), "unknown capture stage");
        let input = support::File::new(b"finite nonempty input");
        let output = maestro_test_process::TestProcess::current(NAME)
            .unwrap()
            .nocapture()
            .case("outer")
            .env("MAESTRO_CAPTURE_REMOVE", "inherited")
            .stdin(std::fs::File::open(input.path()).unwrap())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("INNER-CAPTURE-STDOUT"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("INNER-CAPTURE-STDERR"));
    }
}
