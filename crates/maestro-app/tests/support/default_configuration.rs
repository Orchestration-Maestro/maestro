//! Recorded caller inputs and expectations, with disposable file setup.
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;

/// Named qualification group.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum Group {
    /// The externally removed packages survive unrelated write observation.
    #[serde(rename = "externally_removed_packages_survive_unrelated_write")]
    ExternallyRemovedPackagesSurviveUnrelatedWrite,
    /// The external extensions survive unrelated write observation.
    #[serde(rename = "external_extensions_survive_unrelated_write")]
    ExternalExtensionsSurviveUnrelatedWrite,
    /// The external project prompts survive project extension write observation.
    #[serde(rename = "external_project_prompts_survive_project_extension_write")]
    ExternalProjectPromptsSurviveProjectExtensionWrite,
    /// The project edits win over external same field observation.
    #[serde(rename = "project_edits_win_over_external_same_field")]
    ProjectEditsWinOverExternalSameField,
    /// The external enabled models survive thinking write observation.
    #[serde(rename = "external_enabled_models_survive_thinking_write")]
    ExternalEnabledModelsSurviveThinkingWrite,
    /// The external custom settings survive theme write observation.
    #[serde(rename = "external_custom_settings_survive_theme_write")]
    ExternalCustomSettingsSurviveThemeWrite,
    /// The global edits win over external same field observation.
    #[serde(rename = "global_edits_win_over_external_same_field")]
    GlobalEditsWinOverExternalSameField,
    /// The local extensions remain local observation.
    #[serde(rename = "local_extensions_remain_local")]
    LocalExtensionsRemainLocal,
    /// The filtered package records remain intact observation.
    #[serde(rename = "filtered_package_records_remain_intact")]
    FilteredPackageRecordsRemainIntact,
    /// The reload accepts external global changes observation.
    #[serde(rename = "reload_accepts_external_global_changes")]
    ReloadAcceptsExternalGlobalChanges,
    /// The reload retains previous invalid scope observation.
    #[serde(rename = "reload_retains_previous_invalid_scope")]
    ReloadRetainsPreviousInvalidScope,
    /// The load errors drain once for both scopes observation.
    #[serde(rename = "load_errors_drain_once_for_both_scopes")]
    LoadErrorsDrainOnceForBothScopes,
    /// The reading missing project config creates nothing observation.
    #[serde(rename = "reading_missing_project_config_creates_nothing")]
    ReadingMissingProjectConfigCreatesNothing,
    /// The writing project config creates selected directory observation.
    #[serde(rename = "writing_project_config_creates_selected_directory")]
    WritingProjectConfigCreatesSelectedDirectory,
    /// The shell prefix loads from selected root observation.
    #[serde(rename = "shell_prefix_loads_from_selected_root")]
    ShellPrefixLoadsFromSelectedRoot,
    /// The missing shell prefix stays absent observation.
    #[serde(rename = "missing_shell_prefix_stays_absent")]
    MissingShellPrefixStaysAbsent,
    /// The shell prefix survives unrelated save observation.
    #[serde(rename = "shell_prefix_survives_unrelated_save")]
    ShellPrefixSurvivesUnrelatedSave,
    /// The missing session directory stays absent observation.
    #[serde(rename = "missing_session_directory_stays_absent")]
    MissingSessionDirectoryStaysAbsent,
    /// The global session directory is returned observation.
    #[serde(rename = "global_session_directory_is_returned")]
    GlobalSessionDirectoryIsReturned,
    /// The project session directory overrides global observation.
    #[serde(rename = "project_session_directory_overrides_global")]
    ProjectSessionDirectoryOverridesGlobal,
    /// The session directory tilde uses isolated home observation.
    #[serde(rename = "session_directory_tilde_uses_isolated_home")]
    SessionDirectoryTildeUsesIsolatedHome,
    /// The nested scope merge and writes preserve unedited members observation.
    #[serde(rename = "nested_scope_merge_and_writes_preserve_unedited_members")]
    NestedScopeMergeAndWritesPreserveUneditedMembers,
    /// The defaults and owned reads survive application root selection observation.
    #[serde(rename = "defaults_and_owned_reads_survive_application_root_selection")]
    DefaultsAndOwnedReadsSurviveApplicationRootSelection,
    /// The empty session and shell values remain present observation.
    #[serde(rename = "empty_session_and_shell_values_remain_present")]
    EmptySessionAndShellValuesRemainPresent,
    /// The session directory path boundaries use the existing owner observation.
    #[serde(rename = "session_directory_path_boundaries_use_the_existing_owner")]
    SessionDirectoryPathBoundariesUseTheExistingOwner,
    /// The invalid scope reload preserves other healthy scope observation.
    #[serde(rename = "invalid_scope_reload_preserves_other_healthy_scope")]
    InvalidScopeReloadPreservesOtherHealthyScope,
    /// The typed wrong values fall back without rewriting raw settings observation.
    #[serde(rename = "typed_wrong_values_fall_back_without_rewriting_raw_settings")]
    TypedWrongValuesFallBackWithoutRewritingRawSettings,
    /// The required hint survives resolved prompt discovery observation.
    #[serde(rename = "required_hint_survives_resolved_prompt_discovery")]
    RequiredHintSurvivesResolvedPromptDiscovery,
    /// The optional hint survives resolved prompt discovery observation.
    #[serde(rename = "optional_hint_survives_resolved_prompt_discovery")]
    OptionalHintSurvivesResolvedPromptDiscovery,
    /// The missing hint stays absent after discovery observation.
    #[serde(rename = "missing_hint_stays_absent_after_discovery")]
    MissingHintStaysAbsentAfterDiscovery,
    /// The empty hint stays absent after discovery observation.
    #[serde(rename = "empty_hint_stays_absent_after_discovery")]
    EmptyHintStaysAbsentAfterDiscovery,
    /// The special hint survives resolved prompt discovery observation.
    #[serde(rename = "special_hint_survives_resolved_prompt_discovery")]
    SpecialHintSurvivesResolvedPromptDiscovery,
    /// The discovered prompt arguments stay literal observation.
    #[serde(rename = "discovered_prompt_arguments_stay_literal")]
    DiscoveredPromptArgumentsStayLiteral,
    /// The resolved roots feed native and controlled consumers observation.
    #[serde(rename = "resolved_roots_feed_native_and_controlled_consumers")]
    ResolvedRootsFeedNativeAndControlledConsumers,
    /// The explicit root presence overrides environment observation.
    #[serde(rename = "explicit_root_presence_overrides_environment")]
    ExplicitRootPresenceOverridesEnvironment,
    /// The package and binary paths reach real files observation.
    #[serde(rename = "package_and_binary_paths_reach_real_files")]
    PackageAndBinaryPathsReachRealFiles,
}
/// Complete recorded query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case<I, E> {
    /// Named operation group.
    pub test: Group,
    /// Operands for the public operation.
    pub input: I,
    /// Independently recorded observations.
    pub expected: E,
}
/// Decode only one group, rejecting duplicate complete queries.
///
/// # Panics
/// Panics when fixture admission, setup or a recorded observation fails.
#[must_use]
pub fn cases<I: serde::de::DeserializeOwned, E: serde::de::DeserializeOwned>(
    group: Group,
) -> Vec<Case<I, E>> {
    let all: Vec<Case<Value, Value>> =
        serde_json::from_str(include_str!("../fixtures/default_configuration.json")).unwrap();
    assert_eq!(all.len(), 56);
    let mut queries = std::collections::HashSet::new();
    let mut groups = std::collections::HashSet::new();
    let mut selected = Vec::new();
    for case in all {
        assert!(queries.insert(serde_json::to_string(&case.input).unwrap()));
        groups.insert(format!("{:?}", case.test));
        if case.test == group {
            selected.push(Case {
                test: case.test,
                input: serde_json::from_value(case.input).unwrap(),
                expected: serde_json::from_value(case.expected).unwrap(),
            });
        }
    }
    assert_eq!(groups.len(), 36);
    assert!(!selected.is_empty());
    selected
}
/// Create a new fixture file without truncating an existing one.
///
/// # Panics
/// Panics when fixture admission, setup or a recorded observation fails.
pub fn write(path: impl AsRef<Path>, text: &str) {
    use std::io::Write;
    let path = path.as_ref();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
}
/// Instantiate disposable path tokens without altering other values.
///
/// # Panics
/// Panics when fixture admission, setup or a recorded observation fails.
#[must_use]
pub fn instantiate<T: serde::de::DeserializeOwned>(value: &Value, root: &str, package: &str) -> T {
    /// Substitute tokens recursively in fixture strings, leaving keys untouched.
    fn visit(value: &mut Value, root: &str, package: &str) {
        match value {
            Value::String(s) => *s = s.replace("$ROOT", root).replace("$PACKAGE", package),
            Value::Array(a) => a.iter_mut().for_each(|v| visit(v, root, package)),
            Value::Object(o) => o.values_mut().for_each(|v| visit(v, root, package)),
            _ => {}
        }
    }
    let mut value = value.clone();
    visit(&mut value, root, package);
    serde_json::from_value(value).unwrap()
}

