//! Scoped files, live publication and lossless public-output observations.
use super::records::{Frame, Scopes};
use maestro_app::{ResolvedPaths, SettingsManager};
use maestro_chat::components::config_selector::{
    ConfigSelectorCallbacks, ConfigSelectorComponent, ConfigSelectorOptions, ResourceItem,
};
use maestro_theme::{Collator, ColorMode, ColorValue, Theme, ThemeColor, ThemeOptions, ThemeState};
use maestro_tui::{Component, tui::ChildArray};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
};

/// Palette forced by the independent recorder.
pub(super) fn colors(shift: u8) -> Rc<Theme> {
    Rc::new(
        Theme::new(
            [
                (ThemeColor::Border, ColorValue::Index(31 + shift)),
                (ThemeColor::Accent, ColorValue::Index(32 + shift)),
                (ThemeColor::Muted, ColorValue::Index(33 + shift)),
                (ThemeColor::Dim, ColorValue::Index(34 + shift)),
                (ThemeColor::Success, ColorValue::Index(35 + shift)),
            ],
            [],
            ColorMode::Truecolor,
            ThemeOptions::default(),
        )
        .unwrap(),
    )
}
/// Creates a file exclusively, never overwriting another test's state.
fn seed(path: &std::path::Path, value: &Value) {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(serde_json::to_string_pretty(value).unwrap().as_bytes())
        .unwrap();
}
/// Raw member order independent of JSON object value equality.
fn keys(value: &Value) -> Vec<String> {
    value.as_object().unwrap().keys().cloned().collect()
}
/// A package array may include arbitrary nonobjects.
fn package_keys(value: &Value) -> Vec<Vec<String>> {
    value
        .get("packages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| {
            item.as_object()
                .map_or_else(Vec::new, |map| map.keys().cloned().collect())
        })
        .collect()
}
/// One complete resource transaction, with no process-global cwd changes.
pub(super) struct Harness {
    /// Public component under test.
    pub component: Rc<ConfigSelectorComponent>,
    /// Existing scoped settings owner.
    pub settings: Rc<RefCell<SettingsManager>>,
    /// Complete ordered notifications.
    pub events: Rc<RefCell<Vec<Value>>>,
    /// Live policy independent of theme colors.
    pub styles: Rc<Cell<bool>>,
    /// Actual callback item identity.
    pub captured: Rc<RefCell<Option<Rc<RefCell<ResourceItem>>>>>,
    /// Detached original child array.
    pub children: RefCell<Option<ChildArray>>,
    /// Publisher for later color changes.
    pub publication: ThemeState,
    /// Completion executor for settings queue barriers.
    pub runtime: tokio::runtime::Runtime,
    /// Paths of real storage documents.
    files: [PathBuf; 2],
    /// Removed only after all queue barriers and observations.
    _scratch: crate::scratch::Scratch,
}
impl Harness {
    /// Uses the recorder's ambient cwd, deliberately separate from selected dirs.
    pub fn new(resolved: ResolvedPaths, scopes: &Scopes, dirs: (&str, &str), bold: bool) -> Self {
        let (_, theme) = crate::published::published(&colors(0));
        let options = ConfigSelectorOptions {
            cwd: dirs.0.into(),
            agent_dir: dirs.1.into(),
            path_operations: Rc::new(crate::directories::Directories {
                cwd: Ok("/tmp".into()),
                reads: Cell::new(0),
            }),
            theme,
            collator: Rc::new(Collator::new(&|_| Some("en_US.UTF-8".into())).unwrap()),
            styles_enabled: Rc::new(|| true),
        };
        Self::build(resolved, scopes, options, bold)
    }
    /// Builds supplied native or controlled path effects over real file storage.
    pub fn build(
        resolved: ResolvedPaths,
        scopes: &Scopes,
        mut options: ConfigSelectorOptions,
        bold: bool,
    ) -> Self {
        let scratch = crate::scratch::Scratch::new().unwrap();
        let user = scratch.0.join("user");
        let project = scratch.0.join("project");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(project.join(".maestro")).unwrap();
        let files = [
            user.join("settings.json"),
            project.join(".maestro/settings.json"),
        ];
        seed(&files[0], &scopes.global);
        seed(&files[1], &scopes.project);
        let settings = Rc::new(RefCell::new(SettingsManager::create(
            &project.to_string_lossy(),
            &user.to_string_lossy(),
            ".maestro",
        )));
        let (publication, theme) = crate::published::published(&colors(0));
        let styles = Rc::new(Cell::new(bold));
        let policy = styles.clone();
        options.theme = theme;
        options.styles_enabled = Rc::new(move || policy.get());
        let events = Rc::new(RefCell::new(Vec::new()));
        let close = events.clone();
        let exit = events.clone();
        let render = events.clone();
        let component = Rc::new(ConfigSelectorComponent::new(
            resolved,
            settings.clone(),
            options,
            ConfigSelectorCallbacks {
                on_close: Rc::new(move || close.borrow_mut().push(json!(["close"]))),
                on_exit: Rc::new(move || exit.borrow_mut().push(json!(["exit"]))),
                request_render: Rc::new(move || render.borrow_mut().push(json!(["render"]))),
            },
        ));
        Self {
            component,
            settings,
            events,
            styles,
            captured: Rc::default(),
            children: RefCell::default(),
            publication,
            runtime: tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap(),
            files,
            _scratch: scratch,
        }
    }
    /// Captures every public frame, member order and notification payload.
    pub fn frame(&self, width: usize) -> Frame {
        let global = serde_json::to_value(self.settings.borrow().get_global_settings()).unwrap();
        let project = serde_json::to_value(self.settings.borrow().get_project_settings()).unwrap();
        Frame {
            package_keys: json!({"global":package_keys(&global),"project":package_keys(&project)}),
            lines: self.component.try_render(width).unwrap(),
            list_lines: self
                .component
                .get_resource_list()
                .try_render(width)
                .unwrap(),
            global_keys: keys(&global),
            project_keys: keys(&project),
            global,
            project,
            events: self.events.borrow().clone(),
        }
    }
    /// Waits for existing persistence, then compares real stored JSON and order.
    pub fn persisted(&self, expected: &Scopes) {
        self.runtime.block_on(self.settings.borrow().flush());
        assert!(self.settings.borrow_mut().drain_errors().is_empty());
        for (path, expected) in self.files.iter().zip([&expected.global, &expected.project]) {
            let actual: Value =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            assert_eq!(actual, *expected);
            assert_eq!(
                actual.to_string(),
                expected.to_string(),
                "persisted member order"
            );
        }
    }
}
/// Projects the real live item, omitting only a genuinely absent optional base.
pub(super) fn item_value(item: &ResourceItem) -> Value {
    let mut metadata = json!({"scope":item.metadata.scope,"origin":item.metadata.origin,"source":item.metadata.source});
    if let Some(base) = &item.metadata.base_dir {
        metadata
            .as_object_mut()
            .unwrap()
            .insert("baseDir".into(), json!(base));
    }
    json!({"path":item.path,"enabled":item.enabled,"metadata":metadata,"resourceType":item.resource_type,"displayName":item.display_name,"groupKey":item.group_key,"subgroupKey":item.subgroup_key})
}
