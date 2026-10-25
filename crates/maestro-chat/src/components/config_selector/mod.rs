//! Configuration of resolved resources through a retained, focusable list.
mod groups;
mod render;
mod resource_list;
mod settings;
use maestro_app::{PathMetadata, ResolvedPaths, SettingsManager};
use maestro_theme::{Collator, LiveTheme};
use maestro_tui::{
    Component, Container, Spacer,
    tui::{FocusFlag, Focusable},
};
pub use resource_list::ResourceList;
use std::{cell::RefCell, rc::Rc};

/// Extensible resource kind, with ordered defaults for the resolved collections.
pub type ResourceType = String;
/// Default kinds in display order and their heading labels.
const RESOURCE_TYPE_LABELS: [(&str, &str); 4] = [
    ("extensions", "Extensions"),
    ("skills", "Skills"),
    ("prompts", "Prompts"),
    ("themes", "Themes"),
];
/// One live resource shared by painting, search and notifications.
pub struct ResourceItem {
    /// Authored resource path.
    pub path: String,
    /// Current enabled state.
    pub enabled: bool,
    /// Current provenance used for scoped toggles.
    pub metadata: PathMetadata,
    /// Resource kind used for filtering and settings selection.
    pub resource_type: ResourceType,
    /// Current display label.
    pub display_name: String,
    /// Original group key, editable independently of its captured heading.
    pub group_key: String,
    /// Original subgroup key, editable independently of its captured heading.
    pub subgroup_key: String,
}
/// Notification receiving the same live item used by the list.
pub type ResourceToggle = Rc<dyn Fn(Rc<RefCell<ResourceItem>>, bool)>;
/// Supplied project location and existing ambient presentation/path owners.
pub struct ConfigSelectorOptions {
    /// Project directory used for project-relative settings patterns.
    pub cwd: String,
    /// User resource directory.
    pub agent_dir: String,
    /// Current published colors.
    pub theme: LiveTheme,
    /// Prepared locale comparator.
    pub collator: Rc<Collator>,
    /// Live policy sampled for bold text.
    pub styles_enabled: Rc<dyn Fn() -> bool>,
}
pub use maestro_app::presentation_data::input_selectors::ConfigSelectorCallbacks;
/// Configuration chrome with an independently retained focus/list handle.
pub struct ConfigSelectorComponent {
    /// Sole child-array owner.
    container: Container,
    /// List retained even while removed from the child array.
    resource_list: Rc<ResourceList>,
    /// Focus forwards directly to the retained list.
    focus: FocusFlag,
}
impl ConfigSelectorComponent {
    /// Builds a configuration panel over supplied resolved resources and settings.
    #[must_use]
    pub fn new(
        resolved: ResolvedPaths,
        settings: Rc<RefCell<SettingsManager>>,
        options: ConfigSelectorOptions,
        callbacks: ConfigSelectorCallbacks,
    ) -> Self {
        let groups = groups::build_groups(resolved, &options.collator);
        let options = Rc::new(options);
        let resource_list = Rc::new(ResourceList::new(groups, settings, options.clone()));
        resource_list.set_on_cancel(Some(callbacks.on_close));
        resource_list.set_on_exit(Some(callbacks.on_exit));
        resource_list.set_on_toggle(Some(Rc::new(move |_, _| (callbacks.request_render)())));
        let container = Container::new();
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(Rc::new(super::dynamic_border::DynamicBorder::new(
            options.theme.clone(),
            None,
        )));
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(Rc::new(render::ConfigSelectorHeader(options.clone())));
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(resource_list.clone());
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(Rc::new(super::dynamic_border::DynamicBorder::new(
            options.theme.clone(),
            None,
        )));
        let focus = FocusFlag::forwarding(resource_list.clone());
        Self {
            container,
            resource_list,
            focus,
        }
    }
    /// Returns the retained input-dispatch target, independent of child-array edits.
    #[must_use]
    pub fn get_resource_list(&self) -> &Rc<ResourceList> {
        &self.resource_list
    }
}
component_children!(ConfigSelectorComponent, Container, container);
impl Focusable for ConfigSelectorComponent {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}
impl Component for ConfigSelectorComponent {
    /// Legacy rendering emits no partial frame when a child fails.
    fn render(&self, width: usize) -> Vec<String> {
        self.try_render(width).unwrap_or_default()
    }
    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        self.container.try_render(width)
    }
    fn invalidate(&self) {
        self.container.invalidate();
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}
