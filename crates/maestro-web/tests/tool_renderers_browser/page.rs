//! Default tool content and nested native placement lifetimes.
use maestro_web::{
    dom::{DomContent, DomRoot, DomView, MountedContent},
    render_tool,
    tools::types::ToolParams,
};
use serde_json::Value;
use wasm_bindgen::prelude::wasm_bindgen;

/// Every recorded default branch keeps its literal listings, labels and layout; labels follow
/// localization and literal output is shown and copied as authored.
#[wasm_bindgen]
pub async fn default_tool_renderer_preserves_all_recorded_branches(
    import_module: &js_sys::Function,
) {
    use crate::contributions_support::{container, select, text, view};
    let view = view();
    let _elements = crate::contributions_support::host(&view, import_module);
    let page = crate::contributions_support::page_module(import_module).await;
    localized_and_literal(&view, &page).await;
    let parent = container();
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/tool_renderers.json")).unwrap();
    assert_eq!(fixture["rows"].as_array().unwrap().len(), 95);
    for row in fixture["rows"].as_array().unwrap() {
        let input = &row["input"];
        let message = input
            .get("result")
            .map(|r| serde_json::from_value::<maestro_app::ToolResultMessage>(r.clone()).unwrap());
        let recipe = render_tool::<Value, Value>(
            "unregistered",
            input.get("params").map(ToolParams::json),
            message.as_ref(),
            input["isStreaming"].as_bool().unwrap(),
        )
        .unwrap();
        assert!(!recipe.is_custom);
        let mut slot = MountedContent::default();
        slot.show(recipe.content.as_ref(), &parent, &view).unwrap();
        let root = slot.root().unwrap();
        assert_eq!(
            text(&select(root, ".maestro-tool-caption")),
            row["expected"]["header"]
        );
        let labels = select_all(root, ".maestro-tool-section-label");
        let actual: Vec<_> = labels.iter().map(|label| text(label)).collect();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            row["expected"]["labels"]
        );
        let blocks: Vec<_> = select_all(root, ".maestro-code").iter().map(|block| {
            serde_json::json!({"code": text(&select(block, "pre code")), "language": block.get_attribute("language").unwrap()})
        }).collect();
        assert_eq!(
            serde_json::to_value(blocks).unwrap(),
            row["expected"]["blocks"]
        );
        assert!(parent.query_selector("script").unwrap().is_none());
        default_layout(root);
        slot.clear();
        assert!(parent.first_child().is_none());
    }
    parent.remove();
}

/// Measured label spacing and text metrics match the default card's native layout.
fn default_layout(root: &web_sys::Node) {
    use crate::contributions_support::{computed, select};
    for label in select_all(root, ".maestro-tool-section-label") {
        let code = label.next_element_sibling().unwrap();
        let gap = code.get_bounding_client_rect().top() - label.get_bounding_client_rect().bottom();
        assert!((gap - 4.).abs() < 0.25, "label/listing gap: {gap}");
        assert_eq!(computed(&label, "font-size"), "12px");
        assert_eq!(computed(&label, "line-height"), "16px");
        assert_eq!(computed(&label, "font-weight"), "500");
    }
    assert_eq!(
        computed(&select(root, ".maestro-tool-caption"), "line-height"),
        "20px"
    );
}

/// Descendants in native document order.
fn select_all(root: &web_sys::Node, selector: &str) -> Vec<web_sys::Element> {
    use wasm_bindgen::JsCast as _;
    let nodes = root
        .unchecked_ref::<web_sys::Element>()
        .query_selector_all(selector)
        .unwrap();
    (0..nodes.length())
        .map(|index| nodes.item(index).unwrap().unchecked_into())
        .collect()
}

