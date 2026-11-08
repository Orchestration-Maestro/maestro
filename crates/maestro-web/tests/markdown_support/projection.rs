//! Page-side projection of a shown Markdown document and recorded reference markup.
use crate::browser_fixture_support::{get, invoke, object, set};
use serde_json::{Map, Value, json};
use std::cell::OnceCell;
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::{Document, Element, Node};

thread_local! {
    /// The recorded formulas the structural comparison resolves, parsed once per page.
    static FORMULAS: OnceCell<Vec<Value>> = const { OnceCell::new() };
}

/// Element names whose neighbouring newline-only text is layout rather than content.
const BLOCKS: [&str; 19] = [
    "P",
    "H1",
    "H2",
    "H3",
    "H4",
    "H5",
    "H6",
    "UL",
    "OL",
    "LI",
    "BLOCKQUOTE",
    "TABLE",
    "THEAD",
    "TBODY",
    "TR",
    "TH",
    "TD",
    "HR",
    "DIV",
];

/// Container class names and the painted role each one resolves to.
const ROLES: [(&str, &str); 7] = [
    ("my-4", "math-block"),
    ("maestro-math-block", "math-block"),
    (
        "overflow-x-auto my-2 border border-border rounded",
        "scroll",
    ),
    ("maestro-table-scroll", "scroll"),
    ("my-4 text-red-500 font-mono", "math-fallback"),
    ("maestro-math-fallback", "math-fallback"),
    ("text-red-500 font-mono", "math-fallback"),
];

/// Class names the matching math renderer spells differently from the recorded reference.
const RENAMES: [(&str, &str); 3] = [
    ("base", "katex-base"),
    ("strut", "katex-strut"),
    ("sizing", "katex-sizing"),
];

/// One side of a structural comparison.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    /// The shown document.
    Native,
    /// The recorded reference markup.
    Reference,
}

impl Side {
    /// Whether this side is reference markup.
    fn is_reference(self) -> bool {
        self == Self::Reference
    }
}

/// Class names whose first name marks a formula's markup.
const MATH_CLASSES: [&str; 3] = ["katex", "katex-error", "katex-display"];

/// Painted roles the selected pack resolves, with the selector and property reading each one.
const PAINTED: [(&str, &str, &str, &str); 8] = [
    ("link", "a", "color", "accent"),
    ("code", "code:not(.hljs)", "background-color", "panel"),
    ("quote", "blockquote", "border-left-color", "border"),
    ("header", "th", "background-color", "panel"),
    ("error", ".katex-error", "color", "error"),
    ("label", ".maestro-code-label", "color", "muted"),
    ("listing", ".maestro-code", "border-top-color", "border"),
    (
        "scroller",
        ".maestro-table-scroll",
        "border-top-color",
        "border",
    ),
];

/// Publish the page calls the authored checks read.
///
/// # Panics
/// Panics if the page refuses a reflection read or write.
pub fn install() {
    let root: JsValue =
        Closure::<dyn Fn() -> JsValue>::new(|| root().map_or(JsValue::NULL, JsValue::from))
            .into_js_value();
    let diff: JsValue =
        Closure::<dyn Fn(JsValue, JsValue) -> JsValue>::new(|root: JsValue, html: JsValue| {
            match root.dyn_into::<Element>() {
                Ok(root) => {
                    JsValue::from_str(&difference(&root, &html.as_string().unwrap_or_default()))
                }
                Err(_) => JsValue::from_str("no document is shown"),
            }
        })
        .into_js_value();
    let failures: JsValue =
        Closure::<dyn Fn(JsValue, JsValue) -> JsValue>::new(|colors: JsValue, mode: JsValue| {
            let thinking = mode.as_bool().unwrap_or(false);
            let array = js_sys::Array::new();
            for message in paint_failures(&colors, thinking) {
                array.push(&JsValue::from_str(&message));
            }
            array.into()
        })
        .into_js_value();
    let maestro = object(&[
        ("root", root.as_ref()),
        ("diff", diff.as_ref()),
        ("paintFailures", failures.as_ref()),
    ]);
    set(&js_sys::global(), "maestro", maestro.as_ref());
    FORMULAS.with(|cell| {
        cell.get_or_init(|| {
            serde_json::from_str(include_str!("../fixtures/markdown/formulas.json")).unwrap()
        });
    });
}

