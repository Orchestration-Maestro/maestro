//! Model, effort, tool and resource-path selections of a created session.

use crate::support::startup;
use maestro_app::{Model, ModelRegistry, NoTools, ScopedModel, ThinkingLevel};
use maestro_cli::startup::main::options::{
    BuiltSessionOptions, SessionOptionInputs, build_session_options, resolve_cli_paths,
};
use serde_json::json;
use std::sync::{Arc, RwLock};

/// Write attempts the saved-descriptor probe makes before it gives up.
const PROBES: usize = 1_000_000;

/// Uninterrupted failed write attempts that mean a guard stayed held.
const LOCKED_RUN: usize = 1_000;

/// A scoped model over the catalog's descriptor, with its authored effort.
fn scoped(
    catalog: &ModelRegistry,
    provider: &str,
    id: &str,
    level: Option<ThinkingLevel>,
) -> ScopedModel {
    ScopedModel {
        model: catalog.find(provider, id).expect("a catalog model"),
        thinking_level: level,
    }
}

/// The controlled catalog, settings and comparator one selection reads.
struct Fixture<'a> {
    /// The catalog the selection resolves through.
    catalog: &'a ModelRegistry,
    /// The settings holding the saved default model.
    settings: &'a maestro_app::SettingsManager,
    /// The prepared identifier comparator.
    compare: &'a dyn Fn(&str, &str) -> std::cmp::Ordering,
}

impl Fixture<'_> {
    /// Select options from the supplied arguments and scope.
    fn build(
        &self,
        tokens: &[&str],
        scoped_models: &[ScopedModel],
        has_existing_session: bool,
    ) -> BuiltSessionOptions {
        build_session_options(SessionOptionInputs {
            parsed: &startup::args(tokens),
            scoped_models,
            has_existing_session,
            model_registry: self.catalog,
            settings: self.settings,
            compare_model_ids: self.compare,
        })
    }
}

/// The selected model's provider.
fn provider(built: &BuiltSessionOptions) -> Option<String> {
    built
        .options
        .model
        .as_ref()
        .map(|model| model.read().unwrap().provider.clone())
}

/// The selected model's identifier.
fn model_id(built: &BuiltSessionOptions) -> Option<String> {
    built
        .options
        .model
        .as_ref()
        .map(|model| model.read().unwrap().id.clone())
}

#[test]
fn explicit_model_and_provider_keep_catalog_diagnostics() {
    let catalog = startup::registry(&[("alpha", &["one", "two"]), ("beta", &["one"])]);
    let compare = startup::comparator();
    let settings = startup::settings(&[]);
    let fixture = Fixture {
        catalog: &catalog,
        settings: &settings,
        compare: &compare,
    };
    let select = |tokens: &[&str], scope: &[ScopedModel]| fixture.build(tokens, scope, false);

    let selected = select(&["--provider", "beta", "--model", "one"], &[]);
    assert_eq!(provider(&selected).as_deref(), Some("beta"));
    assert_eq!(model_id(&selected).as_deref(), Some("one"));
    assert!(selected.diagnostics.is_empty());

    let prefixed = select(&["--model", "alpha/two"], &[]);
    assert_eq!(provider(&prefixed).as_deref(), Some("alpha"));
    assert_eq!(model_id(&prefixed).as_deref(), Some("two"));

    let unknown = select(&["--provider", "missing", "--model", "one"], &[]);
    assert!(unknown.options.model.is_none());
    assert_eq!(unknown.diagnostics.len(), 1);
    assert_eq!(
        unknown.diagnostics[0].message,
        r#"Unknown provider "missing". Use --list-models to see available providers/models."#
    );

    let scope = [scoped(&catalog, "alpha", "two", None)];
    let fallback = select(&["--provider", "missing", "--model", "one"], &scope);
    assert_eq!(
        model_id(&fallback).as_deref(),
        Some("two"),
        "a failed explicit selection still takes the fresh scoped default"
    );
    assert_eq!(fallback.diagnostics.len(), 1);
}

