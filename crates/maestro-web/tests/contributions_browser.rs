//! Runtime browser modules loaded from the fixture package and mounted through the shared callers.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{
    PACKAGE, Text, base, body, call, container, contributions, document, driver, host, origin,
    page_module, slot, text, until, view,
};
use maestro_app::{AgentMessage, JsonObject, SharedHandle};
use maestro_web::{
    MessageRenderer, RUNTIME_MESSAGE_ROUTER,
    browser::sandbox_value::BrowserSandboxValue,
    components::message_renderer_registry::dom::render_message,
    components::sandbox::runtime_message_router::MessageConsumer,
    contributions::{
        BrowserContribution, ContributionDescriptor, ContributionFailure, ContributionHost,
        ContributionView, SlotMount, admit, register_contribution, route_contribution,
        slot_contributions,
    },
    dom::{DomContent, DomRoot, DomView, MountedContent},
    register_message_renderer, register_tool_renderer, set_show_json_mode,
    tools::{
        dom::render_tool,
        types::{
            ToolParams, ToolRenderResult, ToolRenderer, ToolRendererBinding, ToolResultMessageView,
            ToolUiError,
        },
    },
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure, prelude::wasm_bindgen};
use web_sys::{Element, HtmlElement, Node};

/// Failure sentences of one load.
fn sentences(failures: &[ContributionFailure]) -> Vec<String> {
    failures.iter().map(ToString::to_string).collect()
}
/// A view, its host and the loaded fixture package.
async fn loaded(
    import_module: &js_sys::Function,
) -> (
    Rc<DomView>,
    ContributionHost,
    maestro_web::contributions::ModuleRegistrations,
    Vec<String>,
) {
    let view = view();
    let host = host(&view, import_module);
    let (registrations, failures) = host.load(&contributions().await, &base()).await;
    (view, host, registrations, sentences(&failures))
}
/// Parse a descriptor from its JSON fields.
fn descriptor(fields: Value) -> ContributionDescriptor {
    serde_json::from_value(fields).unwrap()
}
/// Mount the page rendering of `tool` with `{"text": text}` into `parent`.
fn show_tool(
    mounted: &mut MountedContent,
    parent: &Node,
    view: &DomView,
    tool: &str,
    input: (&str, bool),
) -> bool {
    let params: JsonObject = serde_json::from_value(json!({ "text": input.0 })).unwrap();
    let shown =
        render_tool::<JsonObject, Value>(tool, Some(ToolParams::object(&params)), None, input.1)
            .unwrap();
    mounted.show(shown.content.as_ref(), parent, view).unwrap();
    shown.is_custom
}
/// The user message the module renders.
fn user_message() -> AgentMessage {
    serde_json::from_value(json!({"role": "user", "content": "hi", "timestamp": 1})).unwrap()
}
/// Text the page message renderer for `message` shows, if one is registered.
fn message_text(view: &DomView, message: &AgentMessage) -> Option<String> {
    let content = render_message(message).unwrap()?;
    let parent: Node = container().into();
    MountedContent::default()
        .show(content.as_ref(), &parent, view)
        .unwrap();
    Some(text(&parent))
}
/// The `data-updates` count the module element under `parent` recorded.
fn updates(parent: &Node) -> Option<String> {
    parent
        .unchecked_ref::<Element>()
        .query_selector("[data-updates]")
        .unwrap()
        .and_then(|element| element.get_attribute("data-updates"))
}
/// A descriptor requiring the greeting API.
fn greeting_required() -> ContributionDescriptor {
    descriptor(
        json!({"id": "needs", "kind": "card", "target": "home", "requires": ["fixture.greeting"]}),
    )
}
/// Same-module page renderer replacing the module's tool renderer.
struct NativeTool;
impl ToolRenderer<JsonObject, Value, Box<dyn DomContent>> for NativeTool {
    fn render(
        &self,
        params: Option<&JsonObject>,
        _: Option<ToolResultMessageView<'_, Value>>,
        _: bool,
    ) -> Result<ToolRenderResult<Box<dyn DomContent>>, ToolUiError> {
        assert_eq!(params.map(JsonObject::len), Some(1));
        Ok(ToolRenderResult {
            content: Box::new(Text("native")),
            is_custom: true,
        })
    }
}
/// Same-module page message renderer replacing the module's.
struct NativeMessage;
impl MessageRenderer<maestro_app::NoCustomMessages, Box<dyn DomContent>> for NativeMessage {
    fn render(&self, _: &AgentMessage) -> Result<Box<dyn DomContent>, ToolUiError> {
        Ok(Box::new(Text("replacement")))
    }
}
/// A resource entry name the page fetched, relative to the origin.
fn fetched() -> Vec<String> {
    let performance =
        js_sys::Reflect::get(&web_sys::window().unwrap(), &"performance".into()).unwrap();
    let entries: js_sys::Function = js_sys::Reflect::get(&performance, &"getEntriesByType".into())
        .unwrap()
        .dyn_into()
        .unwrap();
    let list: js_sys::Array = entries
        .call1(&performance, &"resource".into())
        .unwrap()
        .dyn_into()
        .unwrap();
    let prefix = origin();
    list.iter()
        .filter_map(|entry| {
            js_sys::Reflect::get(&entry, &"name".into())
                .ok()?
                .as_string()
        })
        .filter_map(|name| name.strip_prefix(&prefix).map(str::to_owned))
        .collect()
}