/// The page's document.
fn document() -> Document {
    web_sys::window().unwrap().document().unwrap()
}

/// The shown document root, or none when no document is shown.
///
/// # Panics
/// Panics if the page has no document.
#[must_use]
pub fn root() -> Option<Element> {
    document().query_selector(".maestro-markdown").unwrap()
}

/// First difference between the native children of `root` and reference markup, or "".
///
/// # Panics
/// Panics if the page's parser refuses the reference markup.
#[must_use]
pub fn difference(root: &Element, html: &str) -> String {
    let actual = container(children(&nodes(&root.child_nodes()), Side::Native));
    let expected = container(children(&fragment(html), Side::Reference));
    first_difference(&actual, &expected, "")
}

/// First difference between the shown structure and a recorded tree, or "".
///
/// # Panics
/// Panics if no document is shown.
#[must_use]
pub fn tree_difference(recorded: &Value) -> String {
    let root = root().expect("a Markdown document is shown");
    let actual = semantic_tree(&nodes(&root.child_nodes()));
    let expected = recorded.as_array().expect("the recorded tree is an array");
    if actual.len() != expected.len() {
        return format!(
            "top level: {} != {}: {}",
            actual.len(),
            expected.len(),
            truncated(&serialized(&Value::Array(actual)))
        );
    }
    for (index, node) in expected.iter().enumerate() {
        let found = tree_node_difference(&actual[index], node, &format!("[{index}]"));
        if !found.is_empty() {
            return found;
        }
    }
    String::new()
}

/// Compare a mounted formula at the recorder's typography and containing width.
///
/// # Panics
/// Panics if no document is shown or its fonts never load.
pub async fn measure_math(expected: &Value, branch: &str, source: &str) -> String {
    let container = root().expect("a Markdown document is shown");
    let style = format!(
        "{};width:800px;font-size:16px;font-family:\"Times New Roman\";line-height:normal",
        container.get_attribute("style").unwrap_or_default()
    );
    container.set_attribute("style", &style).unwrap();
    wait_for_fonts().await;
    if container
        .query_selector("img,script,iframe")
        .unwrap()
        .is_some()
    {
        return "executable element in formula".to_owned();
    }
    if branch == "throw" {
        let fallback = container
            .query_selector(".maestro-math-fallback")
            .unwrap()
            .and_then(|fallback| fallback.text_content())
            .unwrap_or_default();
        if fallback != source {
            return "delimiter fallback text".to_owned();
        }
        return difference(&container, expected.as_str().unwrap_or_default());
    }
    let Some(formula) = container.query_selector(".katex,.katex-error").unwrap() else {
        return "missing formula".to_owned();
    };
    let error = formula.class_list().contains("katex-error");
    if error != (branch == "error") {
        return "wrong error branch".to_owned();
    }
    if error {
        let title = formula.get_attribute("title").unwrap_or_default();
        let style = get(formula.as_ref(), "style");
        let color = get(&style, "color").as_string().unwrap_or_default();
        if title.is_empty() || color != "var(--maestro-color-error)" {
            return "error tooltip or selected role".to_owned();
        }
    }
    compared(&measured(&formula), expected, "formula")
}

