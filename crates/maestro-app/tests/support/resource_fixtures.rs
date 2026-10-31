//! Recorded resource corpus: directory trees, a controlled module loader, loader
//! construction, task-set execution and the projections the recordings compare.
#![cfg(unix)]
use maestro_app::{
    DefaultResourceLoader, DefaultResourceLoaderOptions, ExtensionAPI, ExtensionFactory,
    ExtensionRunner, ModuleLoader, ResourceDiagnostic, ResourceExtensionPath,
    ResourceExtensionPaths, ResourceKind, ResourceLoader, ResourceLoaderOptions,
};
use maestro_catalog::ModelRegistry;
use maestro_credentials::{AuthStorage, AuthStorageData};
use maestro_extensions::{
    EventError,
    runner::ExtensionRunnerSession,
    types::{CommandHandler, ExtensionFlagOptions, FlagType, RegisteredCommandOptions},
};
use maestro_models::{Local, Tool};
use maestro_resources::{DiagnosticType, PathMetadata, SourceOrigin, SourceScope};
use maestro_session::SessionManager;
use maestro_settings::{Settings, SettingsManager};
use maestro_tools::ToolDefinitionRecord;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};
use tokio::task::LocalSet;

/// The recorded loader corpus.
pub const LOADER: &str = include_str!("../fixtures/resources/resource_loader.json");

/// The configuration directory name the recordings spell `$CONFIG`.
const CONFIG: &str = ".maestro";

/// One tree entry: a text file, a directory, a link, an extension, a JSON file or a theme.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Root-relative or placeholder-absolute location.
    path: String,
    /// File text with placeholders.
    text: Option<String>,
    /// Whether the entry is a directory.
    #[serde(default)]
    dir: bool,
    /// Link target with placeholders.
    link: Option<String>,
    /// Registrations of an extension module.
    extension: Option<ExtensionSpec>,
    /// JSON document with placeholders in its strings.
    json: Option<Value>,
    /// A theme built over the shipped dark theme.
    theme: Option<ThemeDelta>,
}

/// What a controlled extension module registers, or how it fails.
#[derive(Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionSpec {
    /// Command names and descriptions.
    #[serde(default)]
    commands: Vec<(String, String)>,
    /// Tool names and descriptions.
    #[serde(default)]
    tools: Vec<(String, String)>,
    /// Flag names and declared kinds.
    #[serde(default)]
    flags: Vec<(String, String)>,
    /// Initialization failure message.
    fail: Option<String>,
}

/// A theme's name and the values replacing the shipped dark theme's.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeDelta {
    /// Theme name.
    name: String,
    /// Variables added over the dark theme's.
    #[serde(default)]
    vars: Map<String, Value>,
    /// Export colors.
    export: Option<Value>,
}

/// Recorded loader options.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Inputs {
    #[serde(default)]
    additional_extension_paths: Vec<String>,
    #[serde(default)]
    additional_skill_paths: Vec<String>,
    #[serde(default)]
    additional_prompt_template_paths: Vec<String>,
    #[serde(default)]
    additional_theme_paths: Vec<String>,
    no_extensions: Option<bool>,
    no_skills: Option<bool>,
    no_prompt_templates: Option<bool>,
    no_themes: Option<bool>,
    no_context_files: Option<bool>,
    system_prompt: Option<String>,
    append_system_prompt: Option<Vec<String>>,
}

/// Extension-contributed paths with their provenance.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Extend {
    #[serde(default, rename = "skillPaths")]
    skills: Vec<(String, Meta)>,
    #[serde(default, rename = "promptPaths")]
    prompts: Vec<(String, Meta)>,
    #[serde(default, rename = "themePaths")]
    themes: Vec<(String, Meta)>,
}

/// Recorded path provenance.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Meta {
    source: String,
    scope: SourceScope,
    origin: SourceOrigin,
    base_dir: Option<String>,
}

/// One recorded loader case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LoaderCase {
    pub name: String,
    #[serde(default)]
    pub tree: Vec<Entry>,
    #[serde(default)]
    pub options: Inputs,
    pub in_memory_settings: Option<Map<String, Value>>,
    pub reload: bool,
    pub extend: Option<Extend>,
    pub inline: Option<Vec<ExtensionSpec>>,
    pub process_cwd: Option<String>,
    pub expected: Value,
}