/// A different native content type selected after a tool recipe.
struct Replacement;
impl DomContent for Replacement {
    fn mount(&self, view: &DomView) -> Result<DomRoot, maestro_web::tools::types::ToolUiError> {
        let node = view.document.create_element("span").unwrap();
        node.set_text_content(Some("replacement"));
        Ok(DomRoot::new(node))
    }
    fn update(
        &self,
        _: &DomRoot,
        _: &DomView,
    ) -> Result<(), maestro_web::tools::types::ToolUiError> {
        Ok(())
    }
}

/// Replacing a placed recipe detaches its old root even if the caller retains the node; default
/// subsections, command consoles and calculation errors keep their roots only while shown.
#[wasm_bindgen]
pub fn tool_content_preserves_native_roots_until_row_or_recipe_changes(
    import_module: &js_sys::Function,
) {
    use crate::contributions_support::{container, select, view};
    let view = view();
    let _elements = crate::contributions_support::host(&view, import_module);
    let parent = container();
    let input = serde_json::json!({"value":42});
    let recipe =
        render_tool::<Value, Value>("unregistered", Some(ToolParams::json(&input)), None, false)
            .unwrap();
    let mut slot = MountedContent::default();
    slot.show(recipe.content.as_ref(), &parent, &view).unwrap();
    let root = slot.root().unwrap().clone();
    let listing = select(&root, ".maestro-code");
    slot.show(recipe.content.as_ref(), &parent, &view).unwrap();
    assert!(select(slot.root().unwrap(), ".maestro-code").is_same_node(Some(&listing)));
    drop(recipe);
    slot.show(&Replacement, &parent, &view).unwrap();
    assert!(!root.is_connected());
    assert!(!listing.is_connected());
    slot.clear();
    default_subsections(&view, &parent);
    command_lifetimes(&view, &parent);
    parent.remove();
}

/// A captured child placement lives with the root that contains it, not its recipe.
struct Nested(Box<dyn DomContent>);
impl DomContent for Nested {
    fn mount(&self, view: &DomView) -> Result<DomRoot, maestro_web::tools::types::ToolUiError> {
        let root = view.document.create_element("div").unwrap();
        let mut child = MountedContent::default();
        child.show(self.0.as_ref(), &root, view)?;
        Ok(DomRoot {
            node: root.into(),
            owner: Some(Box::new(child)),
        })
    }
    fn update(
        &self,
        _: &DomRoot,
        _: &DomView,
    ) -> Result<(), maestro_web::tools::types::ToolUiError> {
        Ok(())
    }
}

/// Detaching an outer root detaches its independently owned nested native content.
#[wasm_bindgen]
pub fn retained_nested_tool_recipe_detaches_with_outer_row(import_module: &js_sys::Function) {
    use crate::contributions_support::{container, select, view};
    let view = view();
    let _elements = crate::contributions_support::host(&view, import_module);
    let parent = container();
    let input = serde_json::json!({"nested":42});
    let recipe = Nested(
        render_tool::<Value, Value>("unregistered", Some(ToolParams::json(&input)), None, false)
            .unwrap()
            .content,
    );
    let mut slot = MountedContent::default();
    slot.show(&recipe, &parent, &view).unwrap();
    let listing = select(slot.root().unwrap(), ".maestro-code");
    slot.show(&Replacement, &parent, &view).unwrap();
    assert!(!listing.is_connected());
    slot.show(&recipe, &parent, &view).unwrap();
    let current = select(slot.root().unwrap(), ".maestro-code");
    assert!(current.is_connected());
    assert!(!current.is_same_node(Some(&listing)));
    slot.clear();
    assert!(!current.is_connected());
    parent.remove();
}