/// Wait for the page's formula fonts, as the recorder did.
async fn wait_for_fonts() {
    let fonts = get(document().as_ref(), "fonts");
    for family in ["KaTeX_Main", "KaTeX_Math", "KaTeX_Size1", "KaTeX_Size2"] {
        let loaded: js_sys::Promise = invoke(
            &fonts,
            "load",
            &[&JsValue::from_str(&format!("16px {family}"))],
        )
        .unwrap()
        .unchecked_into();
        wasm_bindgen_futures::JsFuture::from(loaded).await.unwrap();
    }
    ready(&fonts).await;
}

/// Wait for the page's fonts to finish loading.
async fn ready(fonts: &JsValue) {
    let ready: js_sys::Promise = get(fonts, "ready").unchecked_into();
    wasm_bindgen_futures::JsFuture::from(ready).await.unwrap();
}

/// The measured text, bounds, glyph rectangles and SVG rectangles of one formula.
fn measured(formula: &Element) -> Value {
    let origin = rect(formula.get_bounding_client_rect().as_ref());
    let mut glyphs = Vec::new();
    for text in text_nodes(formula) {
        if text.text_content().unwrap_or_default() == "\u{200b}" {
            continue;
        }
        let range = invoke(&document(), "createRange", &[]).unwrap();
        invoke(&range, "selectNodeContents", &[text.as_ref()]).unwrap();
        let found = rect(&invoke(&range, "getBoundingClientRect", &[]).unwrap());
        if number(&found, "width") == 0. && number(&found, "height") == 0. {
            continue;
        }
        let mut glyph = Map::new();
        glyph.insert(
            "text".to_owned(),
            Value::String(text.text_content().unwrap_or_default()),
        );
        glyph.extend(relative_boxes(&found, &origin));
        glyphs.push(Value::Object(glyph));
    }
    let svg = elements(&formula.query_selector_all("svg").unwrap())
        .iter()
        .map(|svg| {
            let found = rect(svg.get_bounding_client_rect().as_ref());
            Value::Object(relative_boxes(&found, &origin))
        })
        .collect::<Vec<_>>();
    let mut actual = Map::new();
    actual.insert(
        "text".to_owned(),
        Value::String(formula.text_content().unwrap_or_default()),
    );
    actual.insert("width".to_owned(), json!(number(&origin, "width")));
    actual.insert("height".to_owned(), json!(number(&origin, "height")));
    actual.insert("glyphs".to_owned(), Value::Array(glyphs));
    actual.insert("svg".to_owned(), Value::Array(svg));
    Value::Object(actual)
}

/// Every text node inside `root`, in document order, skipping text inside inline SVG.
fn text_nodes(root: &Element) -> Vec<Node> {
    let mut found = Vec::new();
    let mut pending: Vec<Node> = vec![root.clone().into()];
    while let Some(node) = pending.pop() {
        for child in nodes(&node.child_nodes()).into_iter().rev() {
            pending.push(child);
        }
        let inside_svg = node
            .parent_element()
            .is_some_and(|parent| parent.closest("svg").unwrap().is_some());
        if node.node_type() == 3 && !inside_svg {
            found.push(node);
        }
    }
    found
}

/// The position and size of a page rectangle, in page coordinates.
fn rect(found: &JsValue) -> Map<String, Value> {
    let mut rect = Map::new();
    for key in ["left", "top", "width", "height"] {
        let source = if key == "left" { "x" } else { key };
        rect.insert(
            key.to_owned(),
            json!(get(found, source).as_f64().unwrap_or_default()),
        );
    }
    rect
}

/// The boxes of `found` relative to the formula `origin`.
fn relative_boxes(found: &Map<String, Value>, origin: &Map<String, Value>) -> Map<String, Value> {
    ["left", "top", "width", "height"]
        .into_iter()
        .map(|key| {
            let box_value = if matches!(key, "left" | "top") {
                number(found, key) - number(origin, key)
            } else {
                number(found, key)
            };
            (key.to_owned(), json!(box_value))
        })
        .collect()
}

/// The number at `key` of a measured rectangle.
fn number(rect: &Map<String, Value>, key: &str) -> f64 {
    rect.get(key).and_then(Value::as_f64).unwrap_or_default()
}