/// Shared subprocess owner.
pub(crate) use crate::child;
/// Shared current-thread executor.
use crate::support::browser_providers as runtime;
use maestro_app::SettingsManager;
use maestro_settings::{
    InMemorySettingsStorage, PackageSource, SettingsListEntry, SettingsScope, SettingsStorage,
};
use std::sync::Arc;

/// Environment keys removed from each native qualification child.
pub const REMOVED: &[&str] = &[
    "MAESTRO_CODING_AGENT_DIR",
    "MAESTRO_PACKAGE_DIR",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "MAESTRO_CODING_AGENT_SESSION_DIR",
];
/// Selected typed getter.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum Read {
    /// The packages selection.
    #[serde(rename = "getPackages")]
    Packages,
    /// The theme selection.
    #[serde(rename = "getTheme")]
    Theme,
    /// The default thinking level selection.
    #[serde(rename = "getDefaultThinkingLevel")]
    DefaultThinkingLevel,
    /// The extension paths selection.
    #[serde(rename = "getExtensionPaths")]
    ExtensionPaths,
    /// The default model selection.
    #[serde(rename = "getDefaultModel")]
    DefaultModel,
    /// The shell command prefix selection.
    #[serde(rename = "getShellCommandPrefix")]
    ShellCommandPrefix,
    /// The session dir selection.
    #[serde(rename = "getSessionDir")]
    SessionDir,
    /// The retry settings selection.
    #[serde(rename = "getRetrySettings")]
    RetrySettings,
    /// The compaction settings selection.
    #[serde(rename = "getCompactionSettings")]
    CompactionSettings,
}
/// Selected setter.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    /// The theme selection.
    #[serde(rename = "setTheme")]
    Theme,
    /// The default thinking level selection.
    #[serde(rename = "setDefaultThinkingLevel")]
    DefaultThinkingLevel,
    /// The project extension paths selection.
    #[serde(rename = "setProjectExtensionPaths")]
    ProjectExtensionPaths,
    /// The project packages selection.
    #[serde(rename = "setProjectPackages")]
    ProjectPackages,
    /// The retry enabled selection.
    #[serde(rename = "setRetryEnabled")]
    RetryEnabled,
    /// The quiet startup selection.
    #[serde(rename = "setQuietStartup")]
    QuietStartup,
}
/// Settings documents and operation phases.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SettingsInput {
    /// Initial global document.
    pub global: Option<Value>,
    /// Initial project document.
    pub project: Option<Value>,
    /// Malformed global input kept byte-for-byte.
    pub global_text: Option<String>,
    /// Malformed project input kept byte-for-byte.
    pub project_text: Option<String>,
    /// Global document externally replaced after loading.
    pub external_global: Option<Value>,
    /// Project document externally replaced after loading.
    pub external_project: Option<Value>,
    /// Malformed external replacement.
    pub external_global_text: Option<String>,
    /// Ordered setter calls after the external edits.
    #[serde(default)]
    pub actions: Vec<(Action, Value)>,
    /// Typed getters observed before and after the operation.
    pub reads: Vec<Read>,
    /// Whether to reload after external replacement.
    #[serde(default)]
    pub reload: bool,
    /// Whether the project directory initially stays absent.
    #[serde(default)]
    pub project_missing: bool,
}
/// Settings phase observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SettingsExpected {
    /// Typed reads immediately after construction.
    pub before: std::collections::BTreeMap<Read, Value>,
    /// Typed reads after reload and completed writes.
    pub observed: std::collections::BTreeMap<Read, Value>,
    /// Global document after the completed operation.
    pub global: Value,
    /// Project document after the completed operation.
    pub project: Value,
    /// Error scopes in drain order.
    pub errors: Vec<Scope>,
    /// Whether the native project directory exists after completion.
    pub project_directory_exists: bool,
}
/// Error scope in drain order.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// The global selection.
    Global,
    /// The project selection.
    Project,
}
/// Retain every field of a typed package projection.
fn package(value: PackageSource) -> Value {
    match value {
        PackageSource::Source(s) => Value::String(s),
        PackageSource::Filtered(p) => Value::Object(p.0),
        PackageSource::Unknown(v) => v,
    }
}
/// Retain every list member, including repeated values.
fn entry(value: SettingsListEntry) -> Value {
    match value {
        SettingsListEntry::String(s) => Value::String(s),
        SettingsListEntry::Unknown(v) => v,
    }
}
/// Represent a resolved numeric setting without an incidental float spelling.
fn number(value: f64) -> Value {
    serde_json::from_str(&value.to_string()).unwrap()
}
/// Observe the public typed getter, not the underlying raw setting.
fn read(manager: &SettingsManager, key: Read) -> Value {
    use serde_json::json;
    match key {
        Read::Packages => Value::Array(manager.get_packages().into_iter().map(package).collect()),
        Read::ExtensionPaths => Value::Array(
            manager
                .get_extension_paths()
                .into_iter()
                .map(entry)
                .collect(),
        ),
        Read::Theme => json!(manager.get_theme()),
        Read::DefaultThinkingLevel => json!(
            manager
                .get_default_thinking_level()
                .map(|v| v.as_str().to_owned())
        ),
        Read::DefaultModel => json!(manager.get_default_model()),
        Read::ShellCommandPrefix => json!(manager.get_shell_command_prefix()),
        Read::SessionDir => json!(manager.get_session_dir()),
        Read::RetrySettings => {
            let v = manager.get_retry_settings();
            json!({"enabled":v.enabled,"maxRetries":number(v.max_retries),"baseDelayMs":number(v.base_delay_ms)})
        }
        Read::CompactionSettings => {
            let v = manager.get_compaction_settings();
            json!({"enabled":v.enabled,"reserveTokens":number(v.reserve_tokens),"keepRecentTokens":number(v.keep_recent_tokens)})
        }
    }
}
/// Assert the complete requested getter set at one phase.
fn reads(
    manager: &SettingsManager,
    keys: &[Read],
    expected: &std::collections::BTreeMap<Read, Value>,
) {
    assert_eq!(keys.len(), expected.len());
    for key in keys {
        assert_eq!(read(manager, *key), expected[key], "getter {key:?}");
    }
}
/// Apply the recorded public setters with typed operands.
fn actions(manager: &mut SettingsManager, actions: Vec<(Action, Value)>) {
    for (action, value) in actions {
        match action {
            Action::Theme => manager.set_theme(serde_json::from_value(value).unwrap()),
            Action::DefaultThinkingLevel => {
                assert_eq!(value.as_str(), Some("high"));
                manager.set_default_thinking_level(maestro_settings::ThinkingLevel::High);
            }
            Action::RetryEnabled => {
                manager.set_retry_enabled(serde_json::from_value(value).unwrap());
            }
            Action::QuietStartup => {
                manager.set_quiet_startup(serde_json::from_value(value).unwrap());
            }
            Action::ProjectExtensionPaths => manager.set_project_extension_paths(
                serde_json::from_value::<Vec<String>>(value)
                    .unwrap()
                    .into_iter()
                    .map(SettingsListEntry::String)
                    .collect(),
            ),
            Action::ProjectPackages => {
                let settings = SettingsManager::in_memory(
                    serde_json::from_value(serde_json::json!({"packages":value})).unwrap(),
                );
                manager.set_project_packages(settings.get_packages());
            }
        }
    }
}
/// Input text preserves malformed bytes instead of reparsing them.
fn text(document: Option<&Value>, raw: Option<&String>) -> Option<String> {
    raw.cloned()
        .or_else(|| document.map(|v| serde_json::to_string_pretty(v).unwrap()))
}
/// Read one scope without changing it.
fn stored(storage: &dyn SettingsStorage, scope: SettingsScope) -> Option<String> {
    let mut result = None;
    storage
        .with_lock(scope, &mut |current| {
            result = current.map(str::to_owned);
            Ok(None)
        })
        .unwrap();
    result
}
/// Supply an external edit through the adapter's actual text storage.
fn replace(storage: &dyn SettingsStorage, scope: SettingsScope, text: Option<String>) {
    if let Some(text) = text {
        storage
            .with_lock(scope, &mut |_| Ok(Some(text.clone())))
            .unwrap();
    }
}
/// Compare missing, malformed and usable stored documents distinctly.
fn assert_stored(actual: Option<String>, expected: &Value, malformed: Option<&String>) {
    if expected.is_null() {
        assert!(actual.is_none());
    } else if expected.get("unreadable") == Some(&Value::Bool(true)) {
        assert_eq!(actual.as_ref(), malformed);
        assert!(serde_json::from_str::<Value>(actual.as_ref().unwrap()).is_err());
    } else {
        assert_eq!(
            serde_json::from_str::<Value>(&actual.unwrap()).unwrap(),
            *expected
        );
    }
}
/// Observe both storage phases and the owner's completed writes.
fn settings_case(
    case: Case<SettingsInput, SettingsExpected>,
    storage: &dyn SettingsStorage,
    mut manager: SettingsManager,
) {
    let input = case.input;
    let expected = case.expected;
    reads(&manager, &input.reads, &expected.before);
    replace(
        storage,
        SettingsScope::Global,
        text(
            input.external_global.as_ref(),
            input.external_global_text.as_ref(),
        ),
    );
    replace(
        storage,
        SettingsScope::Project,
        text(input.external_project.as_ref(), None),
    );
    if input.reload {
        runtime::run(manager.reload());
    }
    actions(&mut manager, input.actions);
    runtime::run(manager.flush());
    reads(&manager, &input.reads, &expected.observed);
    if case.test == Group::DefaultsAndOwnedReadsSurviveApplicationRootSelection {
        let mut snapshot = manager.get_global_settings();
        snapshot
            .0
            .insert("theme".into(), Value::String("different".into()));
        let mut packages = manager.get_packages();
        packages.push(PackageSource::Source("different".into()));
        let mut extensions = manager.get_extension_paths();
        extensions.push(SettingsListEntry::String("different".into()));
        reads(&manager, &input.reads, &expected.observed);
        assert!(manager.get_global_settings().0.is_empty());
    }
    assert_stored(
        stored(storage, SettingsScope::Global),
        &expected.global,
        input
            .external_global_text
            .as_ref()
            .or(input.global_text.as_ref()),
    );
    assert_stored(
        stored(storage, SettingsScope::Project),
        &expected.project,
        input.project_text.as_ref(),
    );
    let errors: Vec<_> = manager
        .drain_errors()
        .into_iter()
        .map(|e| match e.scope {
            SettingsScope::Global => Scope::Global,
            SettingsScope::Project => Scope::Project,
        })
        .collect();
    assert_eq!(errors, expected.errors);
    assert!(manager.drain_errors().is_empty());
}
/// Qualify both native and memory adapters in an isolated home.
///
/// # Panics
/// Panics when fixture admission, setup or a recorded observation fails.
pub fn settings(name: &str, group: Group) {
    if let Ok(index) = std::env::var("MAESTRO_DEFAULT_CASE") {
        settings_child(group, index.parse().unwrap());
        println!("\nPROBE settings-ok");
        return;
    }
    let parent = (
        std::env::current_dir().unwrap(),
        std::env::vars_os().collect::<Vec<_>>(),
    );
    for index in 0..cases::<Value, Value>(group).len() {
        let scratch = child::Scratch::new();
        let home = scratch.join("home");
        std::fs::create_dir(&home).unwrap();
        let output = child::probe(
            &crate::child_process::selector(name).unwrap(),
            &scratch.0,
            &[
                ("HOME", &home),
                ("MAESTRO_DEFAULT_ROOT", scratch.0.to_str().unwrap()),
                ("MAESTRO_DEFAULT_CASE", &index.to_string()),
            ],
            REMOVED,
        );
        assert_eq!(output, ["settings-ok"]);
    }
    assert_eq!(
        parent,
        (
            std::env::current_dir().unwrap(),
            std::env::vars_os().collect::<Vec<_>>()
        )
    );
}