#[test]
fn scope_defaults_distinguish_fresh_and_existing_history() {
    let catalog = startup::registry(&[("alpha", &["one", "two"]), ("beta", &["one"])]);
    let compare = startup::comparator();
    let scope = [
        scoped(&catalog, "alpha", "one", None),
        scoped(&catalog, "beta", "one", None),
    ];
    let saved = startup::settings(&[
        ("defaultProvider", json!("beta")),
        ("defaultModel", json!("one")),
    ]);

    let saved_fixture = Fixture {
        catalog: &catalog,
        settings: &saved,
        compare: &compare,
    };
    let fresh = saved_fixture.build(&[], &scope, false);
    assert_eq!(
        provider(&fresh).as_deref(),
        Some("beta"),
        "a saved default in scope wins"
    );
    let existing = saved_fixture.build(&[], &scope, true);
    assert!(
        existing.options.model.is_none(),
        "an existing history skips the default"
    );

    let provider_only = startup::settings(&[("defaultProvider", json!("beta"))]);
    let provider_fixture = Fixture {
        catalog: &catalog,
        settings: &provider_only,
        compare: &compare,
    };
    let first = provider_fixture.build(&[], &scope, false);
    assert_eq!(
        provider(&first).as_deref(),
        Some("alpha"),
        "a provider-only default does not match, so the first entry wins"
    );

    let outside = startup::settings(&[
        ("defaultProvider", json!("alpha")),
        ("defaultModel", json!("two")),
    ]);
    let outside_fixture = Fixture {
        catalog: &catalog,
        settings: &outside,
        compare: &compare,
    };
    let fallback = outside_fixture.build(&[], &scope, false);
    assert_eq!(
        model_id(&fallback).as_deref(),
        Some("one"),
        "a default outside the scope falls back to the first entry"
    );
    assert_eq!(
        fallback.options.scoped_models.as_ref().map(Vec::len),
        Some(2)
    );
}

#[test]
fn thinking_precedence_retains_inherited_scope_values() {
    let catalog = startup::registry(&[("alpha", &["one"])]);
    let compare = startup::comparator();
    let settings = startup::settings(&[]);
    let fixture = Fixture {
        catalog: &catalog,
        settings: &settings,
        compare: &compare,
    };
    let select = |tokens: &[&str], scope: &[ScopedModel]| fixture.build(tokens, scope, false);
    let without = [scoped(&catalog, "alpha", "one", None)];
    let with = [scoped(&catalog, "alpha", "one", Some(ThinkingLevel::High))];

    let scoped_only = select(&[], &with);
    assert_eq!(
        scoped_only.options.thinking_level,
        Some(ThinkingLevel::High)
    );
    assert!(!scoped_only.cli_thinking_from_model);

    let explicit_off = select(&["--thinking", "off"], &with);
    assert_eq!(
        explicit_off.options.thinking_level,
        Some(ThinkingLevel::Off)
    );
    assert!(
        !explicit_off.cli_thinking_from_model,
        "an explicit level is not the shorthand's"
    );

    let shorthand = select(&["--model", "alpha/one:high"], &without);
    assert_eq!(shorthand.options.thinking_level, Some(ThinkingLevel::High));
    assert!(
        shorthand.cli_thinking_from_model,
        "a model shorthand marks the CLI override"
    );

    let overridden = select(
        &["--model", "alpha/one:high", "--thinking", "low"],
        &without,
    );
    assert_eq!(
        overridden.options.thinking_level,
        Some(ThinkingLevel::Low),
        "an explicit level wins over the shorthand"
    );
    assert!(!overridden.cli_thinking_from_model);

    let inherited = select(&[], &without);
    assert_eq!(
        inherited.options.thinking_level, None,
        "an unset scope level stays absent so cycling inherits"
    );
}

