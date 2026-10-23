//! Corrections Maestro makes to recorded page trees, one per corrected behaviour.
use serde_json::{Value, json};

/// Apply the corrections that concern one recording to its page trees.
///
/// # Panics
/// Panics when a recording no longer holds the node a correction replaces.
pub(crate) fn correct(name: &str, pages: &mut [Value]) {
    match name {
        // A non-breaking hyphen is visible text, not an invisible break opportunity.
        "inline-basics" => {
            let hyphen = pages[0]
                .pointer_mut("/children/0/children/0/children/5/children/0")
                .unwrap();
            assert_eq!(hyphen["tag"], "wbr");
            *hyphen = json!({ "text": "\u{2011}" });
        }
        // The document reader quotes a font name that is not a valid family as a CSS string,
        // so the symbol keeps its font where the recording dropped the invalid name.
        "inline-hostile-values" => {
            let symbol = pages[0]
                .pointer_mut("/children/0/children/0/children/2/children/0")
                .unwrap();
            assert_eq!(symbol["children"], json!([{ "text": "A" }]));
            symbol["attributes"] = json!([["style", "font-family:\"Quo\\\"te&\""]]);
        }
        // A section that a page-break-before style closes belongs to the section group after
        // it, so its page takes that group's size rather than the previous paragraph's.
        "style-page-break-before-after-section" => {
            assert_ne!(pages[1]["attributes"], pages[2]["attributes"]);
            pages[1]["attributes"] = pages[2]["attributes"].clone();
        }
        // A link whose target a browser would run as script keeps its element and text but
        // carries no `href`.
        "hyperlink-targets" => {
            let link = pages[0]
                .pointer_mut("/children/0/children/1/children/0")
                .unwrap();
            assert_eq!(link["attributes"], json!([["href", "javascript:alert(1)"]]));
            link["attributes"] = json!([]);
        }
        // A link in a footer or a note reads the relationships of the part that holds it, not
        // the main document's.
        "part-relationship-links" => {
            for (pointer, recorded, target) in [
                (
                    "/children/1/children/0/children/0/children/0",
                    "https://example.test/document",
                    "https://example.test/note",
                ),
                (
                    "/children/2/children/0/children/0",
                    "",
                    "https://example.test/footer",
                ),
            ] {
                let link = pages[0].pointer_mut(pointer).unwrap();
                assert_eq!(link["attributes"], json!([["href", recorded]]));
                link["attributes"] = json!([["href", target]]);
            }
        }
        _ => {}
    }
}

/// The bytes the main document's relationships name for a note image.
pub(crate) const DOCUMENT_IMAGE_BYTES: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAAEUlEQVR4nGP4z8DwHwQZGBgAJ/AE/BTfJvIAAAAASUVORK5CYII=";

/// The bytes the footnotes part's own relationship names for the same note.
pub(crate) const NOTES_IMAGE_BYTES: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYPj/HwADAgH/eAqTPwAAAABJRU5ErkJggg==";

/// Apply the corrections that concern one recorded preview rendering to its containers.
pub(crate) fn correct_preview(name: &str, expected: &mut Vec<Value>, styles: &mut Vec<Value>) {
    match name {
        "alt-chunk" | "alt-chunk-script" | "alt-chunk-missing-part" => sandbox_frames(expected),
        "theme-value-escape" => drop_escaping_theme_values(expected),
        "style-selector-escape" => drop_unusable_style_rules(expected),
        "numbering-id-escape" => drop_unusable_numbering_rules(expected),
        "image-missing-part" => walk(expected, &mut drop_missing_source),
        "image-in-header-and-note" => walk(expected, &mut replace_note_image),
        "embedded-fonts" | "embedded-fonts-data-urls" => drop_missing_font(expected),
        "numbering-picture-bullet" => drop_missing_bullet(expected),
        "font-name-escape" => enclose_font_name(expected),
        "math-structures" => underline_bottom_bar(expected),
        "numbering-level-text" => keep_level_text_literals(expected),
        "vml-defaults" => walk(expected, &mut drop_missing_href),
        _ => {}
    }
    let _ = styles;
}

