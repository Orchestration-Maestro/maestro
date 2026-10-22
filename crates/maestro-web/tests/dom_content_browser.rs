//! Mounted page content, page-content registries and built-in elements in a real document.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{Text, body, container, host, result, text, view};
use maestro_app::{AgentMessage, JsonObject, NoCustomMessages, ToolResultMessage};
use maestro_web::{
    MessageRenderer,
    components::{
        button::{ButtonProps, ButtonSize, ButtonVariant, button},
        message_renderer_registry::get_message_renderer_dom,
    },
    dom::{DomContent, DomView, MountedContent},
    get_message_renderer, register_message_renderer, register_tool_renderer, render_message,
    render_message_dom, render_tool, set_show_json_mode,
    tools::{
        dom::render_tool as render_page_tool,
        types::{
            ToolContent, ToolParams, ToolRenderResult, ToolRenderer, ToolRendererBinding,
            ToolResultMessageView, ToolUiError,
        },
    },
};
use serde_json::{Value, json};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::{Element, Node};

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
    fn mount(&self, view: &DomView) -> Result<Node, ToolUiError> {
        self.mounts.set(self.mounts.get() + 1);
        Text(self.text).mount(view)
    }
    fn update(&self, root: &Node, view: &DomView) -> Result<(), ToolUiError> {
        self.updates.set(self.updates.get() + 1);
        Text(self.text).update(root, view)
    }
}
/// Content whose mount always fails.
struct Refused;
impl DomContent for Refused {
    fn mount(&self, _: &DomView) -> Result<Node, ToolUiError> {
        Err(ToolUiError::Presentation("refused".into()))
    }
    fn update(&self, _: &Node, _: &DomView) -> Result<(), ToolUiError> {
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
/// Canvas renderer for ordinary JSON operands.
struct CanvasRenderer;
impl ToolRenderer<JsonObject, Value> for CanvasRenderer {
    fn render(
        &self,
        _: Option<&JsonObject>,
        _: Option<ToolResultMessageView<'_, Value>>,
        _: bool,
    ) -> Result<ToolRenderResult, ToolUiError> {
        Err(ToolUiError::Presentation("canvas reached".into()))
    }
}

/// Page and canvas handles answer only their own lookups; page operands keep their identity.
#[wasm_bindgen]
pub fn binding_keeps_canvas_and_page_handles_apart() {
    let page = ToolRendererBinding::from_dom::<Native, Details>(Rc::new(PageRenderer));
    assert!(page.renderer::<Native, Details>().is_none());
    assert!(page.renderer::<JsonObject, Value>().is_none());
    register_tool_renderer("page-only", page);
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
    let params = ToolParams::typed(&native, |_| Ok(None));
    let shown = render_page_tool("page-only", Some(params), Some(&message), false).unwrap();
    assert!(shown.is_custom);
    assert_eq!((seen.get(), calls.get()), ("page", 1));
    let params = ToolParams::typed(&native, |_| Ok(None));
    let canvas = render_tool("page-only", Some(params), Some(&message), false);
    assert!(
        matches!(canvas, Err(ToolUiError::Presentation(text)) if text == "Renderer operand types do not match")
    );
    register_tool_renderer(
        "canvas-only",
        ToolRendererBinding::from_raw::<JsonObject, Value>(Rc::new(CanvasRenderer)),
    );
    let object = JsonObject::new();
    let page = render_page_tool::<JsonObject, Value>(
        "canvas-only",
        Some(ToolParams::object(&object)),
        None,
        false,
    );
    assert!(
        matches!(page, Err(ToolUiError::Presentation(text)) if text == "Renderer operand types do not match")
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
/// Canvas message renderer that is never reached by page lookups.
struct CanvasMessage;
impl MessageRenderer for CanvasMessage {
    fn render(&self, _: &AgentMessage) -> Result<Box<dyn ToolContent>, ToolUiError> {
        Err(ToolUiError::Presentation("canvas reached".into()))
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
    assert!(get_message_renderer_dom::<NoCustomMessages>("雪").is_none());
    for role in ["", "雪", "__proto__"] {
        let original: Rc<dyn MessageRenderer<NoCustomMessages, Box<dyn DomContent>>> =
            Rc::new(PageMessage("original"));
        register_message_renderer(role, original);
    }
    let retained = get_message_renderer_dom::<NoCustomMessages>("雪").unwrap();
    let replacement: Rc<dyn MessageRenderer<NoCustomMessages, Box<dyn DomContent>>> =
        Rc::new(PageMessage("replacement"));
    register_message_renderer("雪", Rc::clone(&replacement));
    let current = get_message_renderer_dom::<NoCustomMessages>("雪").unwrap();
    assert!(Rc::ptr_eq(&current, &replacement));
    assert_eq!(shown(&retained), "original");
    assert_eq!(shown(&current), "replacement");
    for role in ["", "__proto__"] {
        assert_eq!(
            shown(&get_message_renderer_dom::<NoCustomMessages>(role).unwrap()),
            "original"
        );
    }
}

/// A page message renderer is invisible to canvas lookups and the other way round.
#[wasm_bindgen]
pub fn canvas_and_page_message_lookups_stay_separate() {
    let page: Rc<dyn MessageRenderer<NoCustomMessages, Box<dyn DomContent>>> =
        Rc::new(PageMessage("page"));
    register_message_renderer("user", page);
    let canvas: Rc<dyn MessageRenderer> = Rc::new(CanvasMessage);
    register_message_renderer("assistant", canvas);
    let user: AgentMessage =
        serde_json::from_value(json!({"role": "user", "content": "hi", "timestamp": 1})).unwrap();
    assert!(get_message_renderer::<NoCustomMessages>("user").is_none());
    assert!(render_message(&user).unwrap().is_none());
    let content = render_message_dom(&user).unwrap().unwrap();
    let view = view();
    let parent: Node = container().into();
    MountedContent::default()
        .show(content.as_ref(), &parent, &view)
        .unwrap();
    assert_eq!(text(&parent), "page");
    assert!(get_message_renderer_dom::<NoCustomMessages>("assistant").is_none());
    assert!(get_message_renderer::<NoCustomMessages>("assistant").is_some());
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
/// Section labels and listing attributes of mounted default content.
fn listings(root: &Element) -> Vec<(String, String, String)> {
    let codes = root.query_selector_all("code-block").unwrap();
    (0..codes.length())
        .map(|index| {
            let code: Element = codes.item(index).unwrap().unchecked_into();
            let label = code.previous_element_sibling().unwrap();
            (
                text(&label),
                code.get_attribute("code").unwrap(),
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
pub fn default_page_renderer_prepares_input_and_output() {
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
            .query_selector(".maestro-code-block-language")
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
