//! The appearance button in a real page: recorded cycles, platform changes and both consumers.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{host, view};
use maestro_theme::BrandMode;
use maestro_web::{AgentInterface, AgentInterfaceProps, ThemeToggle, dom::DomView};
use serde::Deserialize;
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure, prelude::wasm_bindgen};
use web_sys::{Element, HtmlElement};

/// One recorded appearance state.
#[derive(Deserialize)]
struct Step {
    /// The appearance shown.
    theme: String,
    /// The stored value afterwards.
    stored: Option<String>,
    /// Whether the dark mode shows.
    dark: bool,
    /// The icon shown; the recording shows none for an unknown value.
    icon: Option<String>,
}
/// One recorded sequence: the initial state, then two activations.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Cycle {
    /// The value stored before the button appears.
    stored: Option<String>,
    /// Whether the cycle includes system.
    include_system: bool,
    /// The platform appearance.
    color_scheme: String,
    /// The three observed states.
    steps: Vec<Step>,
}
/// A controllable platform dark-preference query, installed as the page's `matchMedia`.
struct Platform {
    /// The query object the page receives.
    query: web_sys::EventTarget,
    /// The page's own `matchMedia`.
    original: JsValue,
    /// The installed replacement.
    _replacement: Closure<dyn Fn(String) -> JsValue>,
}
impl Platform {
    /// Install a query reporting `dark`.
    fn install(dark: bool) -> Self {
        let window = web_sys::window().unwrap();
        let query = web_sys::EventTarget::new().unwrap();
        let original = js_sys::Reflect::get(&window, &"matchMedia".into()).unwrap();
        let shared = query.clone();
        let replacement = Closure::<dyn Fn(String) -> JsValue>::new(move |_| shared.clone().into());
        js_sys::Reflect::set(&window, &"matchMedia".into(), replacement.as_ref()).unwrap();
        let platform = Self {
            query,
            original,
            _replacement: replacement,
        };
        platform.set(dark);
        platform
    }
    /// Report `dark` without announcing it.
    fn set(&self, dark: bool) {
        js_sys::Reflect::set(&self.query, &"matches".into(), &dark.into()).unwrap();
    }
    /// Report `dark` and announce the change.
    fn change(&self, dark: bool) {
        self.set(dark);
        self.query
            .dispatch_event(&web_sys::Event::new("change").unwrap())
            .unwrap();
    }
}
impl Drop for Platform {
    fn drop(&mut self) {
        let window = web_sys::window().unwrap();
        js_sys::Reflect::set(&window, &"matchMedia".into(), &self.original).unwrap();
    }
}
/// The page's local storage.
fn storage() -> web_sys::Storage {
    web_sys::window().unwrap().local_storage().unwrap().unwrap()
}
/// Store `value`, or remove the stored appearance.
fn store(value: Option<&str>) {
    match value {
        Some(value) => storage().set_item("theme", value).unwrap(),
        None => storage().remove_item("theme").unwrap(),
    }
}
/// The page root's applied background colour.
fn applied() -> String {
    let root: HtmlElement = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .document_element()
        .unwrap()
        .unchecked_into();
    root.style()
        .get_property_value("--maestro-color-background")
        .unwrap()
}
/// The background colour of `view`'s pack in the dark or light mode.
fn background(view: &DomView, dark: bool) -> String {
    let mode = if dark {
        BrandMode::Dark
    } else {
        BrandMode::Light
    };
    let presentation = view.pack.presentation(mode).unwrap();
    presentation
        .css_properties()
        .into_iter()
        .find(|(key, _)| key == "--maestro-color-background")
        .unwrap()
        .1
}
/// The catalog icon the button shows, recognised by its first primitive.
fn icon(button: &Element) -> &'static str {
    let first = button.query_selector("svg > *").unwrap().unwrap();
    match (
        first.tag_name().as_str(),
        first.get_attribute("d").unwrap_or_default(),
    ) {
        ("rect", _) => "Monitor",
        ("circle", _) => "Sun",
        ("path", d) if d.starts_with("M20.985") => "Moon",
        other => panic!("unexpected icon {other:?}"),
    }
}
/// Assert one recorded state.
fn observed(view: &DomView, toggle: &ThemeToggle, step: &Step, case: &str) {
    assert_eq!(toggle.theme(), step.theme, "{case}");
    assert_eq!(storage().get_item("theme").unwrap(), step.stored, "{case}");
    assert_eq!(applied(), background(view, step.dark), "{case}");
    // An unknown value shows light; the button shows the sun rather than no icon.
    assert_eq!(
        icon(toggle.root()),
        step.icon.as_deref().unwrap_or("Sun"),
        "{case}"
    );
}

