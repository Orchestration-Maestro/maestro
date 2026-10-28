//! Extensions, skills, prompt templates, themes, browser contributions, context files and
//! system prompt files discovered for one working directory, with provenance for every resource.

mod base_packages;
mod context;
mod discovery;
mod extensions;
mod load_time;
mod overrides;
mod themes;

pub use context::load_project_context_files;
pub use extensions::DefaultEventBus;
pub(crate) use extensions::report;
pub use maestro_models::system_prompt::ContextFile;
pub use maestro_resources::{ResourceCollision, ResourceDiagnostic};

use crate::contributions::LoadContributionsResult;
use discovery::PathIndex;
use maestro_extensions::{
    EventBusController, ExtensionExecution, ExtensionFactory, LoadExtensionsResult, ModuleLoader,
    create_extension_runtime,
};
use maestro_models::{Execution, Local, Threaded};
use maestro_packages::{DefaultPackageManager, NativePackageOperations, PackageManagerOptions};
use maestro_resources::{
    LoadSkillsResult, PathMetadata, PromptTemplate, SourceInfo, create_source_info,
    paths::CONFIG_DIR_NAME,
};
use maestro_settings::SettingsManager;
use maestro_theme::Theme;
use std::{cell::RefCell, collections::BTreeSet, io, rc::Rc};
use tokio::task::LocalSet;

/// One extension-contributed location and its provenance.
pub struct ResourceExtensionPath {
    /// The location, resolved like an additional path.
    pub path: String,
    /// The provenance of everything found there.
    pub metadata: PathMetadata,
}

/// Extension-contributed skill, prompt template and theme locations.
#[derive(Default)]
pub struct ResourceExtensionPaths {
    /// Skill files or directories.
    pub skill_paths: Vec<ResourceExtensionPath>,
    /// Prompt template files or directories.
    pub prompt_paths: Vec<ResourceExtensionPath>,
    /// Theme files or directories.
    pub theme_paths: Vec<ResourceExtensionPath>,
}

/// Loaded prompt templates and their diagnostics.
#[derive(Default)]
pub struct LoadPromptsResult {
    /// Templates in load order, one per name.
    pub prompts: Vec<PromptTemplate>,
    /// Loading problems and name collisions.
    pub diagnostics: Vec<ResourceDiagnostic>,
}

/// Loaded themes and their diagnostics.
#[derive(Default)]
pub struct LoadThemesResult {
    /// Themes in load order, one per name.
    pub themes: Vec<Rc<Theme>>,
    /// Loading problems and name collisions.
    pub diagnostics: Vec<ResourceDiagnostic>,
}

/// A resource kind the loader can skip discovering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceKind {
    /// Configured and automatically discovered extensions.
    Extensions,
    /// Configured and automatically discovered skills.
    Skills,
    /// Configured and automatically discovered prompt templates.
    PromptTemplates,
    /// Configured and automatically discovered themes.
    Themes,
    /// `AGENTS.md` and `CLAUDE.md` context files.
    ContextFiles,
}

/// Replaces a loaded result before it is stored.
pub type ResourceOverride<T> = Box<dyn Fn(T) -> T>;

/// Current resources and their reload.
///
/// Getters return the current snapshot; a reload or an extension of resources replaces
/// snapshots and never changes one already handed out.
pub trait ResourceLoader<E: ExtensionExecution = Threaded> {
    /// The loaded extensions, their load errors and their shared runtime.
    fn get_extensions(&self) -> Rc<LoadExtensionsResult<E>>;
    /// The loaded skills and their diagnostics.
    fn get_skills(&self) -> Rc<LoadSkillsResult>;
    /// The loaded prompt templates and their diagnostics.
    fn get_prompts(&self) -> Rc<LoadPromptsResult>;
    /// The loaded themes and their diagnostics.
    fn get_themes(&self) -> Rc<LoadThemesResult>;
    /// The loaded browser contributions and their diagnostics.
    fn get_contributions(&self) -> Rc<LoadContributionsResult>;
    /// The context files in prompt order.
    fn get_agents_files(&self) -> Rc<[ContextFile]>;
    /// The system prompt replacing the default one, if any.
    fn get_system_prompt(&self) -> Option<Rc<str>>;
    /// The texts appended to the system prompt.
    fn get_append_system_prompt(&self) -> Rc<[String]>;
    /// Contributed model files in order, ranked before the catalog's existing files.
    fn get_model_files(&self) -> Vec<String> {
        Vec::new()
    }
    /// Adds extension-contributed locations and reloads each kind that gained some.
    ///
    /// # Errors
    /// Returns home, working-directory or skill-loading failures.
    fn extend_resources(&self, paths: ResourceExtensionPaths) -> io::Result<()>;
    /// Discovers every resource again.
    ///
    /// # Errors
    /// Returns package resolution, path resolution or extension-loading failures.
    fn reload(&self) -> <Local as Execution>::Future<'_, io::Result<()>>;
}