/// Two separately built modules initialize under the page policy and register through the package path.
#[wasm_bindgen]
pub async fn contributions_load_two_independent_modules_without_rebuilding_the_host(
    import_module: js_sys::Function,
) {
    let (view, _host, _registrations, failures) = loaded(&import_module).await;
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].starts_with("modules/missing/module.js: "),
        "{failures:?}"
    );
    let resources = fetched();
    for asset in [
        "modules/a/module.js",
        "modules/a/module_bg.wasm",
        "modules/b/module.js",
        "modules/b/module_bg.wasm",
    ] {
        assert!(
            resources.contains(&format!("/package/{asset}")),
            "{asset}: {resources:?}"
        );
    }
    let parent: Node = container().into();
    assert!(show_tool(
        &mut MountedContent::default(),
        &parent,
        &view,
        "fixture_tool",
        ("x", false)
    ));
    assert_eq!(
        message_text(&view, &user_message()).as_deref(),
        Some("message:user:hi")
    );
    let page = route_contribution("fixture-page").unwrap().unwrap();
    assert_eq!(
        (page.package.as_str(), page.descriptor.id.as_str()),
        (PACKAGE, "fixture-page")
    );
    let home: Vec<String> = slot_contributions("home")
        .into_iter()
        .map(|(c, _)| c.descriptor.id)
        .collect();
    assert_eq!(
        home,
        ["fixture-frame", "fixture-card", "fixture-needs-missing"]
    );
    assert_eq!(admit(&greeting_required(), None), Ok(()));
    // The page policy refuses eval and inline script while the modules above initialized.
    assert!(js_sys::eval("1").is_err());
    let script = document().create_element("script").unwrap();
    script.set_text_content(Some("window.qaInline = true"));
    body().append_child(&script).unwrap();
    assert!(
        js_sys::Reflect::get(&web_sys::window().unwrap(), &"qaInline".into())
            .unwrap()
            .is_undefined()
    );
}

/// Module URLs that leave the package base are refused before the page import runs.
#[wasm_bindgen]
pub async fn module_urls_outside_the_package_base_are_refused(import_module: js_sys::Function) {
    let seen = Rc::new(RefCell::new(Vec::<String>::new()));
    let (recorded, forward) = (Rc::clone(&seen), import_module.clone());
    let spy = Closure::<dyn Fn(JsValue) -> JsValue>::new(move |url: JsValue| {
        recorded.borrow_mut().push(url.as_string().unwrap());
        forward.call1(&JsValue::NULL, &url).unwrap()
    });
    let view = view();
    let host = host(&view, spy.as_ref().unchecked_ref());
    let card = |entry: &str, wasm: &str| BrowserContribution {
        package: PACKAGE.into(),
        descriptor: descriptor(json!({"id": entry, "kind": "card", "target": "home",
            "module": {"entry": entry, "wasm": wasm, "register": "register"}})),
    };
    let refused = [
        card("../outside/module.js", "modules/a/module_bg.wasm"),
        card(
            "https://elsewhere.example/module.js",
            "modules/a/module_bg.wasm",
        ),
        card("//elsewhere.example/module.js", "modules/a/module_bg.wasm"),
        card("modules/a/module.js", "../outside/module_bg.wasm"),
    ];
    for contribution in &refused {
        let (_registrations, failures) =
            host.load(std::slice::from_ref(contribution), &base()).await;
        assert_eq!(failures.len(), 1);
        assert!(
            failures[0].to_string().contains(" is outside "),
            "{}",
            failures[0]
        );
    }
    // A base without its trailing slash would admit a sibling directory sharing its prefix.
    let sibling = card("package-evil/module.js", "package-evil/module_bg.wasm");
    let (_registrations, failures) = host
        .load(
            std::slice::from_ref(&sibling),
            &format!("{}/package", origin()),
        )
        .await;
    assert_eq!(failures.len(), 1);
    assert!(seen.borrow().is_empty(), "{:?}", seen.borrow());
    let module_a: Vec<BrowserContribution> = contributions()
        .await
        .into_iter()
        .filter(|c| {
            c.descriptor
                .module
                .as_ref()
                .is_some_and(|m| m.entry == "modules/a/module.js")
        })
        .collect();
    let (_registrations, failures) = host.load(&module_a, &base()).await;
    assert!(failures.is_empty(), "{failures:?}");
    assert_eq!(*seen.borrow(), [format!("{}modules/a/module.js", base())]);
}

/// A slot contract and a provided API admit, and an input-free contribution admits anywhere.
#[wasm_bindgen]
pub async fn admission_accepts_matching_input_when_the_required_api_is_provided(
    import_module: js_sys::Function,
) {
    let (_view, _host, _registrations, _) = loaded(&import_module).await;
    let both = descriptor(
        json!({"id": "tab", "kind": "tab", "target": "fixture-page.tabs",
        "input": "fixture.tab", "requires": ["fixture.greeting"]}),
    );
    assert_eq!(admit(&both, Some("fixture.tab")), Ok(()));
    let free = descriptor(json!({"id": "free", "kind": "card", "target": "home"}));
    for supplied in [Some("fixture.tab"), Some("other"), None] {
        assert_eq!(admit(&free, supplied), Ok(()));
    }
    assert_eq!(admit(&greeting_required(), Some("anything")), Ok(()));
}

/// Built-in content inside a slotted contribution.
struct BuiltIn;
/// The built-in code listing element.
struct CodeListing;
impl DomContent for CodeListing {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let block = view.document.create_element("code-block").unwrap();
        block.set_attribute("code", "let built_in = 1;").unwrap();
        block.set_attribute("language", "rust").unwrap();
        Ok(DomRoot::new(block))
    }
    fn update(&self, _: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        Ok(())
    }
}
impl ContributionView for BuiltIn {
    fn content(&self, _: &Value) -> Box<dyn DomContent> {
        Box::new(CodeListing)
    }
}

/// One slot caller mounts a built-in element, module elements and an isolated frame.
#[wasm_bindgen]
pub async fn slot_mounts_built_in_module_and_frame_through_one_caller(
    import_module: js_sys::Function,
) {
    let (view, _host, _registrations, _) = loaded(&import_module).await;
    let built_in = BrowserContribution {
        package: "built-in".into(),
        descriptor: descriptor(json!({"id": "listing", "kind": "card", "target": "home"})),
    };
    let _listing = register_contribution(built_in, Rc::new(BuiltIn));
    let parent = container();
    let mut home = SlotMount::new("home", parent.clone());
    let failures = home.show(&json!({"title": "Home"}), &view);
    assert_eq!(
        sentences(&failures),
        ["fixture-needs-missing: missing host API absent.api"]
    );
    let wrappers =
        std::iter::successors(parent.first_element_child(), Element::next_element_sibling);
    let kinds: Vec<String> = wrappers
        .map(|wrapper| wrapper.first_element_child().unwrap())
        .map(|root| match root.query_selector("iframe").unwrap() {
            Some(frame) => format!(
                "frame:{}",
                frame.get_attribute("sandbox").unwrap_or_default()
            ),
            None => root.local_name(),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "frame:allow-scripts allow-modals",
            "qa-card-view",
            "div",
            "code-block"
        ]
    );
    assert_eq!(
        text(&parent.query_selector("qa-card-view").unwrap().unwrap()),
        "card:Home"
    );
    let listing = parent
        .query_selector("code-block pre code")
        .unwrap()
        .unwrap();
    assert_eq!(text(&listing), "let built_in = 1;");
}