/// First difference between the measured formula and its recorded observation, or "".
fn compared(actual: &Value, expected: &Value, path: &str) -> String {
    if let Some(recorded) = expected.as_f64() {
        let measured = actual.as_f64().unwrap_or_default();
        if (measured - recorded).abs() <= 1. / 64. {
            return String::new();
        }
        return format!("{path}: {measured} != {recorded}");
    }
    if let Some(recorded) = expected.as_array() {
        return compared_entries(actual, recorded, path);
    }
    let (Some(observed), Some(recorded)) = (actual.as_object(), expected.as_object()) else {
        return if actual == expected {
            String::new()
        } else {
            format!("{path}: text differs")
        };
    };
    let keys = |object: &Map<String, Value>| object.keys().cloned().collect::<Vec<_>>();
    if keys(observed) != keys(recorded) {
        return format!("{path}: fields differ");
    }
    for (key, value) in recorded {
        let found = compared(
            observed.get(key).unwrap_or(&Value::Null),
            value,
            &format!("{path}/{key}"),
        );
        if !found.is_empty() {
            return found;
        }
    }
    String::new()
}

/// First difference between two measurement lists, preserving their lengths and ordering.
fn compared_entries(actual: &Value, expected: &[Value], path: &str) -> String {
    let Some(observed) = actual.as_array() else {
        return format!("{path}: text differs");
    };
    if observed.len() != expected.len() {
        return format!("{path}: {} != {} entries", observed.len(), expected.len());
    }
    for (index, value) in expected.iter().enumerate() {
        let found = compared(&observed[index], value, &format!("{path}/{index}"));
        if !found.is_empty() {
            return found;
        }
    }
    String::new()
}

/// Names of the painted roles that do not resolve to the selected pack's colors.
fn paint_failures(colors: &JsValue, thinking: bool) -> Vec<String> {
    let root = root().expect("a Markdown document is shown");
    let mut failures = Vec::new();
    let text = if thinking { "muted" } else { "text" };
    let actual = style(&root, "color");
    let expected = rgb(&get(colors, text).as_string().unwrap_or_default());
    if actual != expected {
        failures.push(format!("text: {actual} != {expected}"));
    }
    for (name, selector, property, role) in PAINTED {
        let Some(element) = document()
            .query_selector(&format!(".maestro-markdown {selector}"))
            .unwrap()
        else {
            failures.push(format!("{name}: missing"));
            continue;
        };
        let actual = style(&element, property);
        let expected = rgb(&get(colors, role).as_string().unwrap_or_default());
        if actual != expected {
            failures.push(format!("{name}: {actual} != {expected}"));
        }
    }
    failures
}

/// The computed `property` of `element`.
fn style(element: &Element, property: &str) -> String {
    web_sys::window()
        .unwrap()
        .get_computed_style(element)
        .unwrap()
        .expect("a styled document")
        .get_property_value(property)
        .unwrap_or_default()
}

/// The page's color for a recorded pack value written as `#rrggbb`.
fn rgb(hex: &str) -> String {
    let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or_default();
    format!(
        "rgb({}, {}, {})",
        value >> 16,
        (value >> 8) & 255,
        value & 255
    )
}

/// The reference markup of a recorded formula, or none when it was not recorded.
fn recorded_formula(tex: &str, display: bool) -> Option<String> {
    FORMULAS.with(|cell| {
        let formulas = cell.get_or_init(|| {
            serde_json::from_str(include_str!("../fixtures/markdown/formulas.json")).unwrap()
        });
        let found = formulas.iter().find(|formula| {
            field(formula, "tex").and_then(Value::as_str) == Some(tex)
                && field(formula, "display").and_then(Value::as_bool) == Some(display)
        })?;
        Some(
            field(found, "html")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        )
    })
}

