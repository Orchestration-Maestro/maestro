//! Mounted page content, page-content registries and built-in elements in a real document.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{
    Clipboard, Recorded, Text, body, computed, container, host, page_module, result, rgb, text,
    until, view,
};
use crate::message_renderer_support::{NativeMessage, message};
use maestro_app::{AgentMessage, JsonObject, NoCustomMessages, ToolResultMessage};
use maestro_web::DefaultRenderer;
use maestro_web::{
    MessageRenderer,
    components::{
        button::{ButtonProps, ButtonSize, ButtonType, ButtonVariant, button},
        message_renderer_registry::dom::{
            get_message_renderer as get_page_message_renderer,
            render_message as render_page_message,
        },
    },
    dom::{DomContent, DomRoot, DomView, MountedContent},
    get_message_renderer, register_message_renderer, register_tool_renderer, render_message,
    render_tool, set_show_json_mode,
    tools::{
        dom::render_tool as render_page_tool,
        types::{
            ToolParams, ToolRenderResult, ToolRenderer, ToolRendererBinding, ToolResultMessageView,
            ToolUiError,
        },
    },
};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};
use wasm_bindgen::{JsCast as _, JsValue, prelude::wasm_bindgen};
use web_sys::{Element, HtmlElement, Node};

/// Content counting its mounts and updates.
struct Counted {
    /// Shown text.
    text: &'static str,
    /// Mount calls.
    mounts: Rc<Cell<u32>>,
    /// Update calls.
    updates: Rc<Cell<u32>>,
}
impl DomContent for Counted {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        self.mounts.set(self.mounts.get() + 1);
        Text(self.text).mount(view)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        self.updates.set(self.updates.get() + 1);
        Text(self.text).update(root, view)
    }
}
/// Content whose mount always fails.
struct Refused;
impl DomContent for Refused {
    fn mount(&self, _: &DomView) -> Result<DomRoot, ToolUiError> {
        Err(ToolUiError::Presentation("refused".into()))
    }
    fn update(&self, _: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        Err(ToolUiError::Presentation("refused".into()))
    }
}
/// A counted content and its counters.
fn counted(text: &'static str) -> (Counted, Rc<Cell<u32>>, Rc<Cell<u32>>) {
    let (mounts, updates) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let content = Counted {
        text,
        mounts: Rc::clone(&mounts),
        updates: Rc::clone(&updates),
    };
    (content, mounts, updates)
}

/// A second show of the same content type updates the first root in place.
#[wasm_bindgen]
pub fn mounted_content_keeps_one_root_while_its_content_type_stays() {
    let view = view();
    let parent: Node = container().into();
    let mut mounted = MountedContent::default();
    assert!(mounted.root().is_none());
    let (first, mounts, updates) = counted("first");
    mounted.show(&first, &parent, &view).unwrap();
    let root = mounted.root().cloned().unwrap();
    let second = Counted {
        text: "second",
        mounts: Rc::clone(&mounts),
        updates: Rc::clone(&updates),
    };
    mounted.show(&second, &parent, &view).unwrap();
    assert_eq!(mounted.root(), Some(&root));
    assert_eq!((mounts.get(), updates.get()), (1, 1));
    assert_eq!(parent.child_nodes().length(), 1);
    assert_eq!(text(&parent), "second");
}

/// A different content type mounts a new root in place of the old one.
#[wasm_bindgen]
pub fn mounted_content_replaces_the_root_when_the_content_type_changes() {
    let view = view();
    let parent: Node = container().into();
    let mut mounted = MountedContent::default();
    let (first, ..) = counted("first");
    mounted.show(&first, &parent, &view).unwrap();
    let old = mounted.root().cloned().unwrap();
    mounted.show(&Text("replacement"), &parent, &view).unwrap();
    let new = mounted.root().cloned().unwrap();
    assert_ne!(old, new);
    assert!(old.parent_node().is_none());
    assert_eq!(parent.first_child(), Some(new));
    assert_eq!(parent.child_nodes().length(), 1);
    assert_eq!(text(&parent), "replacement");
}

