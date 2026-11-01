//! Completion witnesses through the interactive mode's own provider.

use super::{Setup, World, isolated, isolated_case, start};
use crate::interactive_mode::autocomplete::{
    get_autocomplete_source_tag, prefix_autocomplete_description,
};
use maestro_app::{
    DiagnosticErrorInfo, EventError, ExtensionCommandContext, Local, Model, PromptTemplate,
    ProviderConfigInput, ProviderModelInput, RegisteredCommand, ResolvedCommand, ScopedModel,
    SharedHandle, Skill, SourceInfo, SourceOrigin, SourceScope,
};
use maestro_tui::autocomplete::{AutocompleteSuggestions, CompletionOptions, CursorPosition};
use maestro_tui::{AutocompleteItem, AutocompleteProvider, Cancellation};
use serde::Deserialize;
use std::{
    cell::RefCell,
    future::Future,
    path::Path,
    pin::Pin,
    rc::Rc,
    sync::{Arc, RwLock},
};

#[test]
fn thinking_command_completion_includes_level_hint() {
    isolated(
        "thinking_command_completion_includes_level_hint",
        async |directory| {
            let world = Setup::default().build(directory);
            let suggestions = suggested(&provider(&world, &[]), "/thinking")
                .await
                .unwrap()
                .unwrap();
            assert_eq!(suggestions.items.len(), 1);
            let item = &suggestions.items[0];
            assert_eq!(item.value, "thinking");
            assert_eq!(item.label, "thinking");
            assert_eq!(
                item.description.as_deref(),
                Some("<level> — Set thinking level")
            );
        },
    );
}

/// Every recorded completion case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_extensions.json");

/// One recorded case of this module's witnesses.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// The witness the case belongs to.
    test: String,
    /// The recorded operation.
    input: Recorded,
    /// The recorded observation.
    expected: Expected,
}

/// The recorded operation; each witness reads the members it needs.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Recorded {
    /// Which operation was recorded.
    op: String,
    /// The provenance of one described resource.
    #[serde(default)]
    source_info: Option<Source>,
    /// The description the caller supplied, absent when it supplied none.
    #[serde(default)]
    description: Option<String>,
    /// Whether skill commands are enabled.
    #[serde(default)]
    enabled: Option<bool>,
    /// The loaded prompt templates.
    #[serde(default)]
    templates: Vec<Template>,
    /// The registered extension commands.
    #[serde(default)]
    extensions: Vec<ExtensionRecord>,
    /// The loaded skills.
    #[serde(default)]
    skills: Vec<SkillRecord>,
    /// The session's scoped models.
    #[serde(default)]
    scoped: Vec<Descriptor>,
    /// The catalog's available models.
    #[serde(default)]
    registry: Vec<Descriptor>,
    /// The typed prefix or argument text.
    #[serde(default)]
    query: Option<String>,
    /// The argument callback's result kind.
    #[serde(default)]
    outcome: Option<String>,
}

/// A recorded provenance, without the fields the witness never reads.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Source {
    /// Authored source label.
    source: String,
    /// Resource scope name.
    scope: String,
}

/// One recorded prompt template.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Template {
    /// Command name.
    name: String,
    /// Authored description.
    description: String,
    /// Optional argument hint.
    #[serde(default)]
    argument_hint: Option<String>,
    /// Provenance, absent in the reference's stub records.
    #[serde(default)]
    source_info: Option<Source>,
}

/// One recorded extension command.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExtensionRecord {
    /// Raw registered name.
    name: String,
    /// Name the command is invoked by.
    invocation_name: String,
    /// Authored description.
    #[serde(default)]
    description: Option<String>,
    /// Provenance, absent in the reference's stub records.
    #[serde(default)]
    source_info: Option<Source>,
}

/// One recorded skill.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SkillRecord {
    /// Skill name.
    name: String,
    /// Instruction file location.
    file_path: String,
    /// Authored description.
    #[serde(default)]
    description: Option<String>,
    /// Provenance, absent in the reference's stub records.
    #[serde(default)]
    source_info: Option<Source>,
}

/// One recorded model.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Descriptor {
    /// Model identity.
    id: String,
    /// Provider identity.
    provider: String,
}