/// The module's tool element receives the original parameters and streaming flag and keeps its root.
#[wasm_bindgen]
pub async fn module_tool_element_receives_params_and_streaming_state(
    import_module: js_sys::Function,
) {
    let (view, _host, _registrations, _) = loaded(&import_module).await;
    let parent: Node = container().into();
    let mut mounted = MountedContent::default();
    assert!(show_tool(
        &mut mounted,
        &parent,
        &view,
        "fixture_tool",
        ("one", true)
    ));
    assert_eq!(text(&parent), "tool:one:true:hello A");
    let root = mounted.root().cloned().unwrap();
    assert_eq!(updates(&parent), None);
    assert!(show_tool(
        &mut mounted,
        &parent,
        &view,
        "fixture_tool",
        ("two", false)
    ));
    assert_eq!(mounted.root(), Some(&root));
    assert_eq!(text(&parent), "tool:two:false:hello A");
    assert_eq!(updates(&parent).as_deref(), Some("1"));
}

/// The module's message element sees the original message's role and content.
#[wasm_bindgen]
pub async fn module_message_element_receives_the_original_message(import_module: js_sys::Function) {
    let (view, _host, _registrations, _) = loaded(&import_module).await;
    assert_eq!(
        message_text(&view, &user_message()).as_deref(),
        Some("message:user:hi")
    );
    let quoted: AgentMessage =
        serde_json::from_value(json!({"role": "user", "content": "<b>雪</b>", "timestamp": 2}))
            .unwrap();
    assert_eq!(
        message_text(&view, &quoted).as_deref(),
        Some("message:user:<b>雪</b>")
    );
}

/// Unknown tools use the ordinary card, JSON mode bypasses the module and a later replacement wins.
#[wasm_bindgen]
pub async fn unknown_tool_json_mode_and_replacement_have_their_outcomes(
    import_module: js_sys::Function,
) {
    let (view, _host, _registrations, _) = loaded(&import_module).await;
    let unknown: Node = container().into();
    assert!(!show_tool(
        &mut MountedContent::default(),
        &unknown,
        &view,
        "unknown_tool",
        ("x", false)
    ));
    assert!(text(&unknown).starts_with("Tool Call"));
    set_show_json_mode(true);
    let bypassed: Node = container().into();
    let custom = show_tool(
        &mut MountedContent::default(),
        &bypassed,
        &view,
        "fixture_tool",
        ("x", false),
    );
    set_show_json_mode(false);
    assert!(!custom);
    assert!(text(&bypassed).starts_with("Tool Call"));
    let module: Node = container().into();
    assert!(show_tool(
        &mut MountedContent::default(),
        &module,
        &view,
        "fixture_tool",
        ("x", false)
    ));
    assert_eq!(text(&module), "tool:x:false:hello A");
    register_tool_renderer(
        "fixture_tool",
        ToolRendererBinding::from_dom::<JsonObject, Value>(Rc::new(NativeTool)),
    );
    let replaced: Node = container().into();
    assert!(show_tool(
        &mut MountedContent::default(),
        &replaced,
        &view,
        "fixture_tool",
        ("x", false)
    ));
    assert_eq!(text(&replaced), "native");
}

/// Missing APIs and incompatible slot input render their sentence while the rest of the slot mounts.
#[wasm_bindgen]
pub async fn missing_api_and_incompatible_slot_input_are_visible_failures(
    import_module: js_sys::Function,
) {
    let (_view, _host, _registrations, _) = loaded(&import_module).await;
    let home = slot("home", &json!({"title": "Home"}));
    let alert = home.query_selector("[role=alert]").unwrap().unwrap();
    assert_eq!(
        text(&alert),
        "fixture-needs-missing: missing host API absent.api"
    );
    assert_eq!(
        text(&home.query_selector("qa-card-view").unwrap().unwrap()),
        "card:Home"
    );
    let pages = slot("pages", &json!({"title": "Fixture"}));
    let tabs = pages.query_selector("maestro-slot").unwrap().unwrap();
    let alerts = tabs.query_selector_all("[role=alert]").unwrap();
    assert_eq!(alerts.length(), 1);
    assert_eq!(
        text(&alerts.item(0).unwrap()),
        "fixture-bad-tab: expects other.contract, slot supplies fixture.tab"
    );
    assert!(text(&tabs).starts_with("tab:first"));
}

/// A module that cannot initialize reports one failure and keeps nothing it acquired.
#[wasm_bindgen]
pub async fn failed_initialization_releases_and_reports(import_module: js_sys::Function) {
    let (view, host, registrations, failures) = loaded(&import_module).await;
    assert_eq!(failures.len(), 1);
    drop(registrations);
    // Module B provides its API, then fails to redefine its element names in this page.
    let (_registrations, failures) = host.load(&contributions().await, &base()).await;
    let failures = sentences(&failures);
    assert_eq!(failures.len(), 2, "{failures:?}");
    assert!(
        failures[0].starts_with("modules/b/module.js: ")
            && failures[0].contains("NotSupportedError")
    );
    assert!(failures[1].starts_with("modules/missing/module.js: "));
    assert_eq!(route_contribution("fixture-page"), Ok(None));
    assert_eq!(
        admit(&greeting_required(), None),
        Err(ContributionFailure::MissingApi {
            id: "needs".into(),
            api: "fixture.greeting".into()
        })
    );
    let home: Vec<String> = slot_contributions("home")
        .into_iter()
        .map(|(c, _)| c.descriptor.id)
        .collect();
    assert_eq!(home, ["fixture-frame"]);
    let tool: Node = container().into();
    assert!(show_tool(
        &mut MountedContent::default(),
        &tool,
        &view,
        "fixture_tool",
        ("x", false)
    ));
    assert_eq!(
        text(&tool),
        "fixture-tool: missing host API fixture.greeting"
    );
}

