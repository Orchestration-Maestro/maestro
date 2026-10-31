#![cfg(test)]
//! Initial model choice precedence.
use crate::fallback;
use crate::isolation;
use crate::scopes;
use maestro_catalog::{
    DEFAULT_MODEL_PER_PROVIDER, FindInitialModelOptions, ModelResolverOutput,
    RestoreModelFromSessionOptions, ScopedModel, find_initial_model, model_id_comparator,
    resolve_model_scope, restore_model_from_session,
};
use scopes::{Handle, model, registry, replay};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

/// Unconfigured initial-choice operands: no saved default, with any scope the caller supplies.
fn initial<'a>(
    registry: &'a maestro_catalog::ModelRegistry,
    scoped_models: &'a [ScopedModel],
) -> FindInitialModelOptions<'a> {
    FindInitialModelOptions {
        cli_provider: None,
        cli_model: None,
        scoped_models,
        is_continuing: false,
        default_provider: None,
        default_model_id: None,
        default_thinking_level: None,
        model_registry: registry,
    }
}

#[test]
fn initial_explicit_custom_id_preserves_metadata() {
    replay(&["case_070"]);
}

#[test]
fn initial_gateway_singleton_is_first_available() {
    replay(&["case_071"]);
}

#[test]
fn initial_cli_requires_both_nonempty_operands() {
    replay(&[
        "case_072", "case_073", "case_074", "case_075", "case_076", "case_077", "case_078",
    ]);
}

#[test]
fn initial_scope_thinking_precedence_is_independent() {
    replay(&[
        "case_079", "case_080", "case_081", "case_082", "case_083", "case_084",
    ]);
}

#[test]
fn initial_continuation_skips_scope() {
    replay(&["case_085"]);
}

#[test]
fn initial_scope_precedes_saved_default() {
    replay(&["case_086"]);
}

#[test]
fn initial_saved_default_does_not_require_auth() {
    replay(&["case_087", "case_088", "case_089"]);
}

#[test]
fn initial_missing_saved_default_falls_through() {
    replay(&["case_090", "case_091", "case_092", "case_093", "case_094"]);
}

#[test]
fn initial_fallback_uses_provider_table_order() {
    if isolation::isolated("initial_fallback_uses_provider_table_order") {
        return;
    }
    replay(&["case_095", "case_096", "case_097", "case_098"]);
}

#[test]
fn initial_fallback_ignores_supplied_thinking() {
    replay(&["case_099", "case_100"]);
}

#[test]
fn initial_cli_errors_return_catalog_error() {
    replay(&["case_101", "case_102"]);
}

#[test]
fn selection_handles_survive_catalog_removal() {
    let compare = model_id_comparator(&|_| None).unwrap();
    let handles = [model("p", "alpha"), model("p", "beta")];
    let registry = registry(&handles, &["p".into()]);
    let (mut stdout, mut stderr) = (scopes::Broken::default(), scopes::Broken::default());
    let mut output = ModelResolverOutput {
        stdout: &mut stdout,
        stderr: &mut stderr,
        color: false,
    };
    let patterns = ["alpha".to_owned()];
    let scoped = resolve_model_scope(&patterns, &registry, &compare, &mut output).unwrap();
    let initial = find_initial_model(
        FindInitialModelOptions {
            default_provider: Some("p"),
            default_model_id: Some("alpha"),
            ..initial(&registry, &[])
        },
        &compare,
    )
    .unwrap();
    let restored = restore_model_from_session(
        RestoreModelFromSessionOptions {
            saved_provider: "p",
            saved_model_id: "alpha",
            current_model: None,
            should_print_messages: false,
            model_registry: &registry,
        },
        &mut output,
    )
    .unwrap();
    let results: [Handle; 3] = [
        Arc::clone(&scoped[0].model),
        initial.model.unwrap(),
        restored.model.unwrap(),
    ];
    let released = Arc::downgrade(&handles[0]);
    handles[0].write().unwrap().name = "edited".into();
    registry.get_all().write().unwrap().remove(0);
    for result in &results {
        assert!(Arc::ptr_eq(result, &handles[0]));
        assert_eq!(result.read().unwrap().name, "edited");
    }
    drop((results, scoped, handles));
    assert!(released.upgrade().is_none());
}

#[test]
fn initial_custom_choice_stays_detached() {
    let compare = model_id_comparator(&|_| None).unwrap();
    let handles = [model("p", "alpha")];
    let registry = registry(&handles, &["p".into()]);
    let chosen = find_initial_model(
        FindInitialModelOptions {
            cli_provider: Some("p"),
            cli_model: Some("ghost"),
            ..initial(&registry, &[])
        },
        &compare,
    )
    .unwrap()
    .model
    .unwrap();
    let custom = chosen.read().unwrap();
    let source = handles[0].read().unwrap();
    assert_eq!(
        (custom.id.as_str(), custom.name.as_str()),
        ("ghost", "ghost")
    );
    assert_eq!(
        (&custom.api, &custom.base_url),
        (&source.api, &source.base_url)
    );
    assert_eq!(source.id, "alpha");
    assert!(!Arc::ptr_eq(&chosen, &handles[0]));
    assert_eq!(registry.get_all().read().unwrap().len(), 1);
}