/// The child nodes of `list`, in document order.
fn nodes(list: &web_sys::NodeList) -> Vec<Node> {
    (0..list.length())
        .filter_map(|index| list.item(index))
        .collect()
}

/// The element children of `list`, in document order.
fn elements(list: &web_sys::NodeList) -> Vec<Element> {
    nodes(list)
        .into_iter()
        .filter(|node| node.node_type() == 1)
        .map(wasm_bindgen::JsCast::unchecked_into)
        .collect()
}

/// The children of reference `html`, parsed by the page's own parser.
fn fragment(html: &str) -> Vec<Node> {
    let template = document().create_element("template").unwrap();
    template.set_inner_html(html);
    let content = get(template.as_ref(), "content");
    let list: web_sys::NodeList = get(&content, "childNodes").unchecked_into();
    nodes(&list)
}

/// A comparison container whose children are `children`.
fn container(children: Vec<Value>) -> Value {
    let mut object = Map::new();
    object.insert("t".to_owned(), Value::String("div".to_owned()));
    object.insert("a".to_owned(), Value::Object(Map::new()));
    object.insert("c".to_owned(), Value::Array(children));
    Value::Object(object)
}

/// A neutral tree of either reference markup or native nodes.
fn project(node: &Node, math: bool, side: Side) -> Option<Value> {
    match node.node_type() {
        3 => Some(Value::String(node.node_value().unwrap_or_default())),
        1 => Some(projected(&node.clone().unchecked_into(), math, side)),
        _ => None,
    }
}

/// A neutral tree of one element and its children.
fn projected(element: &Element, math: bool, side: Side) -> Value {
    let tag = element.tag_name().to_lowercase();
    let class = element.get_attribute("class");
    if tag == "code-block" {
        return json!([
            "code-block",
            element.get_attribute("language").unwrap_or_default(),
            decode(&element.get_attribute("code").unwrap_or_default()),
        ]);
    }
    if class.as_deref() == Some("maestro-code") {
        return json!([
            "code-block",
            text_of(element, ".maestro-code-label"),
            text_of(element, "code"),
        ]);
    }
    if class.as_deref() == Some("maestro-code-wrap")
        || wrapped_listing(&tag, class.as_deref(), element)
    {
        return element
            .first_element_child()
            .map_or(Value::Null, |child| projected(&child, math, side));
    }
    let math = math || MATH_CLASSES.contains(&first_class(class.as_deref()).as_str());
    let mut fields = Map::new();
    for (name, value) in attributes(element) {
        if name != "class" {
            let value = attribute_value(&name, &value, side.is_reference());
            fields.insert(name, Value::String(value));
        }
    }
    if math && class.is_some() {
        let class = class.unwrap_or_default();
        let value = if side.is_reference() {
            renamed(&class)
        } else {
            class
        };
        fields.insert("class".to_owned(), Value::String(value));
    } else if let Some((_, role)) = ROLES
        .iter()
        .find(|(name, _)| class.as_deref() == Some(name))
    {
        fields.insert("role".to_owned(), Value::String((*role).to_owned()));
    }
    let mut sorted: Vec<_> = fields.into_iter().collect();
    sorted.sort_by(|left, right| left.0.cmp(&right.0));
    let mut object = Map::new();
    object.insert("t".to_owned(), Value::String(tag));
    object.insert("a".to_owned(), Value::Object(sorted.into_iter().collect()));
    object.insert(
        "c".to_owned(),
        Value::Array(kids(&nodes(&element.child_nodes()), math, side)),
    );
    Value::Object(object)
}