/// Show each alternate chunk's text in a frame that runs no script.
fn sandbox_frames(expected: &mut [Value]) {
    walk(expected, &mut |node| {
        if tag(node) == "iframe" {
            drop_null(node, "srcdoc");
            set_attribute(node, "sandbox", "");
        }
    });
}

/// Drop the theme values that would leave their own declaration, keeping the inert one.
fn drop_escaping_theme_values(expected: &mut [Value]) {
    let css = style_css(expected, 1);
    assert!(css.contains("--docx-majorHAnsi-font: A;}html{"));
    assert!(css.contains("--docx-dk1-color: #000;}body{"));
    let kept: Vec<&str> = css
        .lines()
        .filter(|line| {
            !line.contains("--docx-majorHAnsi-font:") && !line.contains("--docx-dk1-color:")
        })
        .collect();
    *css = format!("{}\n", kept.join("\n"));
    assert!(css.contains("--docx-lt1-color: #00FF00</style>"));
}

/// Skip the style rules whose class is not a plain identifier.
fn drop_unusable_style_rules(expected: &mut [Value]) {
    let css = style_css(expected, 1);
    assert!(css.contains("p.docx_x{}body{background:rgb(7,-8,-9)}-y span {"));
    assert!(css.contains("p.docx_a,body span {"));
    *css = drop_rules(css, &["p.docx_x{}", "p.docx_a,body"]);
}

/// Skip the list level whose counter is not an identifier, and its reset entry.
fn drop_unusable_numbering_rules(expected: &mut [Value]) {
    let css = style_css(expected, 1);
    assert!(css.contains("p.docx-num-1{}body{background:rgb(1,1,1)}.z-0:before {"));
    let corrected = drop_rules(css, &["p.docx-num-1{}"]).replace(
        "counter-reset: docx-num-1{}body{background:rgb(1,1,1)}.z-0 0 ",
        "counter-reset: ",
    );
    *css = corrected;
}

/// Keep a missed image without a source.
fn drop_missing_source(node: &mut Value) {
    if tag(node) == "img" {
        drop_null(node, "src");
    }
}

/// Keep a shape whose image relationship does not resolve without an `href`.
fn drop_missing_href(node: &mut Value) {
    if tag(node) == "image" {
        drop_null(node, "href");
    }
}

/// Emit no rule for a face whose bytes cannot be loaded.
fn drop_missing_font(expected: &mut Vec<Value>) {
    assert!(style_css(expected, 2).contains("font-family: 'Missing Part'"));
    assert!(style_css(expected, 2).contains("url(null)"));
    expected.remove(2);
}

/// Emit no rule for a picture bullet whose image is missing.
fn drop_missing_bullet(expected: &mut Vec<Value>) {
    let missing = expected
        .iter()
        .position(|node| {
            node["css"]
                .as_str()
                .is_some_and(|css| css.contains("url(null)"))
        })
        .unwrap();
    assert_eq!(
        expected[missing]["css"],
        ".docx-wrapper { --docx-ridgone: url(null) }"
    );
    expected.remove(missing);
}

/// Keep a hostile font name inside its own rule by enclosing it as a string.
fn enclose_font_name(expected: &mut [Value]) {
    let css = style_css(expected, 2);
    assert!(css.contains(RECORDED_FONT_FAMILY));
    *css = css.replace(RECORDED_FONT_FAMILY, ENCLOSED_FONT_FAMILY);
}

/// Underline the base of a bottom bar, as a bar with the same placement underlines it.
fn underline_bottom_bar(expected: &mut [Value]) {
    walk(expected, &mut |node| {
        let base = node
            .get("children")
            .and_then(Value::as_array)
            .filter(|children| children.len() == 1 && tag(&children[0]) == "mrow");
        if tag(node) == "mrow" && text(node) == "u" && base.is_some() {
            set_attribute(node, "style", "text-decoration:underline");
        }
    });
}

/// Keep the level text's literals instead of inventing a counter for each `%`.
fn keep_level_text_literals(expected: &mut [Value]) {
    let css = style_css(expected, 1);
    assert!(css.contains("counter(docx-num-3-NaN, decimal)"));
    *css = css.replace(RECORDED_LEVEL_TEXT, CORRECTED_LEVEL_TEXT);
    assert!(!css.contains("NaN"));
}