/// The icon-only action has a name and native keyboard activation.
#[wasm_bindgen]
pub async fn theme_toggle_is_named_and_keyboard_operable(import: &js_sys::Function) {
    let shown = view();
    let _host = host(&shown, import);
    let _platform = Platform::install(false);
    store(Some("light"));
    let toggle = ThemeToggle::new(&shown, false).unwrap();
    let parent = crate::contributions_support::container();
    parent.append_child(toggle.root()).unwrap();
    assert_eq!(
        toggle.root().get_attribute("title").as_deref(),
        Some("Toggle theme")
    );
    let tree = crate::contributions_support::driver("driverAria", ".maestro-theme-toggle")
        .await
        .as_string()
        .unwrap();
    assert!(tree.contains("button \"Toggle theme\""), "{tree}");
    toggle
        .root()
        .unchecked_ref::<HtmlElement>()
        .focus()
        .unwrap();
    crate::dialog_support::press("Enter").await;
    assert_eq!(toggle.theme(), "dark");
    assert_eq!(
        storage().get_item("theme").unwrap().as_deref(),
        Some("dark")
    );
    store(None);
}

/// Every stored value, cycle and platform appearance replays its recorded states.
#[wasm_bindgen]
pub fn theme_toggle_replays_all_recorded_cycles(import: &js_sys::Function) {
    let shown = view();
    let _host = host(&shown, import);
    let cycles: Vec<Cycle> =
        serde_json::from_str(include_str!("fixtures/theme_cycles.json")).unwrap();
    assert_eq!(cycles.len(), 24);
    for cycle in &cycles {
        let case = format!(
            "{:?} system={} {}",
            cycle.stored, cycle.include_system, cycle.color_scheme
        );
        let _platform = Platform::install(cycle.color_scheme == "dark");
        store(cycle.stored.as_deref());
        let toggle = ThemeToggle::new(&shown, cycle.include_system).unwrap();
        observed(&shown, &toggle, &cycle.steps[0], &case);
        for step in &cycle.steps[1..] {
            toggle.root().unchecked_ref::<HtmlElement>().click();
            observed(&shown, &toggle, step, &case);
        }
    }
    store(None);
}

/// Platform changes apply only in system mode and only while a button exists; both packs apply,
/// and the session view shows the button only when asked.
#[wasm_bindgen]
pub fn theme_toggle_tracks_system_changes_only_in_system_mode(import: &js_sys::Function) {
    let shown = view();
    let _host = host(&shown, import);
    let platform = Platform::install(false);
    store(None);
    let toggle = ThemeToggle::new(&shown, true).unwrap();
    assert_eq!(applied(), background(&shown, false));
    platform.change(true);
    assert_eq!(applied(), background(&shown, true), "system mode follows");
    toggle.root().unchecked_ref::<HtmlElement>().click();
    platform.change(false);
    assert_eq!(applied(), background(&shown, false), "light stays light");
    platform.change(true);
    assert_eq!(
        applied(),
        background(&shown, false),
        "light ignores the platform"
    );
    store(None);
    platform.change(false);
    drop(toggle);
    platform.change(true);
    assert_eq!(
        applied(),
        background(&shown, false),
        "a dropped button stops listening"
    );
    consumers(&shown, &platform);
    store(None);
}
/// A Signal page applies Signal colours, and the session view places the button on request.
fn consumers(forge: &Rc<DomView>, platform: &Platform) {
    let signal = Rc::new(DomView {
        pack: Rc::new(crate::brand::pack("signal.json").unwrap()),
        ..(**forge).clone()
    });
    store(Some("dark"));
    let toggle = ThemeToggle::new(&signal, false).unwrap();
    assert_eq!(applied(), background(&signal, true));
    drop(toggle);
    platform.set(false);
    let plain: AgentInterface = AgentInterface::new(forge, AgentInterfaceProps::default()).unwrap();
    plain
        .update(AgentInterfaceProps {
            session: Some(Rc::new(maestro_web::BrowserSession::new_local(
                maestro_app::AgentOptions::default(),
            ))),
            ..AgentInterfaceProps::default()
        })
        .unwrap();
    assert!(
        plain
            .root()
            .query_selector(".maestro-theme-toggle")
            .unwrap()
            .is_none()
    );
    plain
        .update(AgentInterfaceProps {
            session: Some(Rc::new(maestro_web::BrowserSession::new_local(
                maestro_app::AgentOptions::default(),
            ))),
            show_theme_toggle: true,
            ..AgentInterfaceProps::default()
        })
        .unwrap();
    assert!(
        plain
            .root()
            .query_selector(".maestro-agent-stats .maestro-theme-toggle")
            .unwrap()
            .is_some()
    );
}