/// The recorded observation.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Expected {
    /// The provenance marker.
    #[serde(default)]
    tag: Option<String>,
    /// The prefixed description.
    #[serde(default)]
    description: Option<String>,
    /// The candidate set, absent when the operation offers none.
    #[serde(default)]
    suggestions: Option<Suggestions>,
    /// The candidate set once the selected search program is gone.
    #[serde(default)]
    without_selected_executable: Option<Suggestions>,
    /// The prefix the argument callback received.
    #[serde(default)]
    called: Option<String>,
    /// The failure message of a rejected callback.
    #[serde(default)]
    error: Option<String>,
}

/// One recorded candidate set.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Suggestions {
    /// The candidates in order.
    items: Vec<Item>,
    /// The text the candidates replace.
    prefix: String,
}

/// One recorded candidate.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Item {
    /// Text applied when chosen.
    value: String,
    /// Text shown.
    label: String,
    /// Optional explanation.
    #[serde(default)]
    description: Option<String>,
}

/// The recorded cases of `test`, which all record the operation `op`.
fn cases(test: &str, op: &str) -> Vec<(Recorded, Expected)> {
    let all: Vec<Case> = serde_json::from_str(FIXTURES).unwrap();
    let cases: Vec<_> = all
        .into_iter()
        .filter(|case| case.test == test)
        .map(|case| (case.input, case.expected))
        .collect();
    assert!(!cases.is_empty(), "recorded cases of {test}");
    for (input, _) in &cases {
        assert_eq!(input.op, op, "the recorded operation of {test}");
    }
    cases
}

/// The one recorded case of `test`, which records the operation `op`.
fn case(test: &str, op: &str) -> (Recorded, Expected) {
    let mut cases = cases(test, op);
    assert_eq!(cases.len(), 1, "one recorded case of {test}");
    cases.remove(0)
}

/// The scope variant the recorded name selects.
fn scope(name: &str) -> SourceScope {
    match name {
        "user" => SourceScope::User,
        "project" => SourceScope::Project,
        "temporary" => SourceScope::Temporary,
        other => panic!("a recorded scope {other}"),
    }
}

/// The provenance a described resource carries, or none when the case recorded none.
fn provenance(source: Option<&Source>) -> Option<SourceInfo> {
    let source = source?;
    Some(SourceInfo {
        path: String::new(),
        source: source.source.clone(),
        scope: scope(&source.scope),
        origin: SourceOrigin::TopLevel,
        base_dir: None,
    })
}

/// The provenance of a typed record, which always carries one.
///
/// The reference's stub records could omit provenance; the typed owner supplies an empty
/// project provenance instead.
fn typed_provenance(source: Option<&Source>) -> SourceInfo {
    provenance(source).unwrap_or(SourceInfo {
        path: String::new(),
        source: String::new(),
        scope: SourceScope::Project,
        origin: SourceOrigin::TopLevel,
        base_dir: None,
    })
}

/// The prompt template one recorded case loads.
fn template(record: &Template) -> PromptTemplate {
    PromptTemplate {
        name: record.name.clone(),
        description: record.description.clone(),
        argument_hint: record.argument_hint.clone(),
        content: String::new(),
        source_info: typed_provenance(record.source_info.as_ref()),
        file_path: String::new(),
    }
}

/// The skill one recorded case loads.
fn skill(record: &SkillRecord) -> Skill {
    Skill {
        name: record.name.clone(),
        description: record.description.clone().unwrap_or_default(),
        file_path: record.file_path.clone(),
        base_dir: String::new(),
        source_info: typed_provenance(record.source_info.as_ref()),
        disable_model_invocation: false,
    }
}

/// The argument callback of a registered command.
type Completions = Rc<
    dyn Fn(
            String,
        )
            -> Pin<Box<dyn Future<Output = Result<Option<Vec<AutocompleteItem>>, EventError>>>>
        + 'static,
>;

/// The command handler of a registered command; these witnesses never invoke it.
type Handler = Rc<
    dyn Fn(
        String,
        ExtensionCommandContext,
    ) -> Pin<Box<dyn Future<Output = Result<(), EventError>>>>,
>;

/// The registration one recorded extension command describes.
fn registered(
    record: &ExtensionRecord,
    completions: Option<Completions>,
) -> ResolvedCommand<Local> {
    let handler: Handler = Rc::new(|_, _| Box::pin(async { Ok(()) }));
    ResolvedCommand {
        command: RegisteredCommand {
            name: record.name.clone(),
            source_info: Arc::new(RwLock::new(typed_provenance(record.source_info.as_ref()))),
            description: record.description.clone(),
            get_argument_completions: completions,
            handler,
        },
        invocation_name: record.invocation_name.clone(),
    }
}