/// The loader corpus sections.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoaderCorpus {
    pub cases: Vec<LoaderCase>,
    pub overrides: Vec<Value>,
    pub precedence: Vec<LoaderCase>,
    pub settings: Value,
}

/// The named loader case from the main or precedence section.
pub fn loader_case(name: &str) -> LoaderCase {
    let corpus: LoaderCorpus = serde_json::from_str(LOADER).unwrap();
    let mut cases = corpus.cases.into_iter().chain(corpus.precedence);
    cases
        .find(|case| case.name == name)
        .unwrap_or_else(|| panic!("no recorded case {name}"))
}

/// One case's directories: the root with `agent` and `project`, and the shared home.
pub struct Root {
    pub path: String,
    pub home: String,
}

impl Root {
    /// Creates `<base>/<name>` with its `agent` and `project` directories.
    pub fn new(base: &str, name: &str) -> Self {
        let path = format!("{base}/{name}");
        for dir in ["agent", "project"] {
            std::fs::create_dir_all(format!("{path}/{dir}")).unwrap();
        }
        Self {
            path,
            home: format!("{base}/home"),
        }
    }
    /// Replaces the placeholders in `text`.
    pub fn fill(&self, text: &str) -> String {
        text.replace("$ROOT", &self.path)
            .replace("$HOME", &self.home)
            .replace("$CONFIG", CONFIG)
    }
    /// The agent directory.
    pub fn agent(&self) -> String {
        format!("{}/agent", self.path)
    }
    /// The project directory, the loader's working directory.
    pub fn project(&self) -> String {
        format!("{}/project", self.path)
    }
    /// `value` with the root, home and configuration directory written as placeholders.
    pub fn normalize(&self, value: &Value) -> Value {
        let text = value
            .to_string()
            .replace(&self.path, "$ROOT")
            .replace(&self.home, "$HOME")
            .replace(&format!("/{CONFIG}/"), "/$CONFIG/")
            .replace(&format!("/{CONFIG}\""), "/$CONFIG\"");
        serde_json::from_str(&text).unwrap()
    }
    /// `value` with placeholders filled in every string.
    fn fill_json(&self, value: &Value) -> Value {
        match value {
            Value::String(text) => Value::String(self.fill(text)),
            Value::Array(items) => items.iter().map(|item| self.fill_json(item)).collect(),
            Value::Object(map) => (map.iter())
                .map(|(key, item)| (key.clone(), self.fill_json(item)))
                .collect(),
            other => other.clone(),
        }
    }
    /// Creates every entry of `tree`.
    pub fn write_tree(&self, tree: &[Entry]) {
        for entry in tree {
            let target = self.fill(&entry.path);
            let path = if target.starts_with('/') {
                target
            } else {
                format!("{}/{target}", self.path)
            };
            let parent = std::path::Path::new(&path).parent().unwrap();
            std::fs::create_dir_all(parent).unwrap();
            if entry.dir {
                std::fs::create_dir_all(&path).unwrap();
            } else if let Some(link) = &entry.link {
                std::os::unix::fs::symlink(self.fill(link), &path).unwrap();
            } else {
                std::fs::write(&path, self.content(entry)).unwrap();
            }
        }
    }
    /// The file text of a non-directory, non-link entry.
    fn content(&self, entry: &Entry) -> String {
        if let Some(spec) = &entry.extension {
            return serde_json::to_string(&json!({
                "commands": spec.commands, "tools": spec.tools,
                "flags": spec.flags, "fail": spec.fail,
            }))
            .unwrap();
        }
        if let Some(theme) = &entry.theme {
            return theme_text(theme);
        }
        if let Some(document) = &entry.json {
            return serde_json::to_string_pretty(&self.fill_json(document)).unwrap();
        }
        self.fill(entry.text.as_deref().unwrap_or_default())
    }
}

/// The shipped dark theme with the delta's name, variables and export colors.
fn theme_text(theme: &ThemeDelta) -> String {
    let dark = include_str!("../../../maestro-theme/assets/theme/dark.json");
    let mut document: Map<String, Value> = serde_json::from_str(dark).unwrap();
    document.insert("name".into(), json!(theme.name));
    let vars = document.entry("vars").or_insert_with(|| json!({}));
    vars.as_object_mut().unwrap().extend(theme.vars.clone());
    if let Some(export) = &theme.export {
        document.insert("export".into(), export.clone());
    }
    serde_json::to_string_pretty(&document).unwrap()
}