/// Loader inputs other than its directories and settings.
pub struct ResourceLoaderOptions<E: ExtensionExecution = Threaded> {
    /// The caller's task set: package commands and default event bus continuations run on it.
    pub local: Rc<LocalSet>,
    /// Turns a resolved extension path into its factory.
    pub module_loader: ModuleLoader<E>,
    /// The bus shared by extensions; absent means the execution family's default bus.
    pub event_bus: Option<EventBusController<E>>,
    /// Existing application tool services available during extension construction.
    pub tool_construction: Option<maestro_extensions::ToolConstruction>,
    /// Ordered base package sources whose factories run before other extensions.
    pub base_package_sources: Vec<String>,
    /// Extension sources loaded before configured ones.
    pub additional_extension_paths: Vec<String>,
    /// Skill paths loaded after discovered ones.
    pub additional_skill_paths: Vec<String>,
    /// Prompt template paths loaded after discovered ones.
    pub additional_prompt_template_paths: Vec<String>,
    /// Theme paths loaded after discovered ones.
    pub additional_theme_paths: Vec<String>,
    /// Factories loaded after path extensions, labelled `<inline:N>`.
    pub extension_factories: Vec<ExtensionFactory<E>>,
    /// Kinds whose configured and automatic discovery is skipped.
    pub disabled: BTreeSet<ResourceKind>,
    /// System prompt text or file; an empty text means none, without discovery.
    pub system_prompt: Option<String>,
    /// Appended texts or files; an empty list means none, without discovery.
    pub append_system_prompt: Option<Vec<String>>,
    /// Replaces the loaded extensions.
    pub extensions_override: Option<ResourceOverride<LoadExtensionsResult<E>>>,
    /// Replaces the loaded skills.
    pub skills_override: Option<ResourceOverride<LoadSkillsResult>>,
    /// Replaces the loaded prompt templates.
    pub prompts_override: Option<ResourceOverride<LoadPromptsResult>>,
    /// Replaces the loaded themes.
    pub themes_override: Option<ResourceOverride<LoadThemesResult>>,
    /// Replaces the discovered context files.
    pub agents_files_override: Option<ResourceOverride<Vec<ContextFile>>>,
    /// Replaces the system prompt.
    pub system_prompt_override: Option<ResourceOverride<Option<String>>>,
    /// Replaces the appended texts.
    pub append_system_prompt_override: Option<ResourceOverride<Vec<String>>>,
    /// A caller's offline selection for configured packages, or the ambient flag.
    pub offline: Option<bool>,
}

impl<E: ExtensionExecution> ResourceLoaderOptions<E> {
    /// Inputs with every optional field absent or empty.
    #[must_use]
    pub fn new(local: Rc<LocalSet>, module_loader: ModuleLoader<E>) -> Self {
        Self {
            local,
            module_loader,
            event_bus: None,
            tool_construction: None,
            base_package_sources: Vec::new(),
            additional_extension_paths: Vec::new(),
            additional_skill_paths: Vec::new(),
            additional_prompt_template_paths: Vec::new(),
            additional_theme_paths: Vec::new(),
            extension_factories: Vec::new(),
            disabled: BTreeSet::new(),
            system_prompt: None,
            append_system_prompt: None,
            extensions_override: None,
            skills_override: None,
            prompts_override: None,
            themes_override: None,
            agents_files_override: None,
            system_prompt_override: None,
            append_system_prompt_override: None,
            offline: None,
        }
    }
}

/// Loader construction inputs.
pub struct DefaultResourceLoaderOptions<E: ExtensionExecution = Threaded> {
    /// The working directory resources are discovered for.
    pub cwd: String,
    /// The user configuration directory.
    pub agent_dir: String,
    /// The shared settings; absent means settings created for `cwd` and `agent_dir`.
    pub settings_manager: Option<Rc<RefCell<SettingsManager>>>,
    /// The remaining inputs.
    pub resources: ResourceLoaderOptions<E>,
}