/// Default labels, empty output and the preparing caption follow localization while the
/// caption stays literal; a literal output listing shows and copies its original text.
async fn localized_and_literal(view: &DomView, page: &wasm_bindgen::JsValue) {
    use crate::contributions_support::{Clipboard, Recorded, localized_view, select, text, until};
    for (code, input, output, empty) in [
        ("en", "Input", "Output", "(no output)"),
        ("de", "Eingabe", "Ausgabe", "(keine Ausgabe)"),
    ] {
        let view = localized_view(view, code);
        let params = serde_json::json!({"value": 42});
        let mut result = crate::recipes::message::<Value>(None);
        result.content.clear();
        let shown = render_tool(
            "unknown",
            Some(ToolParams::json(&params)),
            Some(&result),
            false,
        )
        .unwrap();
        let root = shown.content.mount(&view).unwrap();
        let labels: Vec<_> = select_all(&root.node, ".maestro-tool-section-label")
            .iter()
            .map(|label| text(label))
            .collect();
        assert_eq!(labels, [input, output]);
        let listings = select_all(&root.node, ".maestro-code pre code");
        assert_eq!(text(&listings[1]), empty);
        assert_eq!(
            text(&select(&root.node, ".maestro-tool-caption")),
            "Tool Call"
        );
        let preparing = render_tool::<Value, Value>("unknown", None, None, false).unwrap();
        let root = preparing.content.mount(&view).unwrap();
        let caption = text(&select(&root.node, ".maestro-tool-caption"));
        assert_eq!(caption, "Preparing tool...");
    }
    let mut result = crate::recipes::message::<Value>(None);
    result.content.truncate(1);
    let maestro_app::UserBlock::Text(output) = &mut result.content[0] else {
        unreachable!()
    };
    output.text = "YWJj".into();
    let shown = render_tool::<Value, Value>("literal", None, Some(&result), false).unwrap();
    let parent = crate::contributions_support::container();
    let root = shown.content.mount(view).unwrap();
    parent.append_child(&root.node).unwrap();
    assert_eq!(text(&select(&root.node, ".maestro-code pre code")), "YWJj");
    let clipboard = Recorded::install(Clipboard::Accept);
    let copy = select(&root.node, ".maestro-code button");
    wasm_bindgen::JsCast::unchecked_ref::<web_sys::HtmlElement>(&copy).click();
    until(page, &copy, || text(&copy) == "Copied!").await;
    assert_eq!(clipboard.texts(), ["YWJj"]);
    parent.remove();
}

/// Default input and output listings keep their roots while shown and release them when they
/// disappear; forced JSON and a registered custom route then replace them.
fn default_subsections(view: &DomView, parent: &web_sys::Element) {
    let mut slot = MountedContent::default();
    let input = serde_json::json!({"value": 42});
    let result = crate::recipes::message::<Value>(None);
    let (mut kept_input, mut kept_output) = (None::<web_sys::Element>, None::<web_sys::Element>);
    for (params, output, count) in [
        (None, None, 0),
        (Some(&input), None, 1),
        (Some(&input), Some(&result), 2),
        (None, Some(&result), 1),
        (None, None, 0),
    ] {
        let shown =
            render_tool("unregistered", params.map(ToolParams::json), output, false).unwrap();
        slot.show(shown.content.as_ref(), parent, view).unwrap();
        let listings = select_all(slot.root().unwrap(), ".maestro-code");
        assert_eq!(listings.len(), count);
        let same = |kept: &Option<web_sys::Element>, index: usize| {
            kept.as_ref().unwrap().is_same_node(Some(&listings[index]))
        };
        match (count, params.is_some(), output.is_some()) {
            (1, true, false) => kept_input = Some(listings[0].clone()),
            (2, ..) => {
                assert!(same(&kept_input, 0), "the shown input keeps its root");
                kept_output = Some(listings[1].clone());
            }
            (1, false, true) => {
                assert!(!kept_input.as_ref().unwrap().is_connected());
                assert!(same(&kept_output, 0), "the shown output keeps its root");
            }
            (0, ..) => {
                assert!(kept_output.as_ref().is_none_or(|kept| !kept.is_connected()));
            }
            _ => unreachable!(),
        }
    }
    forced_and_custom(view, parent, &mut slot, &input);
    slot.clear();
}