/// Undeclared registrations and duplicate APIs are refused; a repeated element name needs a fresh name or reload.
#[wasm_bindgen]
pub async fn undeclared_and_duplicate_definitions_are_rejected(import_module: js_sys::Function) {
    let (view, host, registrations, _) = loaded(&import_module).await;
    let window = web_sys::window().unwrap();
    let recorded = |name: &str| {
        js_sys::Reflect::get(&window, &name.into())
            .unwrap()
            .as_string()
    };
    assert_eq!(
        recorded("qaUndeclared").as_deref(),
        Some("fixture-undeclared: not declared by its package")
    );
    assert_eq!(
        recorded("qaDuplicateApi").as_deref(),
        Some("fixture.greeting: host API already provided")
    );
    let defined = window.custom_elements().get("qa-page-view");
    drop(registrations);
    let (_registrations, failures) = host.load(&contributions().await, &base()).await;
    assert!(
        sentences(&failures)
            .iter()
            .any(|failure| failure.contains("NotSupportedError"))
    );
    assert_eq!(window.custom_elements().get("qa-page-view"), defined);
    // Defining the built-in elements again for the same page is not attempted.
    let again =
        ContributionHost::<maestro_app::NoCustomMessages>::new(&view, None, import_module.clone());
    assert!(again.is_ok());
}

/// Disabling removes the module's views and keeps a later explicit replacement serving.
#[wasm_bindgen]
pub async fn disabling_releases_only_still_owned_registrations(import_module: js_sys::Function) {
    let (view, _host, registrations, _) = loaded(&import_module).await;
    let home = slot("home", &json!({"title": "Home"}));
    assert_eq!(home.child_element_count(), 3);
    register_tool_renderer(
        "fixture_tool",
        ToolRendererBinding::from_dom::<JsonObject, Value>(Rc::new(NativeTool)),
    );
    drop(registrations);
    assert_eq!(home.child_element_count(), 0);
    let tool: Node = container().into();
    assert!(show_tool(
        &mut MountedContent::default(),
        &tool,
        &view,
        "fixture_tool",
        ("x", false)
    ));
    assert_eq!(text(&tool), "native");
    assert_eq!(message_text(&view, &user_message()), None);
    assert_eq!(route_contribution("fixture-page"), Ok(None));
    assert!(
        slot_contributions("home").is_empty() && slot_contributions("fixture-page.tabs").is_empty()
    );
    assert!(matches!(
        admit(&greeting_required(), None),
        Err(ContributionFailure::MissingApi { .. })
    ));
}

/// Removing the module's message renderer keeps a later replacement and only then forgets the role.
#[wasm_bindgen]
pub async fn removing_a_message_renderer_keeps_a_later_replacement(
    import_module: js_sys::Function,
) {
    let (view, host, registrations, _) = loaded(&import_module).await;
    drop(registrations);
    assert_eq!(message_text(&view, &user_message()), None);
    let (registrations, _) = host.load(&contributions().await, &base()).await;
    assert_eq!(
        message_text(&view, &user_message()).as_deref(),
        Some("message:user:hi")
    );
    let replacement: Rc<dyn MessageRenderer<maestro_app::NoCustomMessages, Box<dyn DomContent>>> =
        Rc::new(NativeMessage);
    register_message_renderer("user", replacement);
    drop(registrations);
    assert_eq!(
        message_text(&view, &user_message()).as_deref(),
        Some("replacement")
    );
}

/// Reconnected module elements and slots keep exactly one live listener; detached ones receive nothing.
#[wasm_bindgen]
pub async fn reconnect_and_detached_updates_leave_one_live_listener(
    import_module: js_sys::Function,
) {
    let (view, _host, _registrations, _) = loaded(&import_module).await;
    let parent = container();
    let node: Node = parent.clone().into();
    let mut mounted = MountedContent::default();
    show_tool(&mut mounted, &node, &view, "fixture_tool", ("one", false));
    show_tool(&mut mounted, &node, &view, "fixture_tool", ("two", false));
    parent.remove();
    body().append_child(&parent).unwrap();
    show_tool(&mut mounted, &node, &view, "fixture_tool", ("three", false));
    assert_eq!(updates(&node).as_deref(), Some("2"));
    parent.remove();
    show_tool(&mut mounted, &node, &view, "fixture_tool", ("four", false));
    assert_eq!(updates(&node).as_deref(), Some("2"));
    // Each connected slot answers a registration change once.
    let tabs = slot("fixture-page.tabs", &json!({"tab": "x"}));
    let announce = || {
        let other = BrowserContribution {
            package: "built-in".into(),
            descriptor: descriptor(
                json!({"id": "elsewhere", "kind": "card", "target": "elsewhere"}),
            ),
        };
        drop(register_contribution(other, Rc::new(BuiltIn)));
    };
    let first = tabs.query_selector("qa-tab-view").unwrap().unwrap();
    let before = count(&first);
    announce();
    assert_eq!(count(&first), before + 1);
    tabs.remove();
    body().append_child(&tabs).unwrap();
    let second = tabs.query_selector("qa-tab-view").unwrap().unwrap();
    assert_ne!(first, second);
    assert_eq!(count(&second), 0);
    announce();
    assert_eq!(count(&second), 1);
    tabs.remove();
    assert_eq!(tabs.child_element_count(), 0);
    announce();
    assert_eq!((count(&first), count(&second)), (before + 1, 1));
}
/// Input events a module tab element received.
fn count(element: &Element) -> u32 {
    element
        .get_attribute("data-updates")
        .map_or(0, |count| count.parse().unwrap())
}

/// Element classes stay defined after their views disconnect, and later views mount without redefinition.
#[wasm_bindgen]
pub async fn class_definition_outlives_mounted_views(import_module: js_sys::Function) {
    let (view, _host, _registrations, _) = loaded(&import_module).await;
    let registry = web_sys::window().unwrap().custom_elements();
    let parent = container();
    let mut home = SlotMount::new("home", parent.clone());
    home.show(&json!({"title": "One"}), &view);
    let first = parent.query_selector("qa-card-view").unwrap().unwrap();
    let copy = registry.get("copy-button");
    home.clear();
    assert_eq!(parent.child_element_count(), 0);
    assert!(!registry.get("qa-card-view").is_undefined());
    home.show(&json!({"title": "Two"}), &view);
    let second = parent.query_selector("qa-card-view").unwrap().unwrap();
    assert_ne!(first, second);
    assert_eq!(text(&second), "card:Two");
    assert!(
        ContributionHost::<maestro_app::NoCustomMessages>::new(&view, None, import_module.clone())
            .is_ok()
    );
    assert_eq!(registry.get("copy-button"), copy);
}

