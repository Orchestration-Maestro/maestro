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