/// Registers the spec's commands, tools and flags, or fails with its message.
fn register(spec: &ExtensionSpec, api: &ExtensionAPI<Local>) -> Result<(), EventError> {
    if let Some(message) = &spec.fail {
        return Err(message.clone().into());
    }
    for (name, description) in &spec.commands {
        let handler: CommandHandler<Local> = Rc::new(|_, _| Box::pin(async { Ok(()) }));
        let options = RegisteredCommandOptions {
            description: Some(description.clone()),
            get_argument_completions: None,
            handler,
        };
        api.register_command(name.clone(), options)?;
    }
    for (name, description) in &spec.tools {
        api.register_tool(tool(name, description))?;
    }
    for (name, kind) in &spec.flags {
        let r#type = if kind == "boolean" {
            FlagType::Boolean
        } else {
            FlagType::String
        };
        let options = ExtensionFlagOptions {
            description: None,
            r#type,
            default: None,
        };
        api.register_flag(name.clone(), options)?;
    }
    Ok(())
}

/// A tool with an empty object schema that returns nothing.
fn tool(
    name: &str,
    description: &str,
) -> ToolDefinitionRecord<maestro_app::ExtensionContext, Value, Value, (), Local, Local> {
    ToolDefinitionRecord {
        definition: Tool {
            name: name.into(),
            description: description.into(),
            parameters: json!({"type": "object", "properties": {}}).into(),
        },
        label: name.into(),
        prompt_snippet: None,
        prompt_guidelines: None,
        render_shell: None,
        prepare_arguments: None,
        execution_mode: None,
        execute: Rc::new(|_, _, _, _, _| {
            Box::pin(async {
                Ok(maestro_agent::AgentToolResult {
                    content: vec![],
                    details: None,
                    terminate: None,
                    usage: None,
                })
            })
        }),
        render_call: None,
        render_result: None,
    }
}

/// A factory registering what `spec` lists.
pub fn factory(spec: ExtensionSpec) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let registered = register(&spec, &api);
        Box::pin(async move { registered })
    })
}

/// Reads a module's registrations from the JSON spec written at its resolved path.
pub fn module_loader() -> ModuleLoader<Local> {
    Rc::new(|resolved: &str| {
        let module = std::fs::read_to_string(resolved)
            .map_err(EventError::from)
            .and_then(|text| Ok(serde_json::from_str::<ExtensionSpec>(&text)?))
            .map(|spec| Some(factory(spec)));
        Box::pin(std::future::ready(module))
    })
}

pub use crate::support::session_harness::{no_commands, on_task_set};

/// The loader inputs a recorded case describes.
pub fn resource_options(
    root: &Root,
    case: &LoaderCase,
    local: Rc<LocalSet>,
) -> ResourceLoaderOptions<Local> {
    let inputs = &case.options;
    let fill = |paths: &[String]| paths.iter().map(|path| root.fill(path)).collect();
    let mut options = ResourceLoaderOptions::new(local, module_loader());
    options.additional_extension_paths = fill(&inputs.additional_extension_paths);
    options.additional_skill_paths = fill(&inputs.additional_skill_paths);
    options.additional_prompt_template_paths = fill(&inputs.additional_prompt_template_paths);
    options.additional_theme_paths = fill(&inputs.additional_theme_paths);
    options.system_prompt = inputs.system_prompt.as_deref().map(|text| root.fill(text));
    options.append_system_prompt = inputs.append_system_prompt.as_deref().map(fill);
    for (disabled, kind) in [
        (inputs.no_extensions, ResourceKind::Extensions),
        (inputs.no_skills, ResourceKind::Skills),
        (inputs.no_prompt_templates, ResourceKind::PromptTemplates),
        (inputs.no_themes, ResourceKind::Themes),
        (inputs.no_context_files, ResourceKind::ContextFiles),
    ] {
        if disabled == Some(true) {
            options.disabled.insert(kind);
        }
    }
    options.extension_factories = (case.inline.iter().flatten().cloned())
        .map(factory)
        .collect();
    options
}