/// A module registering from inside a lifecycle callback completes, and the slot shows the result.
#[wasm_bindgen]
pub async fn reentrant_host_calls_complete_from_a_callback(import_module: js_sys::Function) {
    let (_view, _host, _registrations, _) = loaded(&import_module).await;
    assert!(
        !slot_contributions("fixture-page.tabs")
            .iter()
            .any(|(c, _)| c.descriptor.id == "fixture-late")
    );
    let pages = slot("pages", &json!({"title": "Fixture"}));
    let ids: Vec<String> = slot_contributions("fixture-page.tabs")
        .into_iter()
        .map(|(c, _)| c.descriptor.id)
        .collect();
    assert_eq!(ids, ["fixture-tab", "fixture-bad-tab", "fixture-late"]);
    let tabs = pages.query_selector("maestro-slot").unwrap().unwrap();
    assert_eq!(tabs.query_selector_all("qa-tab-view").unwrap().length(), 2);
    assert_eq!(text(&tabs.last_element_child().unwrap()), "tab:first");
}

/// Messages from the frame the router registered, as plain data.
struct Received(tokio::sync::mpsc::UnboundedSender<Value>);
impl MessageConsumer<BrowserSandboxValue> for Received {
    fn handle_message(
        &self,
        message: BrowserSandboxValue,
    ) -> maestro_app::BoxFuture<Result<(), maestro_app::DiagnosticErrorInfo>> {
        let text = js_sys::JSON::stringify(&message.0)
            .unwrap()
            .as_string()
            .unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        if value["type"]
            .as_str()
            .is_some_and(|kind| kind.starts_with("qa-"))
        {
            // The receiver may already be gone once its export settled.
            let _ = self.0.send(value);
        }
        Box::pin(async { Ok(()) })
    }
}
/// Mount the home slot and subscribe to the frame contribution's admitted messages.
async fn frame_messages(
    import_module: &js_sys::Function,
) -> (
    Element,
    SlotMount,
    Rc<DomView>,
    impl Sized,
    tokio::sync::mpsc::UnboundedReceiver<Value>,
) {
    let (view, host, registrations, _) = loaded(import_module).await;
    let parent = container();
    let mut home = SlotMount::new("home", parent.clone());
    home.show(&json!({"title": "Home"}), &view);
    (parent, home, view, (host, registrations), frame_receiver())
}
/// Subscribe to the frame contribution's admitted messages; each load registers the frame anew.
fn frame_receiver() -> tokio::sync::mpsc::UnboundedReceiver<Value> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    let consumer: SharedHandle<dyn MessageConsumer<BrowserSandboxValue>> =
        SharedHandle::new(Received(sender));
    RUNTIME_MESSAGE_ROUTER.with(|router| router.add_consumer("fixture-frame", consumer));
    receiver
}