/// Run both adapters for one complete recorded row.
fn settings_child(group: Group, index: usize) {
    let root = std::env::var("MAESTRO_DEFAULT_ROOT").unwrap();
    let rows = cases::<Value, Value>(group);
    let row = &rows[index];
    let input: SettingsInput = instantiate(&row.input, &root, "");
    let global = text(input.global.as_ref(), input.global_text.as_ref());
    let project = text(input.project.as_ref(), input.project_text.as_ref());
    let agent = maestro_app::get_agent_dir().unwrap();
    let cwd = std::env::current_dir()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let global_path = maestro_app::configuration::config::paths::get_settings_path(&agent);
    let project_path = Path::new(&cwd).join(".maestro/settings.json");
    if !input.project_missing {
        std::fs::create_dir_all(project_path.parent().unwrap()).unwrap();
    }
    if let Some(text) = &global {
        write(&global_path, text);
    }
    if let Some(text) = &project {
        write(&project_path, text);
    }
    let expected: SettingsExpected = instantiate(&row.expected, &root, "");
    let directory_exists = expected.project_directory_exists;
    let storage = Arc::new(maestro_settings::FileSettingsStorage::new(
        &cwd, &agent, ".maestro",
    ));
    let manager = SettingsManager::create(&cwd, &agent, ".maestro");
    settings_case(
        Case {
            test: group,
            input,
            expected,
        },
        &*storage,
        manager,
    );
    assert_eq!(project_path.parent().unwrap().exists(), directory_exists);
    let memory = Arc::new(InMemorySettingsStorage::new());
    replace(&*memory, SettingsScope::Global, global);
    replace(&*memory, SettingsScope::Project, project);
    let manager = SettingsManager::from_storage(memory.clone());
    settings_case(
        Case {
            test: group,
            input: instantiate(&row.input, &root, ""),
            expected: instantiate(&row.expected, &root, ""),
        },
        &*memory,
        manager,
    );
}