#[test]
fn tool_selection_keeps_allowlist_with_disable_policy() {
    let catalog = startup::registry(&[("alpha", &["one"])]);
    let compare = startup::comparator();
    let settings = startup::settings(&[]);
    let fixture = Fixture {
        catalog: &catalog,
        settings: &settings,
        compare: &compare,
    };
    let select = |tokens: &[&str]| fixture.build(tokens, &[], false);

    let all = select(&["--no-tools", "--no-builtin-tools", "--tools", "read,bash"]);
    assert_eq!(all.options.no_tools, Some(NoTools::All));
    assert_eq!(
        all.options.tools,
        Some(vec!["read".to_owned(), "bash".to_owned()])
    );

    let builtin = select(&["--no-builtin-tools"]);
    assert_eq!(builtin.options.no_tools, Some(NoTools::Builtin));
    assert_eq!(builtin.options.tools, None);

    let allowlist = select(&["--tools", "read"]);
    assert_eq!(allowlist.options.no_tools, None);
    assert_eq!(allowlist.options.tools, Some(vec!["read".to_owned()]));

    let ordinary = select(&[]);
    assert_eq!(ordinary.options.no_tools, None);
    assert_eq!(ordinary.options.tools, None);
}

#[test]
fn cli_resource_paths_keep_startup_directory() {
    let cwd = "/projects/current";
    let authored = vec![
        "./extensions/local".to_owned(),
        "npm:maestro-formatter".to_owned(),
        "https://example.test/skill".to_owned(),
        "  ./skills/spaced  ".to_owned(),
        "/absolute/theme".to_owned(),
    ];
    let resolved = resolve_cli_paths(cwd, Some(&authored)).unwrap().unwrap();
    // The host's spelling of a POSIX path: Windows puts it on the working drive.
    let native = |path: &str| maestro_app::resolve(&[path]).unwrap();
    assert_eq!(
        resolved,
        vec![
            native("/projects/current/extensions/local"),
            "npm:maestro-formatter".to_owned(),
            "https://example.test/skill".to_owned(),
            native("/projects/current/  ./skills/spaced  "),
            native("/absolute/theme"),
        ]
    );
    assert_eq!(resolve_cli_paths(cwd, None).unwrap(), None);
    assert_eq!(
        resolve_cli_paths(cwd, Some(&[])).unwrap(),
        Some(Vec::new()),
        "an authored empty list stays an empty list"
    );
}

#[test]
fn saved_scope_default_releases_the_descriptor_before_the_traversal() {
    let catalog = startup::registry(&[("alpha", &["one"]), ("beta", &["one"])]);
    let compare = startup::comparator();
    let settings = startup::settings(&[
        ("defaultProvider", json!("alpha")),
        ("defaultModel", json!("one")),
    ]);
    let saved = catalog.find("alpha", "one").expect("the saved descriptor");
    let blocking = catalog
        .find("beta", "one")
        .expect("the scope's first descriptor");
    let scope = [
        ScopedModel {
            model: Arc::clone(&blocking),
            thinking_level: None,
        },
        ScopedModel {
            model: Arc::clone(&saved),
            thinking_level: None,
        },
    ];

    let (ready_tx, ready) = std::sync::mpsc::channel();
    let prober = {
        let (saved, blocking) = (Arc::clone(&saved), Arc::clone(&blocking));
        std::thread::spawn(move || {
            let first = blocking.write().expect("the scope's first descriptor");
            ready_tx.send(()).expect("a waiting selection");
            let locked = locked_while_traversing(&saved);
            drop(first);
            locked
        })
    };
    ready.recv().expect("a held first scope descriptor");

    let fixture = Fixture {
        catalog: &catalog,
        settings: &settings,
        compare: &compare,
    };
    let built = fixture.build(&[], &scope, false);
    let locked_during_traversal = prober.join().expect("the probing thread");

    assert_eq!(
        provider(&built).as_deref(),
        Some("alpha"),
        "a saved default behind the first entry still wins"
    );
    assert!(
        !locked_during_traversal,
        "the saved descriptor stays writable while the scope is traversed"
    );
}