/// A loader for the case in `root`, with in-memory settings when the case has some.
pub fn loader(
    root: &Root,
    case: &LoaderCase,
    resources: ResourceLoaderOptions<Local>,
) -> DefaultResourceLoader<Local> {
    let settings_manager = case
        .in_memory_settings
        .clone()
        .map(|settings| Rc::new(RefCell::new(SettingsManager::in_memory(Settings(settings)))));
    DefaultResourceLoader::new(DefaultResourceLoaderOptions {
        cwd: root.project(),
        agent_dir: root.agent(),
        settings_manager,
        resources,
    })
}

/// The case's extension contributions with placeholders filled.
pub fn contributions(root: &Root, extend: &Extend) -> ResourceExtensionPaths {
    let paths = |entries: &[(String, Meta)]| {
        (entries.iter())
            .map(|(path, meta)| ResourceExtensionPath {
                path: root.fill(path),
                metadata: PathMetadata {
                    source: meta.source.clone(),
                    scope: meta.scope,
                    origin: meta.origin,
                    base_dir: meta.base_dir.as_deref().map(|dir| root.fill(dir)),
                },
            })
            .collect()
    };
    ResourceExtensionPaths {
        skill_paths: paths(&extend.skills),
        prompt_paths: paths(&extend.prompts),
        theme_paths: paths(&extend.themes),
    }
}

/// Runs the recorded case in `root` and returns its normalized observation.
///
/// A recorded process directory is entered first; only isolated children run cases.
pub async fn run_loader_case(root: &Root, case: &LoaderCase, local: Rc<LocalSet>) -> Value {
    root.write_tree(&case.tree);
    if let Some(dir) = &case.process_cwd {
        std::env::set_current_dir(root.fill(dir)).unwrap();
    }
    let loader = loader(root, case, resource_options(root, case, local));
    if case.reload {
        loader.reload().await.unwrap();
    }
    if let Some(extend) = &case.extend {
        loader
            .extend_resources(contributions(root, extend))
            .unwrap();
    }
    let mut observed = observe(&loader);
    if let Some(runner) = case.expected.get("runner") {
        observed["runner"] = runner_view(&loader, runner);
    }
    root.normalize(&observed)
}

pub use super::isolated_home::{with_environment, with_isolated_home};

/// The standard error lines of `stderr` that are warnings.
pub fn warnings(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|line| line.starts_with("Warning:"))
        .collect()
}

/// The native error of reading a directory, which a read failure reports.
pub fn directory_read_error() -> std::io::Error {
    std::fs::read(env!("CARGO_MANIFEST_DIR")).unwrap_err()
}

/// Names the scratch base a standard-error child works in.
const CHILD_BASE: &str = "MAESTRO_RESOURCE_TEST_BASE";

/// Runs `body` with a scratch base in a child whose home is `<base>/home`; in the parent,
/// returns the base and the child's standard error once the child passed.
pub fn child_stderr(test: &str, body: impl FnOnce(&str)) -> Option<(String, String)> {
    if maestro_test_process::selection().is_child() {
        body(&std::env::var(CHILD_BASE).unwrap());
        return None;
    }
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let base = scratch.path().display().to_string();
    let output = maestro_test_process::TestProcess::current(test)
        .unwrap()
        .case("*")
        .nocapture()
        .env("HOME", format!("{base}/home"))
        .env(CHILD_BASE, &base)
        .output()
        .unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "{stderr}");
    Some((base, stderr))
}

/// Runs the named recorded cases, each in its own root, in one isolated-home child.
pub fn check_loader_cases(test: &str, names: &[&str]) {
    with_isolated_home(test, |base| {
        for name in names {
            let case = loader_case(name);
            let root = Root::new(base, name);
            let observed =
                on_task_set(|local| async move { run_loader_case(&root, &case, local).await });
            assert_eq!(observed, loader_case(name).expected, "case {name}");
        }
    });
}

