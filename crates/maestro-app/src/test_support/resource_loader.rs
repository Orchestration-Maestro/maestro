//! A resource loader over supplied extensions, skills and prompt templates, and the loading
//! of inline extensions.
use crate::contribution_files::LoadContributionsResult;
use crate::resources::loading::{
    LoadPromptsResult, LoadThemesResult, ResourceExtensionPaths, ResourceLoader,
};
use maestro_agent::AgentSpawner;
use maestro_extensions::{
    EventError, ExtensionFactory, LoadExtensionsResult, create_event_bus_for,
    create_extension_runtime, load_extension_from_factory,
};
use maestro_models::system_prompt::ContextFile;
use maestro_models::{Execution, Local};
use maestro_resources::LoadSkillsResult;
use std::{cell::RefCell, io, rc::Rc};

/// A loader returning the supplied extensions and no other resources.
pub struct TestResourceLoader {
    /// The loaded extensions.
    pub extensions: Rc<LoadExtensionsResult<Local>>,
    /// The replacement system prompt.
    pub system_prompt: Option<Rc<str>>,
    /// The appended prompt texts.
    pub append_system_prompt: Rc<[String]>,
    /// The loaded skills.
    pub skills: Rc<LoadSkillsResult>,
    /// The loaded prompt templates.
    pub prompts: Rc<LoadPromptsResult>,
    /// Each set of extension resource paths handed to the loader, in order.
    pub extended: Rc<RefCell<Vec<ResourceExtensionPaths>>>,
}

/// A loader returning `extensions` and empty resources.
#[must_use]
pub fn create_test_resource_loader(
    extensions: Option<LoadExtensionsResult<Local>>,
) -> TestResourceLoader {
    let extensions = extensions.unwrap_or_else(|| LoadExtensionsResult {
        extensions: Vec::new(),
        errors: Vec::new(),
        runtime: create_extension_runtime::<Local>(),
    });
    TestResourceLoader {
        extensions: Rc::new(extensions),
        system_prompt: None,
        append_system_prompt: Rc::from(Vec::new()),
        skills: Rc::default(),
        prompts: Rc::default(),
        extended: Rc::default(),
    }
}

impl ResourceLoader<Local> for TestResourceLoader {
    fn get_extensions(&self) -> Rc<LoadExtensionsResult<Local>> {
        Rc::clone(&self.extensions)
    }
    fn get_skills(&self) -> Rc<LoadSkillsResult> {
        Rc::clone(&self.skills)
    }
    fn get_prompts(&self) -> Rc<LoadPromptsResult> {
        Rc::clone(&self.prompts)
    }
    fn get_themes(&self) -> Rc<LoadThemesResult> {
        Rc::default()
    }
    fn get_contributions(&self) -> Rc<LoadContributionsResult> {
        Rc::default()
    }
    fn get_agents_files(&self) -> Rc<[ContextFile]> {
        Rc::from(Vec::new())
    }
    fn get_system_prompt(&self) -> Option<Rc<str>> {
        self.system_prompt.clone()
    }
    fn get_append_system_prompt(&self) -> Rc<[String]> {
        Rc::clone(&self.append_system_prompt)
    }
    fn extend_resources(&self, paths: ResourceExtensionPaths) -> io::Result<()> {
        self.extended.borrow_mut().push(paths);
        Ok(())
    }
    fn reload(&self) -> <Local as Execution>::Future<'_, io::Result<()>> {
        Box::pin(std::future::ready(Ok(())))
    }
}

/// Loads `factories` as one extension set, each at its supplied path or `<inline:N>`.
///
/// # Errors
/// Returns the failure of the first factory that does not load.
pub async fn create_test_extensions_result(
    factories: Vec<(Option<&str>, ExtensionFactory<Local>)>,
    cwd: &str,
) -> Result<LoadExtensionsResult<Local>, EventError> {
    let runtime = create_extension_runtime::<Local>();
    let bus = create_event_bus_for::<Local>(spawn_local(), Rc::new(|_| {}));
    let mut extensions = Vec::new();
    for (index, (path, factory)) in factories.into_iter().enumerate() {
        let path = path.map_or_else(|| format!("<inline:{}>", index + 1), str::to_owned);
        let loaded = load_extension_from_factory(
            factory,
            cwd.to_owned(),
            bus.for_extension(),
            runtime.clone(),
            Some(path),
        );
        extensions.push(loaded.await?);
    }
    Ok(LoadExtensionsResult {
        extensions,
        errors: Vec::new(),
        runtime,
    })
}

/// A spawner running each task on the current task set.
fn spawn_local() -> AgentSpawner<Local> {
    Rc::new(|task| drop(tokio::task::spawn_local(task)))
}