#[test]
fn selection_early_returns_avoid_auth_reads() {
    let compare = model_id_comparator(&|_| None).unwrap();
    let handles = [model("p", "alpha"), model("q", "beta")];
    let registry = registry(&handles, &[]);
    let reads = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&reads);
    registry.auth_storage().set_fallback_resolver(move |_| {
        counter.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    });
    let scoped = [ScopedModel {
        model: Arc::clone(&handles[1]),
        thinking_level: None,
    }];
    let reads_after = |options: FindInitialModelOptions<'_>| {
        reads.store(0, Ordering::SeqCst);
        let chosen = find_initial_model(options, &compare).unwrap().model;
        (chosen.is_some(), reads.load(Ordering::SeqCst))
    };
    let cli = FindInitialModelOptions {
        cli_provider: Some("p"),
        cli_model: Some("alpha"),
        ..initial(&registry, &[])
    };
    assert_eq!(reads_after(cli), (true, 0));
    assert_eq!(reads_after(initial(&registry, &scoped)), (true, 0));
    let saved = FindInitialModelOptions {
        default_provider: Some("q"),
        default_model_id: Some("beta"),
        ..initial(&registry, &[])
    };
    assert_eq!(reads_after(saved), (true, 0));
    assert_eq!(reads_after(initial(&registry, &[])), (false, 2));
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let mut output = ModelResolverOutput {
        stdout: &mut stdout,
        stderr: &mut stderr,
        color: false,
    };
    let mut restore = |current: Option<&Handle>| {
        reads.store(0, Ordering::SeqCst);
        restore_model_from_session(
            RestoreModelFromSessionOptions {
                saved_provider: "none",
                saved_model_id: "none",
                current_model: current,
                should_print_messages: false,
                model_registry: &registry,
            },
            &mut output,
        )
        .unwrap();
        reads.load(Ordering::SeqCst)
    };
    assert_eq!(restore(Some(&handles[0])), 0);
    assert_eq!(restore(None), 2);
    registry.auth_storage().set_fallback_resolver(|_| Ok(None));
}

#[test]
fn initial_availability_callbacks_affect_default_choice() {
    if isolation::isolated("initial_availability_callbacks_affect_default_choice") {
        return;
    }
    let compare = model_id_comparator(&|_| None).unwrap();
    let handles = [model("p", "first"), model("q", "doomed")];
    let registry = registry(&handles, &[]);
    let replacement = model("r", "new");
    let list = registry.get_all();
    let visits = Arc::new(Mutex::new(Vec::new()));
    let (seen, added) = (Arc::clone(&visits), Arc::clone(&replacement));
    registry
        .auth_storage()
        .set_fallback_resolver(move |provider| {
            seen.lock().unwrap().push(provider.to_owned());
            if provider == "p" {
                let mut list = list.write().unwrap();
                list.remove(1);
                list.push(Arc::clone(&added));
                let mut table = DEFAULT_MODEL_PER_PROVIDER.write().unwrap();
                table.clear();
                table.insert("r".into(), "new".into());
            }
            Ok(Some("present".into()))
        });
    let chosen = find_initial_model(initial(&registry, &[]), &compare)
        .unwrap()
        .model
        .unwrap();
    assert!(Arc::ptr_eq(&chosen, &replacement));
    assert_eq!(*visits.lock().unwrap(), ["p", "r"]);
    registry.auth_storage().set_fallback_resolver(|_| Ok(None));
}

#[test]
fn initial_choice_short_circuits_or_returns_availability_error() {
    for case in fallback::cases("initial_choice_short_circuits_or_returns_availability_error") {
        let registry = fallback::registry(false);
        let calls = fallback::failing(&registry);
        let chosen = case.input.chosen.unwrap();
        let model = registry.find("maestro-custom-fallback", "m").unwrap();
        let scoped = [ScopedModel {
            model: model.clone(),
            thinking_level: Some(maestro_models::ModelThinkingLevel::High),
        }];
        let mut options = initial(&registry, &[]);
        match chosen.as_str() {
            "cli" => {
                options.cli_provider = Some("maestro-custom-fallback");
                options.cli_model = Some("m");
            }
            "scope" => options.scoped_models = &scoped,
            "default" => {
                options.default_provider = Some("maestro-custom-fallback");
                options.default_model_id = Some("m");
            }
            "none" => {}
            _ => panic!("unknown recorded initial choice"),
        }
        let result = find_initial_model(options, &str::cmp);
        let actual = match result {
            Ok(result) => {
                serde_json::json!({"branch":"ok", "value": {"model": fallback::model_value(result.model.as_ref().unwrap()), "thinkingLevel":result.thinking_level}, "calls":*calls.lock().unwrap()})
            }
            Err(error) => {
                serde_json::json!({"branch":"error", "error":fallback::error_value(&error), "calls":*calls.lock().unwrap()})
            }
        };
        assert!(actual == case.expected, "recorded initial-choice route");
    }
}

#[test]
fn coding_plan_defaults_track_current_glm() {
    if isolation::isolated("coding_plan_defaults_track_current_glm") {
        return;
    }
    let table = DEFAULT_MODEL_PER_PROVIDER.read().unwrap();
    for provider in ["zai", "zai-coding-cn"] {
        assert_eq!(
            table.get(provider).map(String::as_str),
            Some("glm-5.3"),
            "{provider}"
        );
    }
}
