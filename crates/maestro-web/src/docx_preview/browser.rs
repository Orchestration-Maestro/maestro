//! Renderings mounted as browser nodes.
use super::{HtmlContent, HtmlElement, Namespace, Options, WordPreview, parse_async};
use crate::browser::attachment_content::{AttachmentHostError, browser::host_error};
use crate::browser::timer::{self, Timer};
use crate::utils::download::ObjectUrl;
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, JsValue};
use web_sys::{Document, Element, SvgGraphicsElement};

/// Browser resources a mounted preview names. Dropping it cancels pending shape sizing and
/// revokes its object URLs, so keep it while the preview is shown.
pub struct RenderedDocument {
    /// Pending shape sizing, cancelled on drop.
    _sizing: Option<Timer>,
    /// Object URLs the mounted nodes name, revoked on drop.
    _object_urls: Vec<ObjectUrl>,
}

/// Clear the containers and mount a rendering: styles into the style container (the body
/// container when none is given), then the body, then the resource styles.
///
/// # Errors
/// Returns a host error when the browser refuses a node operation.
pub fn render_document(
    document: &WordPreview,
    body_container: &Element,
    style_container: Option<&Element>,
    options: &Options,
) -> Result<RenderedDocument, AttachmentHostError> {
    let rendering = document.render(options);
    let style_container = style_container.unwrap_or(body_container);
    style_container.set_inner_html("");
    body_container.set_inner_html("");
    let owner = body_container
        .owner_document()
        .ok_or_else(|| AttachmentHostError::Host("Container has no document".into()))?;
    let mut shapes = Vec::new();
    for (container, elements) in [
        (style_container, &rendering.styles),
        (body_container, &rendering.body),
        (style_container, &rendering.resource_styles),
    ] {
        for element in elements {
            let node = to_node(&owner, element, &mut shapes).map_err(|error| host_error(&error))?;
            container
                .append_child(&node)
                .map_err(|error| host_error(&error))?;
        }
    }
    let sizing = if shapes.is_empty() {
        None
    } else {
        Some(size_shapes(shapes)?)
    };
    Ok(RenderedDocument {
        _sizing: sizing,
        _object_urls: rendering.object_urls,
    })
}

/// Parse and render a package; a package failure is a presentation error with its message.
///
/// # Errors
/// As [`parse_async`] (as [`AttachmentHostError::Presentation`]) and [`render_document`].
pub fn render_async(
    data: &[u8],
    body_container: &Element,
    style_container: Option<&Element>,
    options: Options,
) -> Result<(WordPreview, RenderedDocument), AttachmentHostError> {
    let document = parse_async(data, options)
        .map_err(|error| AttachmentHostError::Presentation(error.to_string()))?;
    let rendered = render_document(
        &document,
        body_container,
        style_container,
        &document.options,
    )?;
    Ok((document, rendered))
}

/// Build an element's node in its namespace: attributes in order, the sorted declarations as
/// one `style` attribute and text as text nodes. Shape images are collected for sizing.
fn to_node(
    document: &Document,
    element: &HtmlElement,
    shapes: &mut Vec<Element>,
) -> Result<Element, JsValue> {
    let namespace = match element.namespace() {
        Namespace::Html => "http://www.w3.org/1999/xhtml",
        Namespace::Svg => "http://www.w3.org/2000/svg",
        Namespace::MathMl => "http://www.w3.org/1998/Math/MathML",
    };
    let node = document.create_element_ns(Some(namespace), element.tag())?;
    for (name, value) in element.attributes() {
        node.set_attribute(name, value)?;
    }
    let declarations: Vec<String> = element
        .declarations()
        .map(|(property, value)| format!("{property}:{value}"))
        .collect();
    if !declarations.is_empty() {
        node.set_attribute("style", &declarations.join(";"))?;
    }
    for child in element.children() {
        match child {
            HtmlContent::Element(child) => {
                node.append_child(&to_node(document, child, shapes)?.into())?
            }
            HtmlContent::Text(text) => {
                node.append_child(&document.create_text_node(text).into())?
            }
        };
    }
    if element.namespace() == Namespace::Svg && element.tag() == "svg" {
        shapes.push(node.clone());
    }
    Ok(node)
}

/// One animation frame later, give each shape image the size of its shape's bounding box:
/// the ceiling of its right and bottom edges.
fn size_shapes(shapes: Vec<Element>) -> Result<Timer, AttachmentHostError> {
    let window =
        web_sys::window().ok_or_else(|| AttachmentHostError::Host("No browser window".into()))?;
    let size = move || {
        for shape in &shapes {
            let Some(Ok(bounds)) = shape
                .first_element_child()
                .and_then(|child| child.dyn_into::<SvgGraphicsElement>().ok())
                .map(|child| child.get_b_box())
            else {
                continue;
            };
            let right = f64::from(bounds.x()) + f64::from(bounds.width());
            let bottom = f64::from(bounds.y()) + f64::from(bounds.height());
            let mut text = ryu_js::Buffer::new();
            let _ = shape.set_attribute("width", text.format(right.ceil()));
            let _ = shape.set_attribute("height", text.format(bottom.ceil()));
        }
    };
    timer::frame(window, Rc::new(size)).map_err(|error| host_error(&error))
}