/// The projected children of `children`, skipping the layout text between blocks.
fn kids(children: &[Node], math: bool, side: Side) -> Vec<Value> {
    let mut projected_children: Vec<Value> = Vec::new();
    for (index, child) in children.iter().enumerate() {
        let spacer = child.node_type() == 3
            && is_newlines(&child.node_value().unwrap_or_default())
            && !math
            && (index == 0
                || index + 1 == children.len()
                || is_block(children.get(index.wrapping_sub(1)))
                || is_block(children.get(index + 1)));
        if spacer {
            continue;
        }
        let Some(found) = project(child, math, side) else {
            continue;
        };
        match (found, projected_children.last_mut()) {
            (Value::String(found), Some(Value::String(previous))) => previous.push_str(&found),
            (found, _) => projected_children.push(found),
        }
    }
    projected_children
}

/// Whether `node` is an element whose adjacent newlines are layout.
fn is_block(node: Option<&Node>) -> bool {
    node.is_some_and(|node| {
        node.node_type() == 1
            && BLOCKS.contains(&node.clone().unchecked_into::<Element>().tag_name().as_str())
    })
}

/// Whether `text` is only newlines.
fn is_newlines(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|character| character == '\n')
}

/// The first class name of `class`.
fn first_class(class: Option<&str>) -> String {
    class
        .unwrap_or_default()
        .split(' ')
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// `class` with the names the matching math renderer spells differently renamed.
fn renamed(class: &str) -> String {
    class
        .split(' ')
        .map(|token| {
            RENAMES
                .iter()
                .find(|(name, _)| *name == token)
                .map_or(token, |(_, spelled)| *spelled)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether a block wrapper holds a listing as its first element.
fn wrapped_listing(tag: &str, class: Option<&str>, element: &Element) -> bool {
    tag == "div"
        && class == Some("mt-2")
        && element
            .first_element_child()
            .is_some_and(|child| child.tag_name() == "CODE-BLOCK")
}

/// The text content of the first element matching `selector` inside `element`.
fn text_of(element: &Element, selector: &str) -> String {
    element
        .query_selector(selector)
        .unwrap()
        .and_then(|found| found.text_content())
        .unwrap_or_default()
}

/// Every attribute of `element` in source order.
fn attributes(element: &Element) -> Vec<(String, String)> {
    let list = get(element.as_ref(), "attributes");
    let mut found = Vec::new();
    let mut index = 0;
    loop {
        let attribute = invoke(&list, "item", &[&JsValue::from(index)]).unwrap_or(JsValue::NULL);
        if attribute.is_null() || attribute.is_undefined() {
            return found;
        }
        found.push((
            get(&attribute, "name").as_string().unwrap_or_default(),
            get(&attribute, "value").as_string().unwrap_or_default(),
        ));
        index += 1;
    }
}

/// An attribute value as the comparison reads it.
fn attribute_value(name: &str, value: &str, reference: bool) -> String {
    let mut value = if name == "style" {
        collapsed_style(value)
    } else {
        value.to_owned()
    };
    if reference && name == "title" {
        value = value
            .strip_prefix("ParseError: ")
            .unwrap_or(&value)
            .to_owned();
    }
    if reference {
        value = value.replace("#cc0000", "var(--maestro-color-error)");
    }
    value
}

/// `style` without the spacing after its declarations, as the reference spells it.
fn collapsed_style(style: &str) -> String {
    let mut collapsed = String::with_capacity(style.len());
    let mut skip = false;
    for character in style.chars() {
        if skip && character.is_whitespace() {
            continue;
        }
        skip = matches!(character, ':' | ';');
        collapsed.push(character);
    }
    collapsed
}

/// The bytes a recording writes as base64, as text.
fn decode(base64: &str) -> String {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64)
        .unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The children of `nodes`, projected after cloning them into one container.
fn children(nodes: &[Node], side: Side) -> Vec<Value> {
    let holder = document().create_element("div").unwrap();
    for node in nodes {
        holder
            .append_child(&node.clone_node_with_deep(true).unwrap())
            .unwrap();
    }
    match project(&holder, false, side) {
        Some(Value::Object(mut object)) => match object.shift_remove("c") {
            Some(Value::Array(children)) => children,
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// The string at `key` of a recorded value.
fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.as_object()?.get(key)
}

/// First difference between the native structure and reference markup, or "".
fn first_difference(actual: &Value, expected: &Value, path: &str) -> String {
    let leaf = |value: &Value| matches!(value, Value::String(_) | Value::Null | Value::Array(_));
    if leaf(actual) || leaf(expected) {
        return if actual == expected {
            String::new()
        } else {
            format!("{path}: {} != {}", serialized(actual), serialized(expected))
        };
    }
    let (Some(actual), Some(expected)) = (actual.as_object(), expected.as_object()) else {
        return String::new();
    };
    let tag = |object: &Map<String, Value>| {
        object
            .get("t")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    if tag(actual) != tag(expected) {
        return format!("{path}: <{}> != <{}>", tag(actual), tag(expected));
    }
    let attributes = |object: &Map<String, Value>| object.get("a").cloned().unwrap_or(Value::Null);
    if attributes(actual) != attributes(expected) {
        return format!(
            "{path}<{}>: attrs {} != {}",
            tag(actual),
            serialized(&attributes(actual)),
            serialized(&attributes(expected))
        );
    }
    let children = |object: &Map<String, Value>| {
        object
            .get("c")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let name = tag(actual);
    let (actual, expected) = (children(actual), children(expected));
    if actual.len() != expected.len() {
        return format!(
            "{path}<{name}>: {} != {} children: {} != {}",
            actual.len(),
            expected.len(),
            truncated(&serialized(&Value::Array(actual))),
            truncated(&serialized(&Value::Array(expected)))
        );
    }
    for (index, child) in actual.iter().enumerate() {
        let found = first_difference(child, &expected[index], &format!("{path}/{name}[{index}]"));
        if !found.is_empty() {
            return found;
        }
    }
    String::new()
}

/// `value` as the page's JSON spells it.
fn serialized(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// The first 400 characters of `text`.
fn truncated(text: &str) -> String {
    text.chars().take(400).collect()
}

/// The recorded semantic tree of `nodes`, with each formula kept as its markup.
fn semantic_tree(items: &[Node]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for node in items {
        if node.node_type() == 3 {
            let text = node.node_value().unwrap_or_default();
            let blur = node.previous_sibling().is_none()
                || node.next_sibling().is_none()
                || is_block(node.previous_sibling().as_ref())
                || is_block(node.next_sibling().as_ref());
            if !is_newlines(&text) || !blur {
                push_text(&mut out, &text);
            }
            continue;
        }
        if node.node_type() != 1 {
            continue;
        }
        let element: Element = node.clone().unchecked_into();
        let tag = element.tag_name().to_lowercase();
        let class = first_class(element.get_attribute("class").as_deref());
        let children = || semantic_tree(&nodes(&element.child_nodes()));
        match class.as_str() {
            "maestro-math-block" => out.push(json!({
                "kind": "math-block",
                "html": element
                    .first_element_child()
                    .map(|child| child.outer_html())
                    .unwrap_or_default(),
            })),
            "katex" | "katex-error" => out.push(json!({
                "kind": "math-inline",
                "html": element.outer_html(),
            })),
            "maestro-code-wrap" => {
                let code = text_of(&element, "code");
                out.push(json!({
                    "kind": "code-block",
                    "children": [{
                        "kind": "text",
                        "text": code.strip_suffix('\n').unwrap_or(&code),
                    }],
                }));
            }
            "maestro-table-scroll" => out.extend(children()),
            _ if tag == "table" => out.push(table(&element)),
            _ => match tag.as_str() {
                "code" => out.push(json!({
                    "kind": "code",
                    "text": element.text_content().unwrap_or_default(),
                })),
                "p" | "strong" | "em" | "a" | "blockquote" | "ul" | "ol" | "li" => {
                    out.push(json!({"kind": kind_of(&tag), "children": children()}));
                }
                _ => out.extend(children()),
            },
        }
    }
    out
}

/// The recorded semantic tree of one table.
fn table(element: &Element) -> Value {
    let rows = elements(&element.query_selector_all("tr").unwrap())
        .iter()
        .map(|row| {
            let kind = if row
                .parent_element()
                .is_some_and(|parent| parent.tag_name() == "THEAD")
            {
                "head"
            } else {
                "row"
            };
            let cells = elements(&row.child_nodes())
                .iter()
                .map(|cell| {
                    json!({"kind": "cell", "children": semantic_tree(&nodes(&cell.child_nodes()))})
                })
                .collect::<Vec<_>>();
            json!({"kind": kind, "children": cells})
        })
        .collect::<Vec<_>>();
    json!({"kind": "table", "children": rows})
}

/// The recorded kind of a semantic element.
fn kind_of(tag: &str) -> &'static str {
    match tag {
        "p" => "paragraph",
        "strong" => "strong",
        "em" => "em",
        "a" => "link",
        "blockquote" => "quote",
        "ul" | "ol" => "list",
        _ => "item",
    }
}

/// Append `text`, joining it with an immediately preceding text node.
fn push_text(out: &mut Vec<Value>, text: &str) {
    match out.last_mut() {
        Some(Value::Object(last)) if last.get("kind").and_then(Value::as_str) == Some("text") => {
            let joined = format!(
                "{}{text}",
                last.get("text").and_then(Value::as_str).unwrap_or_default()
            );
            last.insert("text".to_owned(), Value::String(joined));
        }
        _ => out.push(json!({"kind": "text", "text": text})),
    }
}

/// First difference between the shown tree and a recorded one, or "".
fn tree_node_difference(actual: &Value, expected: &Value, path: &str) -> String {
    let kind = field(actual, "kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let recorded = field(expected, "kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if kind != recorded {
        return format!("{path}: {kind} != {recorded}");
    }
    match kind {
        "text" | "code" => {
            let found = field(actual, "text").cloned().unwrap_or(Value::Null);
            let wanted = field(expected, "text").cloned().unwrap_or(Value::Null);
            if found == wanted {
                return String::new();
            }
            format!("{path}: {} != {}", serialized(&found), serialized(&wanted))
        }
        "math-inline" | "math-block" => {
            let tex = field(expected, "text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let Some(recorded) = recorded_formula(&tex, kind == "math-block") else {
                return format!("{path}: no recorded formula {}", serialized(&json!(tex)));
            };
            let found = math_projection(&text(field(actual, "html")), Side::Native);
            if found == math_projection(&recorded, Side::Reference) {
                return String::new();
            }
            format!("{path}: formula {} differs", serialized(&json!(tex)))
        }
        _ => {
            let shown = values(actual, "children");
            let recorded = values(expected, "children");
            if shown.len() != recorded.len() {
                return format!(
                    "{path}<{kind}>: {} != {} children: {}",
                    shown.len(),
                    recorded.len(),
                    truncated(&serialized(&Value::Array(shown)))
                );
            }
            for (index, child) in recorded.iter().enumerate() {
                let found =
                    tree_node_difference(&shown[index], child, &format!("{path}/{kind}[{index}]"));
                if !found.is_empty() {
                    return found;
                }
            }
            String::new()
        }
    }
}

/// The string at `key` of a recorded value, or "".
fn text(value: Option<&Value>) -> String {
    value.and_then(Value::as_str).unwrap_or_default().to_owned()
}

/// The array at `key` of a recorded value, or an empty list.
fn values(value: &Value, key: &str) -> Vec<Value> {
    field(value, key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// The neutral projection of one formula's markup, or "null" when it has no element.
fn math_projection(html: &str, side: Side) -> String {
    let element = fragment(html)
        .into_iter()
        .find(|node| node.node_type() == 1);
    match element {
        Some(element) => serialized(&project(&element, true, side).unwrap_or(Value::Null)),
        None => "null".to_owned(),
    }
}