/// The level text the reference writes for the recorded level, with a counter for every `%`.
const RECORDED_LEVEL_TEXT: &str = "  content: \"\"counter(docx-num-3-NaN, decimal)\"\"counter(docx-num-3-0, decimal)\" \"q\" \\ 100\"counter(docx-num-3-NaN, decimal)\" \"counter(docx-num-3-9, decimal)\" \"counter(docx-num-3-NaN, decimal)\"x \"counter(docx-num-3-NaN, decimal)\"\\9\";";

/// The same text with each `%` kept as text and only `%1` and `%10` read as placeholders.
const CORRECTED_LEVEL_TEXT: &str = "  content: \"%\"counter(docx-num-3-0, decimal)\" \\\"q\\\" \\\\ 100% \"counter(docx-num-3-9, decimal)\" %x %\\9\";";

/// The font family the reference writes for the recorded hostile name.
const RECORDED_FONT_FAMILY: &str =
    "  font-family: 'x'; } body { background: rgb(2, 2, 2) } .q { a: 'b';";

/// The enclosed font family the shared enclosure writes for the recorded hostile name.
const ENCLOSED_FONT_FAMILY: &str =
    "  font-family: \"x'; } body { background: rgb(2, 2, 2) } .q { a: 'b\";";

/// The prefix of a data URL for loaded bytes.
const IMAGE_URL: &str = "data:application/octet-stream;base64,";

/// Point a recorded note image at the bytes its own part's relationship names.
fn replace_note_image(node: &mut Value) {
    if tag(node) != "img" {
        return;
    }
    for pair in attributes_mut(node) {
        if pair[1]
            .as_str()
            .is_some_and(|url| url.ends_with(DOCUMENT_IMAGE_BYTES))
        {
            pair[1] = Value::from(format!("{IMAGE_URL}{NOTES_IMAGE_BYTES}"));
        }
    }
}

/// Visit every recorded element of a container, children included.
fn walk(nodes: &mut [Value], visit: &mut impl FnMut(&mut Value)) {
    for node in nodes {
        if node.get("text").is_some() {
            continue;
        }
        visit(node);
        if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
            walk(children, visit);
        }
    }
}

/// One element's tag name, or an empty name for a text node.
fn tag(node: &Value) -> &str {
    node.get("tag").and_then(Value::as_str).unwrap_or_default()
}

/// The text an element holds, its children's text included.
fn text(node: &Value) -> String {
    if let Some(text) = node.get("text").and_then(Value::as_str) {
        return text.to_owned();
    }
    node.get("children")
        .and_then(Value::as_array)
        .map(|children| children.iter().map(text).collect())
        .unwrap_or_default()
}

/// The style text of the container's element at a position.
///
/// # Panics
/// Panics when that element is not a style element.
fn style_css(container: &mut [Value], index: usize) -> &mut String {
    match container[index].get_mut("css") {
        Some(Value::String(css)) => css,
        _ => panic!("element {index} is not a style element"),
    }
}

/// Remove the attributes whose value is the reference's written `null`.
fn drop_null(node: &mut Value, name: &str) {
    let attributes = attributes_mut(node);
    attributes.retain(|attribute| attribute[0] != name || attribute[1] != "null");
}

/// Assign an attribute in sorted position.
fn set_attribute(node: &mut Value, name: &str, value: &str) {
    let slot = node
        .as_object_mut()
        .expect("element record")
        .entry("attributes")
        .or_insert_with(|| Value::Array(Vec::new()));
    let attributes = slot.as_array_mut().expect("element attributes");
    attributes.retain(|attribute| attribute[0] != name);
    attributes.push(json!([name, value]));
    attributes.sort_by(|left, right| left[0].as_str().cmp(&right[0].as_str()));
}

/// Borrow an element's attribute pairs.
fn attributes_mut(node: &mut Value) -> &mut Vec<Value> {
    node.as_object_mut()
        .expect("element record")
        .entry("attributes")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .expect("element attributes")
}

/// Drop the rules whose selector starts with one of the recorded injections.
fn drop_rules(css: &str, injections: &[&str]) -> String {
    let mut kept = String::new();
    for block in css.split_inclusive("}\n") {
        let injected = injections
            .iter()
            .any(|injection| block.starts_with(injection));
        if !injected {
            kept.push_str(block);
        }
    }
    kept
}