/// The provider the mode builds over `records`.
fn provider(
    world: &World,
    records: &[ResolvedCommand<Local>],
) -> Rc<dyn AutocompleteProvider<Signal = Cancellation>> {
    world.mode.create_base_autocomplete_provider(records)
}

/// The suggestion set one authored line produces.
async fn suggested(
    provider: &Rc<dyn AutocompleteProvider<Signal = Cancellation>>,
    line: &str,
) -> Result<Option<AutocompleteSuggestions>, maestro_tui::autocomplete::CompletionError> {
    let signal = Cancellation::new();
    let lines = [line.to_owned()];
    let cursor = CursorPosition {
        line: 0,
        col: line.len(),
    };
    provider
        .get_suggestions(
            &lines,
            cursor,
            CompletionOptions {
                signal: &signal,
                force: None,
            },
        )
        .await
}

/// Asserts one observed candidate set against its recorded candidates.
fn recorded(observed: Option<AutocompleteSuggestions>, expected: Option<&Suggestions>) {
    let Some(expected) = expected else {
        assert!(observed.is_none(), "no candidate set was recorded");
        return;
    };
    let observed = observed.expect("a recorded candidate set");
    assert_eq!(observed.prefix, expected.prefix);
    let items: Vec<_> = observed
        .items
        .into_iter()
        .map(|item| (item.value, item.label, item.description))
        .collect();
    let wanted: Vec<_> = expected
        .items
        .iter()
        .map(|item| {
            (
                item.value.clone(),
                item.label.clone(),
                item.description.clone(),
            )
        })
        .collect();
    assert_eq!(items, wanted);
}

/// The world one recorded base case builds.
fn base_world(directory: &Path, input: &Recorded) -> World {
    let enabled = input.enabled;
    let prompts = input.templates.iter().map(template).collect();
    let skills = input.skills.iter().map(skill).collect();
    Setup {
        configure: Box::new(move |settings| {
            if let Some(enabled) = enabled {
                settings.set_enable_skill_commands(enabled);
            }
        }),
        prompts: vec![prompts],
        skills,
        ..Setup::default()
    }
    .build(directory)
}

/// The extension records one recorded base case registers.
fn base_records(input: &Recorded) -> Vec<ResolvedCommand<Local>> {
    input
        .extensions
        .iter()
        .map(|record| registered(record, None))
        .collect()
}

/// The world one recorded model case builds, with its scoped and available models.
fn model_world(directory: &Path, input: &Recorded) -> World {
    let world = Setup::default().build(directory);
    register_models(&world, &input.registry);
    let scoped: Vec<ScopedModel> = input
        .scoped
        .iter()
        .map(|descriptor| ScopedModel {
            model: catalog_model(&world, descriptor),
            thinking_level: None,
        })
        .collect();
    world.mode.runtime_host.session().set_scoped_models(scoped);
    world
}

/// One catalog model carrying the recorded identity and provider.
fn catalog_model(world: &World, descriptor: &Descriptor) -> Arc<RwLock<Model>> {
    let session = world.mode.runtime_host.session();
    let all = session.model_registry().get_all();
    let all = all.read().unwrap();
    let mut model = all[0].read().unwrap().clone();
    model.provider.clone_from(&descriptor.provider);
    model.id.clone_from(&descriptor.id);
    model.name.clone_from(&descriptor.id);
    Arc::new(RwLock::new(model))
}

/// Registers `descriptors` as the catalog's available models, one provider at a time.
fn register_models(world: &World, descriptors: &[Descriptor]) {
    let catalog = world.mode.runtime_host.session().model_registry().clone();
    let mut providers: Vec<(String, Vec<ProviderModelInput>)> = Vec::new();
    for descriptor in descriptors {
        let input = ProviderModelInput {
            id: descriptor.id.clone(),
            name: descriptor.id.clone(),
            api: None,
            base_url: Some("https://example.invalid".to_owned()),
            reasoning: false,
            thinking_level_map: None,
            input: Vec::new(),
            cost: maestro_app::ModelCost::default(),
            context_window: 0.0,
            max_tokens: 0.0,
            headers: None,
            compat: None,
        };
        match providers
            .iter_mut()
            .find(|(provider, _)| *provider == descriptor.provider)
        {
            Some((_, models)) => models.push(input),
            None => providers.push((descriptor.provider.clone(), vec![input])),
        }
    }
    for (name, models) in providers {
        catalog
            .register_provider(
                &name,
                SharedHandle::new(RwLock::new(ProviderConfigInput {
                    base_url: Some("https://example.invalid".to_owned()),
                    api_key: Some("fixture-key".to_owned()),
                    api: Some("openai-completions".into()),
                    models: Some(models),
                    ..ProviderConfigInput::default()
                })),
            )
            .unwrap();
    }
}