/// Whether the saved descriptor stays read-locked for a long run of attempts.
///
/// A traversal that keeps the saved descriptor's guard held across the scope produces
/// such a run; one that reads it and releases it never does. The iteration bound keeps
/// the probe finite when nothing holds the guard.
fn locked_while_traversing(saved: &Arc<RwLock<Model>>) -> bool {
    let mut streak = 0;
    for _ in 0..PROBES {
        if saved.try_write().is_ok() {
            streak = 0;
        } else {
            streak += 1;
            if streak == LOCKED_RUN {
                return true;
            }
        }
    }
    false
}

#[test]
fn selected_model_keeps_the_shared_descriptor() {
    let catalog = startup::registry(&[("alpha", &["one"])]);
    let compare = startup::comparator();
    let settings = startup::settings(&[]);
    let scope = [scoped(&catalog, "alpha", "one", None)];
    let fixture = Fixture {
        catalog: &catalog,
        settings: &settings,
        compare: &compare,
    };
    let built = fixture.build(&[], &scope, false);
    let selected = built.options.model.expect("a selected model");
    assert!(
        Arc::ptr_eq(&selected, &catalog.find("alpha", "one").unwrap()),
        "the shared descriptor is retained rather than copied"
    );
}

#[test]
fn model_patterns_use_cli_before_saved_settings() {
    use crate::startup_runtime::{Fixture, run};
    use maestro_app::ModelResolverOutput;
    use maestro_cli::startup::main::runtime::create_runtime;
    run(
        "model_patterns_use_cli_before_saved_settings",
        |local| async move {
            for (stored, cli, expected) in [
                (json!(["startup/first"]), Some(vec![]), vec![]),
                (
                    json!(["startup/first"]),
                    Some(vec!["startup/second".into()]),
                    vec!["second"],
                ),
                (json!(["startup/first"]), None, vec!["first"]),
                (json!({}), None, vec![]),
                (json!([]), None, vec![]),
            ] {
                let fixture = Fixture::new(
                    local.clone(),
                    startup::settings(&[("enabledModels", stored)]),
                )
                .await;
                let parsed = maestro_cli::input::args::Args {
                    models: cli,
                    ..Default::default()
                };
                let created = create_runtime(
                    &parsed,
                    fixture.creation(),
                    &mut ModelResolverOutput {
                        stdout: &mut Vec::new(),
                        stderr: &mut Vec::new(),
                        color: false,
                    },
                )
                .await
                .unwrap();
                assert_eq!(
                    created
                        .session
                        .scoped_models()
                        .iter()
                        .map(|model| model.model.read().unwrap().id.clone())
                        .collect::<Vec<_>>(),
                    expected
                );
                created.session.dispose();
            }
            assert_mixed_patterns_fail(local).await;
        },
    );
}

/// Invalid stored members fail after the valid prefix reports its warnings.
async fn assert_mixed_patterns_fail(local: std::rc::Rc<tokio::task::LocalSet>) {
    use crate::startup_runtime::Fixture;
    use maestro_app::ModelResolverOutput;
    use maestro_cli::startup::main::runtime::create_runtime;
    let fixture = Fixture::new(
        local,
        startup::settings(&[(
            "enabledModels",
            json!(["unmatched-prefix", null, "startup/first"]),
        )]),
    )
    .await;
    let mut warnings = Vec::new();
    let result = create_runtime(
        &startup::args(&[]),
        fixture.creation(),
        &mut ModelResolverOutput {
            stdout: &mut Vec::new(),
            stderr: &mut warnings,
            color: false,
        },
    )
    .await;
    let Err(error) = result else {
        panic!("mixed patterns must fail");
    };
    assert_eq!(error.message, "Model pattern must be a string");
    let warnings = String::from_utf8(warnings).unwrap();
    assert!(warnings.contains("unmatched-prefix"));
    assert!(!warnings.contains("startup/first"));
}