/// The router delivers the frame's own messages and drops a forged one from the page.
#[wasm_bindgen]
pub async fn isolated_frame_accepts_only_its_registered_source(import_module: js_sys::Function) {
    let (parent, _home, _view, _kept, mut received) = frame_messages(&import_module).await;
    let first = received.recv().await.unwrap();
    assert_eq!(
        (first["type"].as_str(), first["text"].as_str()),
        (Some("qa-frame"), Some("frame:Home"))
    );
    let window = web_sys::window().unwrap();
    let forged =
        js_sys::JSON::parse(r#"{"sandboxId":"fixture-frame","type":"qa-forged"}"#).unwrap();
    window.post_message(&forged, "*").unwrap();
    let frame: web_sys::HtmlIFrameElement = parent
        .query_selector("iframe")
        .unwrap()
        .unwrap()
        .unchecked_into();
    let ping = js_sys::JSON::parse(r#"{"type":"qa-ping"}"#).unwrap();
    frame
        .content_window()
        .unwrap()
        .post_message(&ping, "*")
        .unwrap();
    let next = received.recv().await.unwrap();
    assert_eq!(next["type"].as_str(), Some("qa-pong"));
    assert!(received.try_recv().is_err());
}

/// The isolated frame has an opaque origin, no storage or cookies and no reach into the host page.
#[wasm_bindgen]
pub async fn isolated_frame_denies_credentials_and_native_access(import_module: js_sys::Function) {
    let (parent, _home, _view, _kept, mut received) = frame_messages(&import_module).await;
    let probe = received.recv().await.unwrap();
    assert_eq!(probe["origin"].as_str(), Some("null"));
    for field in ["cookie", "storage", "parent"] {
        assert_eq!(
            probe[field].as_str(),
            Some("SecurityError"),
            "{field}: {probe}"
        );
    }
    let frame: web_sys::HtmlIFrameElement = parent
        .query_selector("iframe")
        .unwrap()
        .unwrap()
        .unchecked_into();
    assert!(frame.content_document().is_none());
    let sandbox = frame.get_attribute("sandbox").unwrap();
    assert!(
        !sandbox
            .split_whitespace()
            .any(|token| token == "allow-same-origin"),
        "{sandbox}"
    );
}

/// Mounted elements expose role and name, follow keyboard order and are found by browser find.
#[wasm_bindgen]
pub async fn exercised_elements_keep_accessible_names_focus_and_find_text(
    import_module: js_sys::Function,
) {
    let (view, _host, _registrations, _) = loaded(&import_module).await;
    let parent = container();
    for code in ["first listing", "second listing"] {
        let block = document().create_element("code-block").unwrap();
        block.set_attribute("code", code).unwrap();
        parent.append_child(&block).unwrap();
    }
    let mut home = SlotMount::new("home", container());
    home.show(&json!({"title": "Home"}), &view);
    let tree = driver("driverAria", "body").await.as_string().unwrap();
    assert_eq!(tree.matches("button \"Copy code\"").count(), 2, "{tree}");
    assert!(
        tree.contains("alert")
            && tree.contains("fixture-needs-missing: missing host API absent.api"),
        "{tree}"
    );
    let buttons = parent.query_selector_all("copy-button button").unwrap();
    for index in 0..2 {
        driver("driverPress", "Tab").await;
        let active = document().active_element().unwrap();
        assert_eq!(
            Some(active),
            buttons
                .item(index)
                .map(wasm_bindgen::JsCast::unchecked_into),
            "focus {index}"
        );
    }
    let find: js_sys::Function = js_sys::Reflect::get(&web_sys::window().unwrap(), &"find".into())
        .unwrap()
        .dyn_into()
        .unwrap();
    assert_eq!(
        find.call1(&JsValue::NULL, &"second listing".into())
            .unwrap()
            .as_bool(),
        Some(true)
    );
    let window = web_sys::window().unwrap();
    let selection = js_sys::Reflect::get(&window, &"getSelection".into()).unwrap();
    let selection = selection
        .unchecked_ref::<js_sys::Function>()
        .call0(&window)
        .unwrap();
    assert_eq!(
        String::from(selection.unchecked_ref::<js_sys::Object>().to_string()),
        "second listing"
    );
    let anchor: Node = js_sys::Reflect::get(&selection, &"anchorNode".into())
        .unwrap()
        .unchecked_into();
    assert!(parent.last_element_child().unwrap().contains(Some(&anchor)));
}

/// The copy control writes its attribute text, shows success and restores after two seconds.
#[wasm_bindgen]
pub async fn copy_control_reports_success_then_restores(import_module: js_sys::Function) {
    let page = page_module(&import_module).await;
    let view = view();
    let _host = host(&view, &import_module);
    let clipboard = crate::contributions_support::Recorded::install(
        crate::contributions_support::Clipboard::Accept,
    );
    let control = document().create_element("copy-button").unwrap();
    control.set_attribute("text", "copied text").unwrap();
    control.set_attribute("show-text", "").unwrap();
    body().append_child(&control).unwrap();
    let idle = control.inner_html();
    let button: HtmlElement = control
        .query_selector("button")
        .unwrap()
        .unwrap()
        .unchecked_into();
    assert_eq!(button.title(), "Copy");
    let clicked = js_sys::Date::now();
    button.click();
    until(&page, &control, || text(&control) == "Copied!").await;
    assert_eq!(clipboard.texts(), ["copied text"]);
    assert_ne!(control.inner_html(), idle);
    until(&page, &control, || text(&control).is_empty()).await;
    assert!(js_sys::Date::now() - clicked >= 2000.0);
    assert_eq!(control.inner_html(), idle);
}

/// Call a page helper and read its result as plain data.
async fn plain(page: &JsValue, name: &str, args: &js_sys::Array) -> Value {
    let value = call(page, name, args).await;
    let text = js_sys::JSON::stringify(&value)
        .unwrap()
        .as_string()
        .unwrap();
    serde_json::from_str(&text).unwrap()
}
/// Raw and compressed sizes of the host bundle and both module artifacts.
async fn asset_sizes(page: &JsValue) -> Value {
    let assets: js_sys::Array = [
        "/sandbox.js",
        "/sandbox_bg.wasm",
        "/package/modules/a/module.js",
        "/package/modules/a/module_bg.wasm",
        "/package/modules/b/module.js",
        "/package/modules/b/module_bg.wasm",
    ]
    .into_iter()
    .map(|asset| JsValue::from(format!("{}{asset}", origin())))
    .collect();
    let sizes = plain(page, "sizes", &js_sys::Array::of1(&assets)).await;
    let measured = sizes.as_object().unwrap();
    assert_eq!(measured.len(), 6);
    assert!(
        measured
            .values()
            .all(|size| size["raw"].as_u64() > Some(0) && size["gzip"].as_u64() > Some(0))
    );
    sizes
}

/// Mutation counts, frame intervals, load time, repeated mount memory and asset sizes, recorded without thresholds.
#[wasm_bindgen]
pub async fn repeated_mount_and_unmount_records_mutations_timings_memory_and_sizes(
    import_module: js_sys::Function,
) {
    let page = page_module(&import_module).await;
    let view = view();
    let host = host(&view, &import_module);
    let started = js_sys::Date::now();
    let (_registrations, _) = host.load(&contributions().await, &base()).await;
    let load_ms = js_sys::Date::now() - started;
    let parent = container();
    let node: Node = parent.clone().into();
    let take: js_sys::Function = call(&page, "recorder", &js_sys::Array::of1(&parent))
        .await
        .dyn_into()
        .unwrap();
    let count = || take.call0(&JsValue::NULL).unwrap().as_f64().unwrap();
    let mut mounted = MountedContent::default();
    show_tool(&mut mounted, &node, &view, "fixture_tool", ("one", false));
    let mount = count();
    show_tool(&mut mounted, &node, &view, "fixture_tool", ("two", false));
    let update = count();
    show_tool(&mut mounted, &node, &view, "unknown_tool", ("three", false));
    let replace = count();
    assert!(mount > 0.0 && update > 0.0 && replace > 0.0);
    let frames = plain(&page, "frames", &js_sys::Array::of1(&JsValue::from(10))).await;
    assert_eq!(frames.as_array().map(Vec::len), Some(10));
    let no_arguments = js_sys::Array::new();
    let heap_before = plain(&page, "heap", &no_arguments).await;
    let mut home = SlotMount::new("home", container());
    for cycle in 0..200 {
        home.show(&json!({"title": cycle}), &view);
        home.clear();
    }
    let heap_after = plain(&page, "heap", &no_arguments).await;
    let report = json!({
        "mutations": {"mount": mount, "update": update, "replace": replace},
        "frameIntervalsMs": frames,
        "loadMs": load_ms,
        "heapBytes": {"before": heap_before, "after200MountCycles": heap_after},
        "assets": asset_sizes(&page).await,
    });
    web_sys::console::log_1(&format!("MEASURE {report}").into());
}

/// An isolated frame follows a changed slot input by loading again with it.
#[wasm_bindgen]
pub async fn isolated_frame_receives_changed_input(import_module: js_sys::Function) {
    let (parent, mut home, view, _kept, mut received) = frame_messages(&import_module).await;
    let first = received.recv().await.unwrap();
    assert_eq!(first["text"].as_str(), Some("frame:Home"));
    let before = parent.query_selector("iframe").unwrap().unwrap();
    let failures = home.show(&json!({"title": "Next"}), &view);
    assert_eq!(
        sentences(&failures),
        ["fixture-needs-missing: missing host API absent.api"]
    );
    let after = parent.query_selector("iframe").unwrap().unwrap();
    assert_ne!(before, after, "the frame kept its first input");
    assert_eq!(parent.query_selector_all("iframe").unwrap().length(), 1);
    let next = frame_receiver().recv().await.unwrap();
    assert_eq!(next["text"].as_str(), Some("frame:Next"));
    // Showing the same input again keeps the loaded frame.
    home.show(&json!({"title": "Next"}), &view);
    assert_eq!(parent.query_selector("iframe").unwrap(), Some(after));
}

/// Replacing a registered document reloads it even when the slot input is unchanged.
#[wasm_bindgen]
pub async fn replacing_a_document_with_unchanged_input_loads_the_new_document(
    import_module: js_sys::Function,
) {
    let (parent, mut home, view, _kept, mut received) = frame_messages(&import_module).await;
    assert_eq!(received.recv().await.unwrap()["text"], "frame:Home");
    let wrapper = parent.first_element_child().unwrap();
    let before = parent.query_selector("iframe").unwrap().unwrap();
    assert_eq!(
        wrapper.query_selector("iframe").unwrap(),
        Some(before.clone())
    );
    let mut replacement = contributions()
        .await
        .into_iter()
        .find(|c| c.descriptor.id == "fixture-frame")
        .unwrap();
    replacement.descriptor.document = Some("frames/replacement.html".into());
    let replacement_host = host(&view, &import_module);
    let (registrations, failures) = replacement_host.load(&[replacement], &base()).await;
    assert!(failures.is_empty());
    home.show(&json!({"title": "Home"}), &view);
    let after = parent.query_selector("iframe").unwrap().unwrap();
    assert_ne!(before, after, "the frame kept its replaced document");
    assert_eq!(
        parent.first_element_child(),
        Some(wrapper),
        "replacing a contribution reordered its slot"
    );
    assert_eq!(parent.query_selector_all("iframe").unwrap().length(), 1);
    assert_eq!(
        frame_receiver().recv().await.unwrap()["text"],
        "replacement:Home"
    );
    home.show(&json!({"title": "Home"}), &view);
    assert_eq!(parent.query_selector("iframe").unwrap(), Some(after));
    drop(registrations);
}

/// Content whose mount always fails.
struct Unmountable;
impl DomContent for Unmountable {
    fn mount(&self, _: &DomView) -> Result<DomRoot, ToolUiError> {
        Err(ToolUiError::Presentation("cannot mount".into()))
    }
    fn update(&self, _: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        Err(ToolUiError::Presentation("cannot mount".into()))
    }
}
/// A contribution view whose content never mounts.
struct FailingView;
impl ContributionView for FailingView {
    fn content(&self, _: &Value) -> Box<dyn DomContent> {
        Box::new(Unmountable)
    }
}
/// A built-in contribution with these descriptor fields.
fn built_in(fields: Value) -> BrowserContribution {
    BrowserContribution {
        package: "built-in".into(),
        descriptor: descriptor(fields),
    }
}

/// A failing mount shows its failure in place, later contributions still mount and clearing removes every wrapper.
#[wasm_bindgen]
pub fn failing_mounts_show_their_failure_and_later_contributions_mount() {
    let view = view();
    let _failing = register_contribution(
        built_in(json!({"id": "qa-failing", "kind": "card", "target": "qa-failures"})),
        Rc::new(FailingView),
    );
    let _later = register_contribution(
        built_in(json!({"id": "qa-later", "kind": "card", "target": "qa-failures"})),
        Rc::new(BuiltIn),
    );
    let parent = container();
    let mut slot = SlotMount::new("qa-failures", parent.clone());
    let shown = slot.show(&json!({}), &view);
    assert_eq!(sentences(&shown), ["qa-failing: cannot mount"]);
    assert_eq!(parent.child_element_count(), 2);
    let alert = parent.query_selector("[role=alert]").unwrap().unwrap();
    assert_eq!(text(&alert), "qa-failing: cannot mount");
    assert!(parent.query_selector("code-block").unwrap().is_some());
    slot.clear();
    assert_eq!(parent.child_element_count(), 0);
}

/// A replacement renderer under a fresh element name replaces the mounted element.
#[wasm_bindgen]
pub async fn a_replacement_with_a_new_tag_replaces_the_element(import_module: js_sys::Function) {
    let (view, _host, _registrations, _) = loaded(&import_module).await;
    let parent: Node = container().into();
    let mut mounted = MountedContent::default();
    show_tool(&mut mounted, &parent, &view, "fixture_tool", ("one", false));
    let name = |mounted: &MountedContent| mounted.root().unwrap().node_name().to_lowercase();
    assert_eq!(name(&mounted), "qa-tool-view");
    let entry = format!("{}modules/a/module.js", base());
    let module = crate::contributions_support::settle(
        import_module.call1(&JsValue::NULL, &entry.into()).unwrap(),
    )
    .await
    .unwrap();
    call(&module, "upgrade", &js_sys::Array::new()).await;
    show_tool(&mut mounted, &parent, &view, "fixture_tool", ("two", false));
    assert_eq!(name(&mounted), "qa-tool-view-2");
    assert_eq!(parent.child_nodes().length(), 1);
    assert_eq!(text(&parent), "tool:two:false:hello A");
}

/// Two contributions declaring one child slot with different contracts make the slot's entries visible failures.
#[wasm_bindgen]
pub fn conflicting_slot_contracts_are_visible_failures() {
    let view = view();
    let _one = register_contribution(
        built_in(json!({"id": "qa-one", "kind": "page", "target": "qa-pages",
            "slots": [{"name": "qa-shared", "input": "qa.a"}]})),
        Rc::new(BuiltIn),
    );
    let _two = register_contribution(
        built_in(json!({"id": "qa-two", "kind": "page", "target": "qa-pages",
            "slots": [{"name": "qa-shared", "input": "qa.b"}]})),
        Rc::new(BuiltIn),
    );
    let _child = register_contribution(
        built_in(json!({"id": "qa-child", "kind": "tab", "target": "qa-shared", "input": "qa.a"})),
        Rc::new(BuiltIn),
    );
    let parent = container();
    let failures = SlotMount::new("qa-shared", parent.clone()).show(&json!({}), &view);
    let conflict = "qa-shared: declared by qa-one and qa-two";
    assert_eq!(sentences(&failures), [conflict]);
    let alert = parent.query_selector("[role=alert]").unwrap();
    assert_eq!(alert.map(|alert| text(&alert)).as_deref(), Some(conflict));
    assert!(parent.query_selector("code-block").unwrap().is_none());
}

/// A module message renderer whose required API is missing shows its failure instead of the element.
#[wasm_bindgen]
pub async fn message_renderers_report_a_missing_required_api(import_module: js_sys::Function) {
    let view = view();
    let host = host(&view, &import_module);
    let mut declared = contributions().await;
    for contribution in &mut declared {
        if contribution.descriptor.id == "fixture-message" {
            contribution.descriptor.requires.push("absent.api".into());
        }
    }
    let (_registrations, _) = host.load(&declared, &base()).await;
    assert_eq!(
        message_text(&view, &user_message()).as_deref(),
        Some("fixture-message: missing host API absent.api")
    );
}

/// A module registering from an update handler while it is released keeps nothing registered.
#[wasm_bindgen]
pub async fn registrations_from_teardown_callbacks_do_not_survive(import_module: js_sys::Function) {
    let view = view();
    let host = host(&view, &import_module);
    // A built-in page supplies the tab contract, so module A's tabs stay admitted while it is released.
    let _tabs_page = register_contribution(
        built_in(
            json!({"id": "qa-tabs-page", "kind": "page", "target": "qa-pages",
            "slots": [{"name": "fixture-page.tabs", "input": "fixture.tab"}]}),
        ),
        Rc::new(BuiltIn),
    );
    let module_a: Vec<BrowserContribution> = contributions()
        .await
        .into_iter()
        .filter(|c| {
            c.descriptor
                .module
                .as_ref()
                .is_some_and(|m| m.entry == "modules/a/module.js")
        })
        .collect();
    let (registrations, _) = host.load(&module_a, &base()).await;
    // Each update of this tab registers the module's tool renderer again.
    let _tabs = slot("fixture-page.tabs", &json!({"tab": "again"}));
    drop(registrations);
    let tool: Node = container().into();
    let custom = show_tool(
        &mut MountedContent::default(),
        &tool,
        &view,
        "fixture_tool",
        ("x", false),
    );
    assert!(!custom, "{}", text(&tool));
    assert!(text(&tool).starts_with("Tool Call"));
}

/// Identifiers, element tags and API names that are not strings are refused.
#[wasm_bindgen]
pub async fn non_string_identifiers_tags_and_names_are_refused(import_module: js_sys::Function) {
    let (_view, _host, _registrations, _) = loaded(&import_module).await;
    let window = web_sys::window().unwrap();
    let recorded = |name: &str| {
        js_sys::Reflect::get(&window, &name.into())
            .unwrap()
            .as_string()
    };
    assert_eq!(
        recorded("qaNonStringId").as_deref(),
        Some("register: identifier is not a string")
    );
    assert_eq!(
        recorded("qaNonStringTag").as_deref(),
        Some("register: tag is not a string")
    );
    assert_eq!(
        recorded("qaNonStringApi").as_deref(),
        Some("api: name is not a string")
    );
}

/// A descriptor naming both a module and a document mounts the module and reports the ignored document.
#[wasm_bindgen]
pub async fn a_descriptor_with_module_and_document_mounts_the_module(
    import_module: js_sys::Function,
) {
    let view = view();
    let host = host(&view, &import_module);
    let mut declared = contributions().await;
    for contribution in &mut declared {
        if contribution.descriptor.id == "fixture-card" {
            contribution.descriptor.document = Some("frames/card.html".into());
        }
    }
    let (_registrations, failures) = host.load(&declared, &base()).await;
    assert!(
        sentences(&failures)
            .contains(&"frames/card.html: ignored, the module supplies this contribution".into()),
        "{failures:?}",
        failures = sentences(&failures)
    );
    let home = slot("home", &json!({"title": "Home"}));
    assert_eq!(home.query_selector_all("qa-card-view").unwrap().length(), 1);
    assert_eq!(home.query_selector_all("iframe").unwrap().length(), 1);
}

/// A missing registration export and binding glue paired with another module's WebAssembly fail initialization.
#[wasm_bindgen]
pub async fn missing_register_export_and_mismatched_glue_fail_initialization(
    import_module: js_sys::Function,
) {
    let view = view();
    let host = host(&view, &import_module);
    let card = |register: &str, wasm: &str| BrowserContribution {
        package: PACKAGE.into(),
        descriptor: descriptor(json!({"id": "qa-module", "kind": "card", "target": "home",
            "module": {"entry": "modules/a/module.js", "wasm": wasm, "register": register}})),
    };
    // Glue initializes once per page, so the mismatch runs before any successful start.
    let (_registrations, failures) = host
        .load(&[card("register", "modules/b/module_bg.wasm")], &base())
        .await;
    let failures = sentences(&failures);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].starts_with("modules/a/module.js: ") && failures[0].contains("Error"),
        "{failures:?}"
    );
    let (_registrations, failures) = host
        .load(&[card("absent", "modules/a/module_bg.wasm")], &base())
        .await;
    assert_eq!(
        sentences(&failures),
        ["modules/a/module.js: missing export absent"]
    );
    assert!(slot_contributions("home").is_empty());
}

/// The page runs the pinned element bridge's shipped 43-line subclass shim unchanged.
#[wasm_bindgen]
pub async fn the_element_bridge_ships_its_pinned_shim(import_module: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import_module);
    let shim: Vec<String> = fetched()
        .into_iter()
        .filter(|name| name.starts_with("/snippets/custom-element-"))
        .collect();
    assert_eq!(shim.len(), 1, "{shim:?}");
    assert!(shim[0].ends_with("/src/createCustomElement.js"), "{shim:?}");
    let source =
        crate::contributions_support::fetch_text(&format!("{}{}", origin(), shim[0])).await;
    assert_eq!(source.lines().count(), 43);
    let page = page_module(&import_module).await;
    let digest = call(&page, "sha256", &js_sys::Array::of1(&source.into())).await;
    assert_eq!(
        digest.as_string().as_deref(),
        Some("3aa9226c9bc4b441dafcfe30649deef5e2f5dfe57eefe915a3ad0eaffe95f1e8")
    );
}