/// A recorded diagnostic projection: type, message, path and collision paths.
pub fn diagnostic(diagnostic: &ResourceDiagnostic) -> Value {
    let r#type = match diagnostic.r#type {
        DiagnosticType::Warning => "warning",
        DiagnosticType::Error => "error",
        DiagnosticType::Collision => "collision",
    };
    let mut projected = json!({"type": r#type, "message": diagnostic.message});
    if let Some(path) = &diagnostic.path {
        projected["path"] = json!(path);
    }
    if let Some(collision) = &diagnostic.collision {
        projected["collision"] = json!({
            "resourceType": collision.resource_type, "name": collision.name,
            "winnerPath": collision.winner_path, "loserPath": collision.loser_path,
        });
    }
    projected
}

/// The recorded projection of every loader result.
pub fn observe(loader: &impl ResourceLoader<Local>) -> Value {
    let extensions = loader.get_extensions();
    let skills = loader.get_skills();
    let prompts = loader.get_prompts();
    let themes = loader.get_themes();
    json!({
        "extensions": extensions.extensions.iter().map(|handle| {
            let extension = handle.read().unwrap();
            let shared = (extension.commands.values().map(|c| Arc::clone(&c.read().unwrap().source_info)))
                .chain(extension.tools.values().map(|t| Arc::clone(&t.source_info)))
                .all(|info| Arc::ptr_eq(&info, &extension.source_info));
            json!({
                "path": extension.path,
                "sourceInfo": *extension.source_info.read().unwrap(),
                "commands": extension.commands.keys().collect::<Vec<_>>(),
                "tools": extension.tools.keys().collect::<Vec<_>>(),
                "flags": extension.flags.keys().collect::<Vec<_>>(),
                "sharedSourceInfo": shared,
            })
        }).collect::<Vec<_>>(),
        "extensionErrors": extensions.errors.iter().map(|e| json!({"path": e.path, "error": e.error})).collect::<Vec<_>>(),
        "skills": skills.skills.iter().map(|s| json!({
            "name": s.name, "description": s.description, "filePath": s.file_path,
            "baseDir": s.base_dir, "sourceInfo": s.source_info,
        })).collect::<Vec<_>>(),
        "skillDiagnostics": skills.diagnostics.iter().map(diagnostic).collect::<Vec<_>>(),
        "prompts": prompts.prompts.iter().map(|p| json!({
            "name": p.name, "description": p.description, "filePath": p.file_path,
            "sourceInfo": p.source_info,
        })).collect::<Vec<_>>(),
        "promptDiagnostics": prompts.diagnostics.iter().map(diagnostic).collect::<Vec<_>>(),
        "themes": themes.themes.iter().map(|t| json!({
            "name": t.name(), "sourcePath": t.source_path(),
            "sourceInfo": t.source_info().map(|info| info.borrow().clone()),
        })).collect::<Vec<_>>(),
        "themeDiagnostics": themes.diagnostics.iter().map(diagnostic).collect::<Vec<_>>(),
        "agentsFiles": *loader.get_agents_files(),
        "systemPrompt": loader.get_system_prompt().as_deref(),
        "appendSystemPrompt": *loader.get_append_system_prompt(),
    })
}

/// Invocation names and the queried commands' and tools' descriptions, from a runner
/// over the loaded extensions.
fn runner_view(loader: &impl ResourceLoader<Local>, recorded: &Value) -> Value {
    let result = loader.get_extensions();
    let auth = Arc::new(AuthStorage::in_memory(AuthStorageData::default()));
    let session = ExtensionRunnerSession {
        cwd: "/work".into(),
        session_manager: Rc::new(RefCell::new(SessionManager::in_memory(None).unwrap())),
        model_registry: maestro_models::SharedHandle::new(
            ModelRegistry::in_memory(auth, no_commands()).unwrap(),
        ),
    };
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(result.extensions.clone())),
        result.runtime.clone(),
        session,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    let names =
        |key: &str| (recorded[key].as_object().unwrap().keys().cloned()).collect::<Vec<_>>();
    let commands: Map<String, Value> = (names("commands").into_iter())
        .map(|name| {
            let description = runner
                .get_command(&name)
                .and_then(|c| c.description.clone());
            (name, json!(description))
        })
        .collect();
    let tools: Map<String, Value> = (names("tools").into_iter())
        .map(|name| {
            let definition = runner.get_tool_definition::<Value, Value, ()>(&name);
            let description = definition.map(|d| d.read().unwrap().definition.description.clone());
            (name, json!(description))
        })
        .collect();
    let invocation_names: Vec<String> = (runner.get_registered_commands().into_iter())
        .map(|command| command.invocation_name)
        .collect();
    json!({"invocationNames": invocation_names, "commands": commands, "tools": tools})
}