/// The skill, prompt template or theme collection an extension can contribute to.
#[derive(Clone, Copy)]
enum Collection {
    /// Skills.
    Skills,
    /// Prompt templates.
    Prompts,
    /// Themes.
    Themes,
}

/// One collection's last load paths and its extension-contributed provenance.
#[derive(Default)]
struct Contributed {
    /// The paths the collection was last loaded from.
    paths: Vec<String>,
    /// Provenance recorded by extensions, by resolved path.
    sources: PathIndex<SourceInfo>,
}

/// Results replaced as whole snapshots, and the inputs of later rebuilds.
struct LoaderState<E: ExtensionExecution> {
    /// Loaded extensions.
    extensions: Rc<LoadExtensionsResult<E>>,
    /// Loaded skills.
    skills: Rc<LoadSkillsResult>,
    /// Loaded prompt templates.
    prompts: Rc<LoadPromptsResult>,
    /// Loaded themes.
    themes: Rc<LoadThemesResult>,
    /// Loaded browser contributions.
    contributions: Rc<LoadContributionsResult>,
    /// Discovered context files.
    agents_files: Rc<[ContextFile]>,
    /// The replacing system prompt.
    system_prompt: Option<Rc<str>>,
    /// The appended texts.
    append_system_prompt: Rc<[String]>,
    /// Base model files in contribution order.
    model_files: Vec<String>,
    /// The system prompt candidate below ordinary prompt inputs.
    base_system_prompt: Option<String>,
    /// Text that always precedes the ordinary append choice.
    base_append_text: Vec<String>,
    /// Skill paths and contributions.
    skill_sources: Contributed,
    /// Prompt template paths and contributions.
    prompt_sources: Contributed,
    /// Theme paths and contributions.
    theme_sources: Contributed,
    /// Provenance of every resolved resource, rebuilt by each reload; first entry per path.
    metadata: PathIndex<PathMetadata>,
}

impl<E: ExtensionExecution> LoaderState<E> {
    /// The paths and contributions of `collection`.
    fn contributed(&self, collection: Collection) -> &Contributed {
        match collection {
            Collection::Skills => &self.skill_sources,
            Collection::Prompts => &self.prompt_sources,
            Collection::Themes => &self.theme_sources,
        }
    }
    /// The paths and contributions of `collection`, for replacement.
    fn contributed_mut(&mut self, collection: Collection) -> &mut Contributed {
        match collection {
            Collection::Skills => &mut self.skill_sources,
            Collection::Prompts => &mut self.prompt_sources,
            Collection::Themes => &mut self.theme_sources,
        }
    }
}

/// The file-system resource loader.
pub struct DefaultResourceLoader<E: ExtensionExecution = Threaded> {
    /// The working directory resources are discovered for.
    cwd: String,
    /// The user configuration directory.
    agent_dir: String,
    /// Settings refreshed by each reload.
    settings_manager: Rc<RefCell<SettingsManager>>,
    /// The bus shared by loaded extensions.
    event_bus: EventBusController<E>,
    /// Resolves configured and explicit sources.
    package_manager: DefaultPackageManager<NativePackageOperations>,
    /// The remaining inputs.
    options: ResourceLoaderOptions<E>,
    /// Current results.
    state: RefCell<LoaderState<E>>,
}