/// A failed replacement reports its error and keeps the previous root attached.
#[wasm_bindgen]
pub fn failed_replacement_keeps_the_previous_root() {
    let view = view();
    let parent: Node = container().into();
    let mut mounted = MountedContent::default();
    let (first, ..) = counted("kept");
    mounted.show(&first, &parent, &view).unwrap();
    let kept = mounted.root().cloned().unwrap();
    let error = mounted.show(&Refused, &parent, &view).unwrap_err();
    assert_eq!(error.to_string(), "refused");
    assert_eq!(mounted.root(), Some(&kept));
    assert_eq!(parent.first_child(), Some(kept));
    assert_eq!(text(&parent), "kept");
}

/// Callback-bearing native parameters, never serialized.
struct Native {
    /// Called by the renderer with its own name.
    notify: Rc<dyn Fn(&'static str)>,
}
/// Native details whose identity the renderer observes.
struct Details(Rc<Cell<u32>>);
/// Page renderer for the native operands.
struct PageRenderer;
impl ToolRenderer<Native, Details, Box<dyn DomContent>> for PageRenderer {
    fn render(
        &self,
        params: Option<&Native>,
        result: Option<ToolResultMessageView<'_, Details>>,
        _: bool,
    ) -> Result<ToolRenderResult<Box<dyn DomContent>>, ToolUiError> {
        if let Some(params) = params {
            (params.notify)("page");
        }
        if let Some(Details(calls)) = result.as_ref().and_then(|result| result.details()) {
            calls.set(calls.get() + 1);
        }
        Ok(ToolRenderResult {
            content: Box::new(Text("page")),
            is_custom: true,
        })
    }
}
/// Default-output renderer for ordinary JSON operands.
struct DefaultOutputRenderer;
impl ToolRenderer<JsonObject, Value> for DefaultOutputRenderer {
    fn render(
        &self,
        _: Option<&JsonObject>,
        _: Option<ToolResultMessageView<'_, Value>>,
        _: bool,
    ) -> Result<ToolRenderResult, ToolUiError> {
        Err(ToolUiError::Presentation("default output reached".into()))
    }
}

/// Root and page aliases retain the same callback and original native operands.
#[wasm_bindgen]
pub fn binding_root_and_page_aliases_keep_original_operands() {
    let renderer: Rc<dyn ToolRenderer<Native, Details>> = Rc::new(PageRenderer);
    let binding = ToolRendererBinding::from_dom(Rc::clone(&renderer));
    assert!(Rc::ptr_eq(
        &binding.renderer::<Native, Details>().unwrap(),
        &renderer
    ));
    assert!(binding.renderer::<JsonObject, Value>().is_none());
    register_tool_renderer("page-only", binding);
    let seen = Rc::new(Cell::new(""));
    let observed = Rc::clone(&seen);
    let native = Native {
        notify: Rc::new(move |name| observed.set(name)),
    };
    let calls = Rc::new(Cell::new(0));
    let message = ToolResultMessage {
        tool_call_id: "call".into(),
        tool_name: "page-only".into(),
        timestamp: 1.0,
        content: Vec::new(),
        details: Some(Details(Rc::clone(&calls))),
        is_error: false,
    };
    for dispatch in [render_tool, render_page_tool] {
        let params = ToolParams::typed(&native, |_| Ok(None));
        let shown = dispatch("page-only", Some(params), Some(&message), false).unwrap();
        assert!(shown.is_custom);
        let root = shown.content.mount(&view()).unwrap();
        assert_eq!(text(&root.node), "page");
    }
    assert_eq!((seen.get(), calls.get()), ("page", 2));
    register_tool_renderer(
        "default-output",
        ToolRendererBinding::from_raw::<JsonObject, Value>(Rc::new(DefaultOutputRenderer)),
    );
    let object = JsonObject::new();
    let result = render_page_tool::<JsonObject, Value>(
        "default-output",
        Some(ToolParams::object(&object)),
        None,
        false,
    );
    assert!(
        matches!(result, Err(ToolUiError::Presentation(text)) if text == "default output reached")
    );
}

/// Page renderer counting its calls.
struct CountingPage(Rc<Cell<u32>>);
impl ToolRenderer<JsonObject, Value, Box<dyn DomContent>> for CountingPage {
    fn render(
        &self,
        _: Option<&JsonObject>,
        _: Option<ToolResultMessageView<'_, Value>>,
        _: bool,
    ) -> Result<ToolRenderResult<Box<dyn DomContent>>, ToolUiError> {
        self.0.set(self.0.get() + 1);
        Ok(ToolRenderResult {
            content: Box::new(Text("custom")),
            is_custom: true,
        })
    }
}

/// JSON mode selects the default page content and leaves the page registration in place.
#[wasm_bindgen]
pub fn json_mode_bypasses_page_renderers_without_unregistering() {
    let calls = Rc::new(Cell::new(0));
    register_tool_renderer(
        "custom",
        ToolRendererBinding::from_dom::<JsonObject, Value>(Rc::new(CountingPage(Rc::clone(
            &calls,
        )))),
    );
    let object: JsonObject = serde_json::from_value(json!({"a": 1})).unwrap();
    let render = || {
        render_page_tool::<JsonObject, Value>(
            "custom",
            Some(ToolParams::object(&object)),
            None,
            false,
        )
        .unwrap()
    };
    set_show_json_mode(true);
    let bypassed = render();
    set_show_json_mode(false);
    assert!(!bypassed.is_custom);
    assert_eq!(calls.get(), 0);
    let view = view();
    let parent: Node = container().into();
    MountedContent::default()
        .show(bypassed.content.as_ref(), &parent, &view)
        .unwrap();
    assert!(text(&parent).contains("Tool Call"));
    assert!(render().is_custom);
    assert_eq!(calls.get(), 1);
}

/// Page message renderer showing its own name.
struct PageMessage(&'static str);
impl MessageRenderer<NoCustomMessages, Box<dyn DomContent>> for PageMessage {
    fn render(&self, _: &AgentMessage) -> Result<Box<dyn DomContent>, ToolUiError> {
        Ok(Box::new(Text(self.0)))
    }
}
/// Default-output message renderer whose error both aliases preserve.
struct DefaultOutputMessage;
impl MessageRenderer for DefaultOutputMessage {
    fn render(&self, _: &AgentMessage) -> Result<Box<dyn DomContent>, ToolUiError> {
        Err(ToolUiError::Presentation("default output reached".into()))
    }
}
/// The text a page message renderer's content shows.
fn shown(renderer: &Rc<dyn MessageRenderer<NoCustomMessages, Box<dyn DomContent>>>) -> String {
    let message: AgentMessage =
        serde_json::from_value(json!({"role": "user", "content": "hi", "timestamp": 1})).unwrap();
    let view = view();
    let parent: Node = container().into();
    MountedContent::default()
        .show(renderer.render(&message).unwrap().as_ref(), &parent, &view)
        .unwrap();
    text(&parent)
}

/// Page lookups accept open roles, replace in place and keep retained handles alive.
#[wasm_bindgen]
pub fn page_message_renderers_keep_open_roles_and_replacements() {
    assert!(get_page_message_renderer::<NoCustomMessages>("雪").is_none());
    for role in ["", "雪", "__proto__"] {
        let original: Rc<dyn MessageRenderer<NoCustomMessages, Box<dyn DomContent>>> =
            Rc::new(PageMessage("original"));
        register_message_renderer(role, original);
    }
    let retained = get_page_message_renderer::<NoCustomMessages>("雪").unwrap();
    let replacement: Rc<dyn MessageRenderer<NoCustomMessages, Box<dyn DomContent>>> =
        Rc::new(PageMessage("replacement"));
    register_message_renderer("雪", Rc::clone(&replacement));
    let current = get_page_message_renderer::<NoCustomMessages>("雪").unwrap();
    assert!(Rc::ptr_eq(&current, &replacement));
    assert_eq!(shown(&retained), "original");
    assert_eq!(shown(&current), "replacement");
    for role in ["", "__proto__"] {
        assert_eq!(
            shown(&get_page_message_renderer::<NoCustomMessages>(role).unwrap()),
            "original"
        );
    }
}

/// Root and page message lookups use one map and preserve the selected callback's error.
#[wasm_bindgen]
pub fn root_and_page_message_aliases_share_roles_and_errors() {
    let renderer: Rc<dyn MessageRenderer> = Rc::new(PageMessage("page"));
    register_message_renderer("user", Rc::clone(&renderer));
    register_message_renderer("assistant", Rc::new(DefaultOutputMessage));
    let user: AgentMessage =
        serde_json::from_value(json!({"role":"user","content":"hi","timestamp":1})).unwrap();
    for lookup in [
        get_message_renderer::<NoCustomMessages>,
        get_page_message_renderer::<NoCustomMessages>,
    ] {
        assert!(Rc::ptr_eq(&lookup("user").unwrap(), &renderer));
        assert!(lookup("assistant").is_some());
    }
    for dispatch in [render_message, render_page_message] {
        let root = dispatch(&user).unwrap().unwrap().mount(&view()).unwrap();
        assert_eq!(text(&root.node), "page");
    }
    let failed = get_page_message_renderer::<NoCustomMessages>("assistant")
        .unwrap()
        .render(&user);
    assert!(
        matches!(failed, Err(ToolUiError::Presentation(text)) if text == "default output reached")
    );
}

/// Mount the default page content for these operands and return its root.
fn default_content(
    params: Option<&Value>,
    output: Option<(&str, bool)>,
    streaming: bool,
) -> Element {
    let message = output.map(|(text, is_error)| result(text, is_error));
    let shown = render_page_tool::<Value, Value>(
        "unknown",
        params.map(ToolParams::json),
        message.as_ref(),
        streaming,
    )
    .unwrap();
    assert!(!shown.is_custom);
    let view = view();
    let parent = container();
    MountedContent::default()
        .show(shown.content.as_ref(), &parent, &view)
        .unwrap();
    parent.first_element_child().unwrap()
}
/// Section labels, shown listing text and listing languages of mounted default content.
fn listings(root: &Element) -> Vec<(String, String, String)> {
    let codes = root.query_selector_all(".maestro-code").unwrap();
    (0..codes.length())
        .map(|index| {
            let code: Element = codes.item(index).unwrap().unchecked_into();
            let label = code.previous_element_sibling().unwrap();
            (
                text(&label),
                text(&code.query_selector("pre code").unwrap().unwrap()),
                code.get_attribute("language").unwrap(),
            )
        })
        .collect()
}
/// The header caption of mounted default content.
fn caption(root: &Element) -> String {
    text(
        &root
            .query_selector(".maestro-tool-header")
            .unwrap()
            .unwrap(),
    )
}

/// The default page renderer shows formatted input, text or JSON output and its preparing states.
#[wasm_bindgen]
pub fn default_page_renderer_prepares_input_and_output(import_module: &js_sys::Function) {
    let _host = host(&view(), import_module);
    let params = json!({"path": "a.txt", "n": 1});
    let done = default_content(Some(&params), Some(("done", false)), false);
    assert_eq!(caption(&done), "Tool Call");
    let pretty = "{\n  \"path\": \"a.txt\",\n  \"n\": 1\n}";
    assert_eq!(
        listings(&done),
        [
            ("Input".into(), pretty.into(), "json".into()),
            ("Output".into(), "done".into(), "text".into())
        ]
    );
    let json_output = default_content(None, Some(("{\"ok\":true}", false)), false);
    assert_eq!(
        listings(&json_output),
        [(
            "Output".into(),
            "{\n  \"ok\": true\n}".into(),
            "json".into()
        )]
    );
    let empty = default_content(None, Some(("", false)), false);
    assert_eq!(
        listings(&empty),
        [("Output".into(), "(no output)".into(), "text".into())]
    );
    let waiting = default_content(None, None, true);
    assert_eq!(caption(&waiting), "Preparing tool...");
    assert!(listings(&waiting).is_empty());
    let empty_params = default_content(Some(&json!("{}")), None, true);
    assert_eq!(caption(&empty_params), "Preparing tool parameters...");
    let pending = default_content(Some(&params), None, false);
    assert_eq!(
        listings(&pending),
        [("Input".into(), pretty.into(), "json".into())]
    );
}

/// Each state shows its caption once, its status icon class and a single spinner while in progress.
#[wasm_bindgen]
pub fn page_header_shows_state_and_caption() {
    let params = json!({"a": 1});
    for (output, streaming, status, spinners) in [
        (None, true, "maestro-tool-icon", 1),
        (
            Some(("done", false)),
            false,
            "maestro-tool-icon maestro-tool-complete",
            0,
        ),
        (
            Some(("failed", true)),
            false,
            "maestro-tool-icon maestro-tool-error",
            0,
        ),
    ] {
        let root = default_content(Some(&params), output, streaming);
        let header = root
            .query_selector(".maestro-tool-header")
            .unwrap()
            .unwrap();
        assert_eq!(text(&header), "Tool Call");
        let lead = header
            .query_selector(".maestro-tool-header-lead")
            .unwrap()
            .unwrap();
        assert_eq!(lead.first_element_child().unwrap().class_name(), status);
        assert_eq!(lead.query_selector_all("svg").unwrap().length(), 1);
        assert_eq!(
            header
                .query_selector_all(".maestro-tool-spinner")
                .unwrap()
                .length(),
            spinners
        );
        assert_eq!(
            root.query_selector_all(".maestro-tool-header")
                .unwrap()
                .length(),
            1
        );
    }
}

/// The built-in listing keeps literal code, escapes markup and labels its language.
#[wasm_bindgen]
pub fn page_code_block_keeps_literal_code_and_language(import_module: &js_sys::Function) {
    let view = view();
    let _host = host(&view, import_module);
    let code = "<b>bold</b> && 'q'\n\tYWJjZA==";
    for (language, label) in [("rust", "rust"), ("", "plaintext")] {
        let block = view.document.create_element("code-block").unwrap();
        block.set_attribute("code", code).unwrap();
        block.set_attribute("language", language).unwrap();
        body().append_child(&block).unwrap();
        let listing = block.query_selector("pre code").unwrap().unwrap();
        assert_eq!(text(&listing), code);
        assert!(listing.query_selector("b").unwrap().is_none());
        let shown = block
            .query_selector(".maestro-code-label")
            .unwrap()
            .unwrap();
        assert_eq!(text(&shown), label);
        let copy = block.query_selector("copy-button").unwrap().unwrap();
        assert_eq!(copy.get_attribute("text").as_deref(), Some(code));
        assert_eq!(copy.get_attribute("title").as_deref(), Some("Copy code"));
    }
    let block = body().last_element_child().unwrap();
    block.set_attribute("code", "changed").unwrap();
    assert_eq!(
        text(&block.query_selector("pre code").unwrap().unwrap()),
        "changed"
    );
}

/// Every variant and size has its class; disabled and loading refuse activation.
#[wasm_bindgen]
pub fn button_classes_cover_each_variant_size_and_state() {
    let document = view().document.clone();
    let variants = [
        (ButtonVariant::Default, "default"),
        (ButtonVariant::Destructive, "destructive"),
        (ButtonVariant::Outline, "outline"),
        (ButtonVariant::Secondary, "secondary"),
        (ButtonVariant::Ghost, "ghost"),
        (ButtonVariant::Link, "link"),
    ];
    let sizes = [
        (ButtonSize::Sm, "sm"),
        (ButtonSize::Md, "md"),
        (ButtonSize::Lg, "lg"),
        (ButtonSize::Icon, "icon"),
    ];
    for (variant, variant_name) in variants {
        for (size, size_name) in sizes {
            let props = ButtonProps {
                variant,
                size,
                ..ButtonProps::default()
            };
            let label = document.create_element("span").unwrap();
            label.set_text_content(Some("Go"));
            let label: Node = label.into();
            let created = button(&document, &props, &[label]).unwrap();
            assert_eq!(
                created.class_name(),
                format!("maestro-button maestro-button-{variant_name} maestro-button-{size_name}")
            );
            assert_eq!(created.type_(), "button");
            assert!(!created.disabled());
            assert_eq!(text(&created), "Go");
        }
    }
    for (disabled, loading, refused, spinner) in [
        (false, false, false, false),
        (true, false, true, false),
        (false, true, true, true),
        (true, true, true, true),
    ] {
        let props = ButtonProps {
            disabled,
            loading,
            title: Some("Save"),
            ..ButtonProps::default()
        };
        let created = button(&document, &props, &[]).unwrap();
        assert_eq!(created.disabled(), refused);
        assert_eq!(created.title(), "Save");
        assert_eq!(
            created
                .query_selector(".maestro-button-spinner")
                .unwrap()
                .is_some(),
            spinner
        );
    }
    let plain = button(&document, &ButtonProps::default(), &[]).unwrap();
    assert_eq!(plain.title(), "");
}

/// The built-in listing decodes base64 code and shows other code literally.
#[wasm_bindgen]
pub fn page_code_block_decodes_encoded_code(import_module: &js_sys::Function) {
    use base64::Engine as _;
    let view = view();
    let _host = host(&view, import_module);
    let original = "let 雪 = \"<b>\";";
    let block = view.document.create_element("code-block").unwrap();
    let encoded = base64::engine::general_purpose::STANDARD.encode(original);
    block.set_attribute("code", &encoded).unwrap();
    body().append_child(&block).unwrap();
    let listing = block.query_selector("pre code").unwrap().unwrap();
    assert_eq!(text(&listing), original);
    let copy = block.query_selector("copy-button").unwrap().unwrap();
    assert_eq!(copy.get_attribute("text").as_deref(), Some(original));
}

/// Built-in elements follow their properties and observed attributes after connecting.
#[wasm_bindgen]
pub async fn built_in_elements_follow_property_and_attribute_changes(
    import_module: js_sys::Function,
) {
    let view = view();
    let _host = host(&view, &import_module);
    let block = view.document.create_element("code-block").unwrap();
    body().append_child(&block).unwrap();
    js_sys::Reflect::set(&block, &"code".into(), &"from property".into()).unwrap();
    js_sys::Reflect::set(&block, &"language".into(), &"rust".into()).unwrap();
    let listing = block.query_selector("pre code").unwrap().unwrap();
    assert_eq!(text(&listing), "from property");
    assert_eq!(
        text(&block.query_selector("span").unwrap().unwrap()),
        "rust"
    );
    let clipboard = crate::contributions_support::Recorded::install(
        crate::contributions_support::Clipboard::Accept,
    );
    let control = view.document.create_element("copy-button").unwrap();
    body().append_child(&control).unwrap();
    let title = |control: &Element| {
        control
            .query_selector("button")
            .unwrap()
            .unwrap()
            .unchecked_into::<HtmlElement>()
            .title()
    };
    assert_eq!(title(&control), "Copy");
    control.set_attribute("title", "Copy this").unwrap();
    assert_eq!(title(&control), "Copy this");
    js_sys::Reflect::set(&control, &"text".into(), &"property text".into()).unwrap();
    js_sys::Reflect::set(&control, &"showText".into(), &JsValue::TRUE).unwrap();
    assert!(control.has_attribute("show-text"));
    let page = crate::contributions_support::page_module(&import_module).await;
    control
        .query_selector("button")
        .unwrap()
        .unwrap()
        .unchecked_into::<HtmlElement>()
        .click();
    crate::contributions_support::until(&page, &control, || text(&control) == "Copied!").await;
    assert_eq!(clipboard.texts(), ["property text"]);
}

/// Success feedback and its restoration keep the focused native control in place.
#[wasm_bindgen]
pub async fn copy_control_keeps_focus_through_success_and_restoration(
    import_module: js_sys::Function,
) {
    let view = view();
    let _host = host(&view, &import_module);
    let _clipboard = Recorded::install(Clipboard::Accept);
    let control = view.document.create_element("copy-button").unwrap();
    control.set_attribute("text", "payload").unwrap();
    control.set_attribute("show-text", "").unwrap();
    body().append_child(&control).unwrap();
    let button: HtmlElement = control
        .query_selector("button")
        .unwrap()
        .unwrap()
        .unchecked_into();
    button.focus().unwrap();
    let focused = || {
        view.document
            .active_element()
            .is_some_and(|active| active.is_same_node(Some(button.unchecked_ref())))
    };
    assert!(focused(), "the control did not take focus");
    let page = page_module(&import_module).await;
    button.click();
    until(&page, &control, || text(&control) == "Copied!").await;
    assert!(focused(), "success feedback dropped the focused control");
    until(&page, &control, || text(&control).is_empty()).await;
    assert!(focused(), "restoration dropped the focused control");
}

/// Running animations on an element.
fn animations(element: &Element) -> u32 {
    let list: js_sys::Function = js_sys::Reflect::get(element, &"getAnimations".into())
        .unwrap()
        .unchecked_into();
    list.call0(element)
        .unwrap()
        .unchecked_into::<js_sys::Array>()
        .length()
}

/// Built-in elements paint the selected pack's roles: status colors, a spinning loader, a bounded listing and distinct buttons.
#[wasm_bindgen]
pub fn built_in_elements_paint_their_token_styles(import_module: &js_sys::Function) {
    let view = view();
    let _host = host(&view, import_module);
    let presentation = view.pack.presentation(view.mode).unwrap();
    let role = |name: &str| rgb(presentation.colors[name]);
    let params = json!({"a": 1});
    for (output, streaming, color) in [
        (None, true, role("text")),
        (Some(("done", false)), false, role("success")),
        (Some(("failed", true)), false, role("error")),
    ] {
        let root = default_content(Some(&params), output, streaming);
        let status = root
            .query_selector(".maestro-tool-header-lead > span")
            .unwrap()
            .unwrap();
        assert_eq!(computed(&status, "color"), color, "{output:?}");
        if let Some(spinner) = root.query_selector(".maestro-tool-spinner").unwrap() {
            assert!(animations(&spinner) > 0, "the loader does not spin");
        }
    }
    let block = view.document.create_element("code-block").unwrap();
    block.set_attribute("code", &"line\n".repeat(200)).unwrap();
    body().append_child(&block).unwrap();
    let scroller = block
        .query_selector("pre")
        .unwrap()
        .unwrap()
        .parent_element()
        .unwrap();
    assert!(scroller.client_height() > 0 && scroller.client_height() <= 384);
    assert!(scroller.scroll_height() > scroller.client_height());
    assert_button_variants_differ(&view.document);
}
/// Each button variant paints a background, text color and border no other variant shares.
fn assert_button_variants_differ(document: &web_sys::Document) {
    let variants = [
        ButtonVariant::Default,
        ButtonVariant::Destructive,
        ButtonVariant::Outline,
        ButtonVariant::Secondary,
        ButtonVariant::Ghost,
        ButtonVariant::Link,
    ];
    let painted: Vec<[String; 3]> = variants
        .into_iter()
        .map(|variant| {
            let props = ButtonProps {
                variant,
                ..ButtonProps::default()
            };
            let created = button(document, &props, &[]).unwrap();
            body().append_child(&created).unwrap();
            ["background-color", "color", "border-top-color"]
                .map(|property| computed(&created, property))
        })
        .collect();
    for (index, style) in painted.iter().enumerate() {
        assert!(
            !painted[index + 1..].contains(style),
            "{:?} paints like a later variant: {style:?}",
            variants[index]
        );
    }
}

/// Updating default page content keeps the mounted listings and changes them in place.
#[wasm_bindgen]
pub fn default_page_content_updates_its_listings_in_place(import_module: &js_sys::Function) {
    let view = view();
    let _host = host(&view, import_module);
    let params = json!({"path": "a.txt"});
    let parent: Node = container().into();
    let mut mounted = MountedContent::default();
    let show = |mounted: &mut MountedContent, message: Option<ToolResultMessage<Value>>| {
        let shown = render_page_tool::<Value, Value>(
            "unknown",
            Some(ToolParams::json(&params)),
            message.as_ref(),
            message.is_none(),
        )
        .unwrap();
        mounted
            .show(shown.content.as_ref(), &parent, &view)
            .unwrap();
    };
    show(&mut mounted, None);
    let root: Element = mounted.root().cloned().unwrap().unchecked_into();
    let input = root.query_selector(".maestro-code").unwrap().unwrap();
    show(&mut mounted, Some(result("done", false)));
    let blocks = root.query_selector_all(".maestro-code").unwrap();
    assert_eq!(blocks.length(), 2);
    assert_eq!(blocks.item(0), Some(input.into()));
    let output: Element = blocks.item(1).unwrap().unchecked_into();
    assert_eq!(
        text(&output.query_selector("pre code").unwrap().unwrap()),
        "done"
    );
    assert_eq!(caption(&root), "Tool Call");
}

/// The default renderer renders page content directly from its operands.
#[wasm_bindgen]
pub fn default_renderer_renders_page_content_directly(import_module: &js_sys::Function) {
    let _host = host(&view(), import_module);
    let params = json!({"path": "a.txt"});
    let rendered = ToolRenderer::<Value, Value, Box<dyn DomContent>>::render(
        &DefaultRenderer,
        Some(&params),
        None,
        false,
    )
    .unwrap();
    assert!(!rendered.is_custom);
    let view = view();
    let parent = container();
    MountedContent::default()
        .show(rendered.content.as_ref(), &parent, &view)
        .unwrap();
    let root = parent.first_element_child().unwrap();
    let pretty = "{\n  \"path\": \"a.txt\"\n}";
    assert_eq!(
        listings(&root),
        [("Input".into(), pretty.into(), "json".into())]
    );
}

/// Page message renderer observing the original native handle of a custom message.
struct HandlePage(Rc<Cell<u32>>, Arc<AtomicU32>);
impl MessageRenderer<NativeMessage, Box<dyn DomContent>> for HandlePage {
    fn render(
        &self,
        message: &AgentMessage<NativeMessage>,
    ) -> Result<Box<dyn DomContent>, ToolUiError> {
        let AgentMessage::Custom(value) = message else {
            unreachable!()
        };
        let value = value.read().unwrap();
        assert!(Arc::ptr_eq(&value.handle, &self.1));
        self.0.set(value.handle.load(Ordering::Relaxed));
        Ok(Box::new(Text("custom")))
    }
}

/// Custom messages with live native handles reach page renderers unserialized.
#[wasm_bindgen]
pub fn custom_messages_keep_callbacks_through_the_page_registry() {
    let handle = Arc::new(AtomicU32::new(5));
    let input = message("qa-native", Arc::clone(&handle));
    let seen = Rc::new(Cell::new(0));
    let renderer: Rc<dyn MessageRenderer<NativeMessage, Box<dyn DomContent>>> =
        Rc::new(HandlePage(Rc::clone(&seen), Arc::clone(&handle)));
    register_message_renderer("qa-native", renderer);
    handle.store(42, Ordering::Relaxed);
    let content = render_page_message(&input).unwrap().unwrap();
    assert_eq!(seen.get(), 42);
    let view = view();
    let parent: Node = container().into();
    MountedContent::default()
        .show(content.as_ref(), &parent, &view)
        .unwrap();
    assert_eq!(text(&parent), "custom");
}

/// Caller classes follow the variant and size classes, and the form type is the caller's.
#[wasm_bindgen]
pub fn button_props_add_caller_classes_and_form_type() {
    let document = view().document.clone();
    for (r#type, name) in [
        (ButtonType::Button, "button"),
        (ButtonType::Submit, "submit"),
        (ButtonType::Reset, "reset"),
    ] {
        let props = ButtonProps {
            r#type,
            class_name: "qa-extra",
            ..ButtonProps::default()
        };
        let created = button(&document, &props, &[]).unwrap();
        assert_eq!(created.type_(), name);
        assert_eq!(
            created.class_name(),
            "maestro-button maestro-button-default maestro-button-md qa-extra"
        );
    }
}