/// Forced JSON shows the arguments of a registered route; without it the custom content shows
/// no default parts.
fn forced_and_custom(
    view: &DomView,
    parent: &web_sys::Element,
    slot: &mut MountedContent,
    input: &Value,
) {
    crate::recipes::reset();
    maestro_web::register_tool_renderer(
        "route",
        maestro_web::tools::types::ToolRendererBinding::from_raw(std::rc::Rc::new(
            crate::recipes::Renderer(std::rc::Rc::new(
                |_: Option<&Value>,
                 _: Option<maestro_web::tools::types::ToolResultMessageView<'_>>,
                 _| Ok(crate::recipes::content("custom", true)),
            )),
        )),
    );
    maestro_web::set_show_json_mode(true);
    let forced =
        render_tool::<Value, Value>("route", Some(ToolParams::json(input)), None, false).unwrap();
    slot.show(forced.content.as_ref(), parent, view).unwrap();
    assert_eq!(select_all(slot.root().unwrap(), ".maestro-code").len(), 1);
    maestro_web::set_show_json_mode(false);
    let custom = render_tool::<Value, Value>("route", None, None, false).unwrap();
    slot.show(custom.content.as_ref(), parent, view).unwrap();
    let selector = ".maestro-code, .maestro-tool-caption, .maestro-tool-section-label";
    assert!(parent.query_selector(selector).unwrap().is_none());
    crate::recipes::reset();
}

/// A command console and caption keep their elements while the command changes, the console
/// goes with the command, and a calculation error keeps its element until it succeeds.
fn command_lifetimes(view: &DomView, parent: &web_sys::Element) {
    use crate::contributions_support::{select, text};
    use maestro_web::{BashRenderer, CalculateRenderer, tools::types::ToolRenderer};
    let mut slot = MountedContent::default();
    let mut show = |shown: maestro_web::ToolRenderResult| {
        slot.show(shown.content.as_ref(), parent, view).unwrap();
        slot.root().unwrap().clone()
    };
    let command = |value: &str| serde_json::json!({"command": value});
    let bash = |args: Option<&Value>| {
        <BashRenderer as ToolRenderer<maestro_app::JsonObject, ()>>::render(
            &BashRenderer,
            args.and_then(Value::as_object),
            None,
            false,
        )
        .unwrap()
    };
    let root = show(bash(Some(&command("first"))));
    let console = select(&root, ".maestro-console");
    let caption = select(&root, ".maestro-tool-caption");
    assert_eq!(text(&select(&console, "pre")), "> first");
    let root = show(bash(Some(&command("second"))));
    assert!(select(&root, ".maestro-console").is_same_node(Some(&console)));
    assert_eq!(text(&select(&console, "pre")), "> second");
    let root = show(bash(None));
    assert!(!console.is_connected());
    assert!(select(&root, ".maestro-tool-caption").is_same_node(Some(&caption)));
    let mut result = crate::recipes::message::<()>(None);
    let calculate = |expression: &str, result: &maestro_app::ToolResultMessage<()>| {
        let args = serde_json::json!({"expression": expression});
        CalculateRenderer
            .render(args.as_object(), Some(result.into()), false)
            .unwrap()
    };
    let root = show(calculate("literal expression", &result));
    let error = select(&root, "div.maestro-tool-error");
    let caption = select(&root, ".maestro-tool-caption");
    assert_eq!(text(&caption), "literal expression");
    assert_eq!(text(&error), "one\ntwo");
    let root = show(calculate("changed expression", &result));
    assert!(select(&root, "div.maestro-tool-error").is_same_node(Some(&error)));
    assert!(select(&root, ".maestro-tool-caption").is_same_node(Some(&caption)));
    assert_eq!(text(&caption), "changed expression");
    result.is_error = false;
    let root = show(calculate("changed expression", &result));
    assert!(!error.is_connected());
    let caption = select(&root, ".maestro-tool-caption");
    assert_eq!(text(&caption), "changed expression = one\ntwo");
    let root = show(calculate("changed expression", &result));
    assert!(select(&root, ".maestro-tool-caption").is_same_node(Some(&caption)));
    slot.clear();
}
