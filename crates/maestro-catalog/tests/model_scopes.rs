#![cfg(test)]
//! Model scopes resolved against authenticated models.
use crate::fallback;
use crate::scopes;
use maestro_catalog::ModelRegistry;
use maestro_catalog::{ModelResolverOutput, model_id_comparator, resolve_model_scope};
use maestro_credentials::{AuthStorage, AuthStorageData};
use maestro_models::SharedHandle;
use scopes::replay;
use std::sync::{Arc, Mutex};

#[test]
fn scope_uses_available_models_and_empty_inputs() {
    replay(&["case_001", "case_002", "case_003"]);
}

#[test]
fn scope_globs_preserve_catalog_order() {
    replay(&["case_004", "case_005", "case_006"]);
}

#[test]
fn scope_glob_triggers_are_literal_characters() {
    replay(&[
        "case_007", "case_008", "case_009", "case_010", "case_011", "case_012", "case_066",
        "case_067", "case_068",
    ]);
}

#[test]
fn scope_magic_uses_shared_matcher() {
    replay(&[
        "case_013", "case_014", "case_015", "case_016", "case_017", "case_018", "case_019",
        "case_020", "case_021",
    ]);
}

#[test]
fn scope_slashes_globstar_and_dot_rules_remain_distinct() {
    replay(&[
        "case_022", "case_023", "case_024", "case_025", "case_026", "case_027", "case_028",
    ]);
}

#[test]
fn scope_escaping_is_not_path_normalization() {
    replay(&["case_029", "case_030", "case_031", "case_032"]);
    // The glob reads `\\` in a model id as a separator on Windows, as minimatch does there,
    // so the recorded single match of `a/b*` holds only where `\\` stays part of the name.
    if cfg!(windows) {
        eprintln!("premise not met: this host reads `\\` in a matched name as a separator");
    } else {
        replay(&["case_033"]);
    }
}

#[test]
fn scope_valid_glob_suffixes_are_exact() {
    replay(&[
        "case_034", "case_035", "case_036", "case_037", "case_038", "case_039",
    ]);
}

#[test]
fn scope_invalid_glob_suffixes_stay_in_pattern() {
    replay(&["case_040", "case_041", "case_042", "case_043"]);
}

#[test]
fn scope_glob_suffix_is_removed_only_once() {
    replay(&["case_044", "case_045"]);
}

#[test]
fn scope_pattern_results_keep_parser_semantics() {
    replay(&[
        "case_046", "case_047", "case_048", "case_049", "case_050", "case_051", "case_052",
        "case_053", "case_069",
    ]);
}

#[test]
fn scope_warnings_keep_order_and_continue() {
    replay(&["case_054"]);
}

#[test]
fn scope_first_identity_keeps_its_level() {
    replay(&["case_055", "case_056", "case_057", "case_058"]);
}

#[test]
fn scope_auth_presence_does_not_run_secret_commands() {
    replay(&["case_059"]);
}

#[test]
fn scope_warning_styles_match_exact_bytes() {
    replay(&["case_060", "case_061", "case_062"]);
}

#[test]
fn scope_matcher_errors_propagate_after_prior_warnings() {
    let handles = [scopes::model("p", "alpha")];
    let registry = scopes::registry(&handles, &["p".into()]);
    let compare = model_id_comparator(&|_| None).unwrap();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let patterns = ["absent".to_owned(), format!("{}*", "a".repeat(65_537))];
    let result = resolve_model_scope(
        &patterns,
        &registry,
        &compare,
        &mut ModelResolverOutput {
            stdout: &mut stdout,
            stderr: &mut stderr,
            color: false,
        },
    );
    assert!(result.is_err());
    assert_eq!(
        String::from_utf8(stderr).unwrap(),
        "Warning: No models match pattern \"absent\"\n"
    );
}

#[test]
fn scope_diagnostics_are_best_effort() {
    let handles = [scopes::model("p", "alpha")];
    let registry = scopes::registry(&handles, &["p".into()]);
    let compare = model_id_comparator(&|_| None).unwrap();
    let (mut stdout, mut stderr) = (Vec::new(), scopes::Broken::default());
    let patterns = ["none*".to_owned(), "alpha".to_owned()];
    let scoped = resolve_model_scope(
        &patterns,
        &registry,
        &compare,
        &mut ModelResolverOutput {
            stdout: &mut stdout,
            stderr: &mut stderr,
            color: false,
        },
    )
    .unwrap();
    assert_eq!(stderr.0, 1);
    assert_eq!(scoped.len(), 1);
    assert!(Arc::ptr_eq(&scoped[0].model, &handles[0]));
}

#[test]
fn scope_empty_patterns_still_consult_authentication() {
    let handles = [scopes::model("p", "alpha")];
    let storage = SharedHandle::new(AuthStorage::in_memory(AuthStorageData::new()));
    let asked = Arc::new(Mutex::new(Vec::new()));
    let observer = Arc::clone(&asked);
    storage.set_fallback_resolver(move |provider| {
        observer.lock().unwrap().push(provider.to_owned());
        Ok(None)
    });
    let registry = ModelRegistry::in_memory(storage, crate::support::values()).unwrap();
    *registry.get_all().write().unwrap() = handles.to_vec();
    let compare = model_id_comparator(&|_| None).unwrap();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let scoped = resolve_model_scope(
        &[],
        &registry,
        &compare,
        &mut ModelResolverOutput {
            stdout: &mut stdout,
            stderr: &mut stderr,
            color: false,
        },
    )
    .unwrap();
    assert!(scoped.is_empty());
    assert_eq!(*asked.lock().unwrap(), ["p"]);
}

#[test]
fn scope_availability_error_precedes_pattern_diagnostics() {
    for case in fallback::cases("scope_availability_error_precedes_pattern_diagnostics") {
        let registry = fallback::registry(false);
        let calls = fallback::failing(&registry);
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let result = resolve_model_scope(
            &case.input.patterns.unwrap(),
            &registry,
            &|_, _| panic!("matcher must not run"),
            &mut ModelResolverOutput {
                stdout: &mut stdout,
                stderr: &mut stderr,
                color: false,
            },
        );
        let actual = serde_json::json!({"branch":"error", "error":fallback::error_value(&result.err().unwrap())});
        assert!(actual == case.expected, "recorded scope failure");
        assert!(stdout.is_empty() && stderr.is_empty());
        assert_eq!(*calls.lock().unwrap(), ["maestro-custom-fallback"]);
    }
}