/// The candidate values of one suggestion set.
fn values(observed: Option<&AutocompleteSuggestions>) -> Vec<String> {
    observed
        .map(|suggestions| {
            suggestions
                .items
                .iter()
                .map(|item| item.value.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// The base provider one recorded case builds, in its own world.
fn case_provider(
    directory: &Path,
    input: &Recorded,
) -> (World, Rc<dyn AutocompleteProvider<Signal = Cancellation>>) {
    let world = base_world(directory, input);
    let completer = provider(&world, &base_records(input));
    (world, completer)
}

/// Asserts every recorded base case of `test` against its own provider.
async fn assert_base_cases(test: &str, directory: &Path) {
    for (input, expected) in cases(test, "base") {
        let (_, completer) = case_provider(directory, &input);
        recorded(
            suggested(&completer, "/").await.unwrap(),
            expected.suggestions.as_ref(),
        );
    }
}

#[test]
fn autocomplete_descriptions_keep_scope_git_npm_and_ecmascript_whitespace() {
    let test = "autocomplete_descriptions_keep_scope_git_npm_and_ecmascript_whitespace";
    let cases = cases(test, "description");
    assert_eq!(cases.len(), 117);
    for (input, expected) in cases {
        let info = provenance(input.source_info.as_ref());
        let source = input
            .source_info
            .as_ref()
            .map(|source| source.source.clone());
        assert_eq!(
            get_autocomplete_source_tag(info.as_ref()),
            expected.tag,
            "{source:?}"
        );
        assert_eq!(
            prefix_autocomplete_description(input.description.as_deref(), info.as_ref()),
            expected.description,
            "{source:?}"
        );
    }
}

#[test]
fn autocomplete_descriptions_preserve_missing_source_and_empty_description() {
    let test = "autocomplete_descriptions_preserve_missing_source_and_empty_description";
    for (input, expected) in cases(test, "description") {
        assert!(input.source_info.is_none());
        assert_eq!(get_autocomplete_source_tag(None), expected.tag);
        assert_eq!(
            prefix_autocomplete_description(input.description.as_deref(), None),
            expected.description
        );
    }
}

#[test]
fn autocomplete_builds_ordered_builtin_template_extension_and_enabled_skill_commands() {
    let test = "autocomplete_builds_ordered_builtin_template_extension_and_enabled_skill_commands";
    isolated(test, async |directory| {
        assert_base_cases(test, directory).await;
    });
}

#[test]
fn autocomplete_skill_map_rebuild_discards_disabled_and_missing_entries() {
    let test = "autocomplete_skill_map_rebuild_discards_disabled_and_missing_entries";
    isolated(test, async |directory| {
        let (input, expected) = case(test, "base");
        let (world, completer) = case_provider(directory, &input);
        recorded(
            suggested(&completer, "/").await.unwrap(),
            expected.suggestions.as_ref(),
        );
        world
            .mode
            .runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .set_enable_skill_commands(false);
        let rebuilt = provider(&world, &base_records(&input));
        let values = values(suggested(&rebuilt, "/").await.unwrap().as_ref());
        assert!(
            values.iter().all(|value| !value.starts_with("skill:")),
            "{values:?}"
        );
    });
}

#[test]
fn autocomplete_command_sources_preserve_duplicate_entries() {
    let test = "autocomplete_command_sources_preserve_duplicate_entries";
    isolated(test, async |directory| {
        assert_base_cases(test, directory).await;
    });
}

#[test]
fn autocomplete_empty_resources_keep_builtin_commands() {
    let test = "autocomplete_empty_resources_keep_builtin_commands";
    isolated(test, async |directory| {
        assert_base_cases(test, directory).await;
    });
}

#[test]
fn model_arguments_use_live_scoped_or_registry_candidates_and_fuzzy_provider_id() {
    let test = "model_arguments_use_live_scoped_or_registry_candidates_and_fuzzy_provider_id";
    isolated(test, async |directory| {
        for (input, expected) in cases(test, "model") {
            let query = input.query.clone().unwrap_or_default();
            let world = model_world(directory, &input);
            let provider = provider(&world, &[]);
            let line = format!("/model {query}");
            recorded(
                suggested(&provider, &line).await.unwrap(),
                expected.suggestions.as_ref(),
            );
        }
    });
}

#[test]
fn extension_argument_callback_receives_the_typed_prefix() {
    let test = "extension_argument_callback_receives_the_typed_prefix";
    isolated(test, async |directory| {
        for (input, expected) in cases(test, "extension-argument") {
            let outcome = input
                .outcome
                .clone()
                .unwrap_or_else(|| "success".to_owned());
            let called = Rc::new(RefCell::new(Vec::new()));
            let record = output_record(&called, &outcome);
            let world = Setup::default().build(directory);
            let provider = provider(&world, &[record]);
            let result = suggested(&provider, "/addon value").await;
            assert_eq!(
                called.borrow().as_slice(),
                [expected.called.clone().unwrap()]
            );
            match expected.error {
                Some(message) => assert_eq!(result.unwrap_err().to_string(), message),
                None => recorded(result.unwrap(), expected.suggestions.as_ref()),
            }
        }
    });
}

/// The registration whose argument callback records its prefix and answers with `outcome`.
fn output_record(called: &Rc<RefCell<Vec<String>>>, outcome: &str) -> ResolvedCommand<Local> {
    let called = Rc::clone(called);
    let outcome = outcome.to_owned();
    let completions: Completions = Rc::new(move |prefix: String| {
        called.borrow_mut().push(prefix);
        let outcome = outcome.clone();
        Box::pin(async move {
            match outcome.as_str() {
                "none" => Ok(None),
                "empty" => Ok(Some(Vec::new())),
                "success" => Ok(Some(vec![AutocompleteItem {
                    value: "arg".to_owned(),
                    label: "Argument".to_owned(),
                    description: Some("from callback".to_owned()),
                }])),
                _ => Err("argument failed".into()),
            }
        })
    });
    let record = ExtensionRecord {
        name: "addon".to_owned(),
        invocation_name: "addon".to_owned(),
        description: None,
        source_info: None,
    };
    registered(&record, Some(completions))
}

#[test]
fn model_completion_propagates_catalog_credential_failure() {
    let test = "model_completion_propagates_catalog_credential_failure";
    isolated(test, async |directory| {
        let world = Setup::default().build(directory);
        let descriptor = Descriptor {
            id: "unreachable".to_owned(),
            provider: "maestro-failing-fallback".to_owned(),
        };
        register_models(&world, std::slice::from_ref(&descriptor));
        let registry = world.mode.runtime_host.session().model_registry().clone();
        registry.auth_storage().set_fallback_resolver(|_| {
            Err(DiagnosticErrorInfo::new(None, "Credential fallback failed").into())
        });
        let provider = provider(&world, &[]);
        let observed = suggested(&provider, "/model unreachable").await;
        assert!(
            observed
                .as_ref()
                .is_err_and(|error| error.to_string() == "Credential fallback failed"),
            "{observed:?}"
        );
    });
}

#[test]
fn model_completion_reads_the_session_at_request_time() {
    let test = "model_completion_reads_the_session_at_request_time";
    isolated(test, async |directory| {
        let (input, expected) = case(test, "live-model");
        let world = Setup::default().build(directory);
        let before = Descriptor {
            id: "before".to_owned(),
            provider: "a".to_owned(),
        };
        world
            .mode
            .runtime_host
            .session()
            .set_scoped_models(vec![ScopedModel {
                model: catalog_model(&world, &before),
                thinking_level: None,
            }]);
        let provider = provider(&world, &[]);
        let after = Descriptor {
            id: "after".to_owned(),
            provider: "b".to_owned(),
        };
        world
            .mode
            .runtime_host
            .session()
            .set_scoped_models(vec![ScopedModel {
                model: catalog_model(&world, &after),
                thinking_level: None,
            }]);
        let query = input.query.unwrap_or_else(|| "after".to_owned());
        let line = format!("/model {query}");
        recorded(
            suggested(&provider, &line).await.unwrap(),
            expected.suggestions.as_ref(),
        );
    });
}

#[test]
fn autocomplete_handles_survive_mode_release_without_retaining_mode_cycles() {
    let test = "autocomplete_handles_survive_mode_release_without_retaining_mode_cycles";
    isolated(test, async |directory| {
        let world = Setup::default().build(directory);
        let descriptor = Descriptor {
            id: "kept".to_owned(),
            provider: "alpha".to_owned(),
        };
        world
            .mode
            .runtime_host
            .session()
            .set_scoped_models(vec![ScopedModel {
                model: catalog_model(&world, &descriptor),
                thinking_level: None,
            }]);
        let provider = world.mode.create_base_autocomplete_provider(&[]);
        let weak = Rc::downgrade(&world.mode);
        let World {
            mode,
            terminal,
            runtime,
            themes,
            host,
            loader,
        } = world;
        drop(mode);
        assert!(weak.upgrade().is_none(), "the provider retains the mode");
        let observed = suggested(&provider, "/model kept").await.unwrap();
        assert_eq!(values(observed.as_ref()), ["alpha/kept"]);
        drop((terminal, runtime, themes, host, loader, provider));
    });
}

/// Installs `text` as an executable through a child process, never in-process.
///
/// The probe commands precede the shell commands; the installing child receives the
/// path the parent inherited under its own name.
fn script(path: &Path, text: &str) {
    let tool_path = std::env::var("MAESTRO_TEST_TOOL_PATH").unwrap_or_default();
    let source = path.with_extension("source");
    std::fs::write(&source, text).unwrap();
    let status = std::process::Command::new("install")
        .args(["-m", "0700"])
        .arg(&source)
        .arg(path)
        .env("PATH", tool_path)
        .status()
        .unwrap_or_else(|error| panic!("install runs: {error}"));
    assert!(status.success(), "the fixture executable installs");
}

/// The handshake probe both search programs run when the other must be in flight.
fn handshake(marker: &Path, own: &str, other: &str) -> String {
    format!(
        "#!/bin/sh\n: > '{}/{}'\ni=0\nwhile [ ! -f '{}/{}' ]; do\ni=$((i+1))\n[ \"$i\" -gt 200000 ] && exit 1\ndone\nexit 0\n",
        marker.display(),
        own,
        marker.display(),
        other
    )
}

#[test]
fn interactive_initialization_prepares_search_tools_before_first_completion() {
    let test = "interactive_initialization_prepares_search_tools_before_first_completion";
    let tool_path = std::env::var("PATH").unwrap_or_default();
    isolated_case(
        test,
        "managed",
        &[
            ("PATH", "probes:/bin"),
            ("MAESTRO_OFFLINE", "1"),
            ("MAESTRO_TEST_TOOL_PATH", &tool_path),
        ],
        async |directory| {
            let managed = directory.join("bin");
            std::fs::create_dir_all(&managed).unwrap();
            script(&managed.join("fd"), "#!/bin/sh\nexit 0\n");
            let elsewhere = directory.join("elsewhere");
            std::fs::create_dir(&elsewhere).unwrap();
            std::fs::create_dir(elsewhere.join("bin")).unwrap();
            script(&elsewhere.join("bin").join("fd"), "#!/bin/sh\nexit 1\n");
            let world = Setup {
                cwd: Some(elsewhere.to_str().unwrap().to_owned()),
                ..Setup::default()
            }
            .build(directory);
            start(&world).await;
            assert_eq!(
                world.mode.fd_path.borrow().as_deref(),
                Some(managed.join("fd").to_str().unwrap()),
                "the managed binary directory supplies the search program"
            );
            std::fs::remove_file(managed.join("fd")).unwrap();
            start(&world).await;
            assert!(
                world.mode.fd_path.borrow().is_some(),
                "a repeated initialization keeps the prepared program"
            );
        },
    );
}

#[test]
fn interactive_initialization_starts_both_search_programs_before_either_finishes() {
    let test = "interactive_initialization_starts_both_search_programs_before_either_finishes";
    let tool_path = std::env::var("PATH").unwrap_or_default();
    isolated_case(
        test,
        "child",
        &[
            ("PATH", "probes:/bin"),
            ("MAESTRO_OFFLINE", "1"),
            ("MAESTRO_TEST_TOOL_PATH", &tool_path),
        ],
        async |directory| {
            let probes = directory.join("probes");
            std::fs::create_dir(&probes).unwrap();
            let marker = directory.join("markers");
            std::fs::create_dir(&marker).unwrap();
            script(
                &probes.join("fd"),
                &handshake(&marker, "fd.started", "rg.started"),
            );
            script(
                &probes.join("rg"),
                &handshake(&marker, "rg.started", "fd.started"),
            );
            let world = Setup::default().build(directory);
            start(&world).await;
            assert!(marker.join("fd.started").exists());
            assert!(marker.join("rg.started").exists());
            assert_eq!(world.mode.fd_path.borrow().as_deref(), Some("fd"));
        },
    );
}

#[test]
fn file_completion_consumes_the_search_owner_fd_path() {
    let test = "file_completion_consumes_the_search_owner_fd_path";
    let tool_path = std::env::var("PATH").unwrap_or_default();
    isolated_case(
        test,
        "child",
        &[
            ("PATH", "probes:/bin"),
            ("MAESTRO_OFFLINE", "1"),
            ("MAESTRO_TEST_TOOL_PATH", &tool_path),
        ],
        async |directory| {
            let (input, expected) = case(test, "fd-route");
            let query = input.query.unwrap();
            let managed = directory.join("bin");
            std::fs::create_dir_all(&managed).unwrap();
            script(
                &managed.join("fd"),
                "#!/bin/sh\nprintf 'target-unique.rs\\000'\n",
            );
            let world = Setup::default().build(directory);
            start(&world).await;
            assert!(
                world.mode.fd_path.borrow().is_some(),
                "the managed program is prepared"
            );
            let completer = provider(&world, &[]);
            let observed = suggested(&completer, &query).await.unwrap();
            recorded(observed.clone(), expected.suggestions.as_ref());
            let item = &observed.unwrap().items[0];
            let applied = completer.apply_completion(
                std::slice::from_ref(&query),
                CursorPosition {
                    line: 0,
                    col: query.len(),
                },
                item,
                &query,
            );
            assert_eq!(applied.lines, ["@target-unique.rs "]);
            std::fs::remove_file(managed.join("fd")).unwrap();
            let world = Setup::default().build(directory);
            start(&world).await;
            let completer = provider(&world, &[]);
            let without = suggested(&completer, &query).await.unwrap();
            recorded(without, expected.without_selected_executable.as_ref());
        },
    );
}

#[test]
fn search_preparation_honors_selected_offline_policy() {
    search_download_policy("search_preparation_honors_selected_offline_policy", false);
}

#[test]
fn interactive_initialization_honors_selected_offline_policy() {
    search_download_policy(
        "interactive_initialization_honors_selected_offline_policy",
        true,
    );
}

/// Exercises missing helpers through preparation or initialization in checked children.
fn search_download_policy(test: &str, initialize: bool) {
    for (case, offline, environment, expected) in [
        ("flag", Some(true), "", 0),
        ("online", Some(false), "", 2),
        ("environment", None, "1", 0),
    ] {
        let mut envs = vec![("PATH", "missing-helpers")];
        if !environment.is_empty() {
            envs.push(("MAESTRO_OFFLINE", environment));
        }
        isolated_case(test, case, &envs, async |directory| {
            assert_eq!(
                std::env::var("MAESTRO_OFFLINE").ok().as_deref(),
                (!environment.is_empty()).then_some(environment)
            );
            let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
            let observed = Arc::clone(&requests);
            let world = Setup {
                offline,
                options: crate::InteractiveModeOptions {
                    fetch: Some(rejected_search_fetch(observed)),
                    ..Default::default()
                },
                ..Setup::default()
            }
            .build(directory);
            if initialize {
                start(&world).await;
            } else {
                world.mode.prepare_search_tools().await.unwrap();
            }
            let requests = requests.lock().unwrap();
            assert_eq!(
                requests.len(),
                expected,
                "unexpected search acquisitions: {requests:?}"
            );
            if expected == 2 {
                assert!(requests.iter().any(|url| url.contains("/sharkdp/fd/")));
                assert!(
                    requests
                        .iter()
                        .any(|url| url.contains("/BurntSushi/ripgrep/"))
                );
            }
            assert!(world.mode.fd_path.borrow().is_none());
        });
    }
}

/// Records release requests without contacting a server.
fn rejected_search_fetch(requests: Arc<std::sync::Mutex<Vec<String>>>) -> maestro_app::Fetch {
    Arc::new(move |request| {
        requests.lock().unwrap().push(request.url);
        Box::pin(async { Err(maestro_app::FetchError::Timeout) })
    })
}