impl<E: DefaultEventBus> DefaultResourceLoader<E> {
    /// Stores the inputs; every result is empty until the first reload.
    ///
    /// An absent settings manager is created for `cwd` and `agent_dir`; an absent event
    /// bus is the execution family's default bus on the supplied task set.
    #[must_use]
    pub fn new(options: DefaultResourceLoaderOptions<E>) -> Self {
        let DefaultResourceLoaderOptions {
            cwd,
            agent_dir,
            settings_manager,
            mut resources,
        } = options;
        let settings_manager = settings_manager.unwrap_or_else(|| {
            Rc::new(RefCell::new(SettingsManager::create(
                &cwd,
                &agent_dir,
                CONFIG_DIR_NAME,
            )))
        });
        let event_bus = resources
            .event_bus
            .take()
            .unwrap_or_else(|| E::default_event_bus(&resources.local));
        let package_manager = DefaultPackageManager::new(
            PackageManagerOptions {
                cwd: cwd.clone(),
                agent_dir: agent_dir.clone(),
                settings_manager: Rc::clone(&settings_manager),
            },
            NativePackageOperations::new(
                Rc::new(crate::output_guard::is_stdout_taken_over),
                &resources.local,
            ),
        )
        .with_offline(resources.offline);
        let state = RefCell::new(LoaderState {
            extensions: Rc::new(LoadExtensionsResult {
                extensions: Vec::new(),
                errors: Vec::new(),
                runtime: create_extension_runtime::<E>(),
            }),
            skills: Rc::default(),
            prompts: Rc::default(),
            themes: Rc::default(),
            contributions: Rc::default(),
            agents_files: Rc::from([]),
            system_prompt: None,
            append_system_prompt: Rc::from([]),
            model_files: Vec::new(),
            base_system_prompt: None,
            base_append_text: Vec::new(),
            skill_sources: Contributed::default(),
            prompt_sources: Contributed::default(),
            theme_sources: Contributed::default(),
            metadata: PathIndex::default(),
        });
        Self {
            cwd,
            agent_dir,
            settings_manager,
            event_bus,
            package_manager,
            options: resources,
            state,
        }
    }

    /// Whether configured and automatic discovery of `kind` is skipped.
    fn is_disabled(&self, kind: ResourceKind) -> bool {
        self.options.disabled.contains(&kind)
    }

    /// Records the resolved `entries` as contributions to `collection`.
    fn record_contributions(&self, collection: Collection, entries: &[ResourceExtensionPath]) {
        let mut state = self.state.borrow_mut();
        let contributed = &mut state.contributed_mut(collection).sources;
        for entry in entries {
            let source = create_source_info(entry.path.clone(), entry.metadata.clone());
            contributed.insert(entry.path.clone(), source);
        }
    }

    /// Adds the resolved `entries` to the paths of `collection` and rebuilds it.
    fn extend_collection(
        &self,
        collection: Collection,
        entries: &[ResourceExtensionPath],
    ) -> io::Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        {
            let mut state = self.state.borrow_mut();
            let contributed = state.contributed_mut(collection);
            let added = entries.iter().map(|entry| entry.path.as_str());
            contributed.paths = self.merge_paths(&contributed.paths, added)?;
        }
        self.update(collection, &[])
    }
}

impl<E: DefaultEventBus> ResourceLoader<E> for DefaultResourceLoader<E> {
    fn get_extensions(&self) -> Rc<LoadExtensionsResult<E>> {
        Rc::clone(&self.state.borrow().extensions)
    }
    fn get_skills(&self) -> Rc<LoadSkillsResult> {
        Rc::clone(&self.state.borrow().skills)
    }
    fn get_prompts(&self) -> Rc<LoadPromptsResult> {
        Rc::clone(&self.state.borrow().prompts)
    }
    fn get_themes(&self) -> Rc<LoadThemesResult> {
        Rc::clone(&self.state.borrow().themes)
    }
    fn get_contributions(&self) -> Rc<LoadContributionsResult> {
        Rc::clone(&self.state.borrow().contributions)
    }
    fn get_agents_files(&self) -> Rc<[ContextFile]> {
        Rc::clone(&self.state.borrow().agents_files)
    }
    fn get_system_prompt(&self) -> Option<Rc<str>> {
        self.state.borrow().system_prompt.clone()
    }
    fn get_append_system_prompt(&self) -> Rc<[String]> {
        Rc::clone(&self.state.borrow().append_system_prompt)
    }
    fn get_model_files(&self) -> Vec<String> {
        self.state.borrow().model_files.clone()
    }
    fn extend_resources(&self, paths: ResourceExtensionPaths) -> io::Result<()> {
        let contributions = [
            (Collection::Skills, self.resolve_entries(paths.skill_paths)?),
            (
                Collection::Prompts,
                self.resolve_entries(paths.prompt_paths)?,
            ),
            (Collection::Themes, self.resolve_entries(paths.theme_paths)?),
        ];
        for (collection, entries) in &contributions {
            self.record_contributions(*collection, entries);
        }
        for (collection, entries) in &contributions {
            self.extend_collection(*collection, entries)?;
        }
        Ok(())
    }
    fn reload(&self) -> <Local as Execution>::Future<'_, io::Result<()>> {
        Box::pin(self.reload_resources())
    }
}
