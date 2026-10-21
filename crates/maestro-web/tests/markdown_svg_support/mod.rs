//! Strict source observations shared by native and actual-browser witnesses.
#![cfg(test)]
#[cfg(target_arch = "wasm32")]
mod browser_records;
#[cfg(not(target_arch = "wasm32"))]
mod native_records;
#[cfg(target_arch = "wasm32")]
pub use browser_records::{Case, Corpus};
#[cfg(not(target_arch = "wasm32"))]
pub use native_records::{Case, Corpus, Header};
/// Decode every committed field strictly.
/// # Panics
/// Panics if the committed recording has invalid fields.
#[must_use]
pub fn corpus() -> Corpus {
    use crate::support::strict_record;
    #[cfg(not(target_arch = "wasm32"))]
    let source = include_str!("../fixtures/markdown_svg_artifacts_native.json");
    #[cfg(target_arch = "wasm32")]
    let source = include_str!("../fixtures/markdown_svg_artifacts_browser.json");
    let valid: serde_json::Value = serde_json::from_str(source).unwrap();
    #[cfg(not(target_arch = "wasm32"))]
    {
        use native_records::{Case, Header, Toggle};
        strict_record::<Corpus>(&valid, &["toggles", "headers", "content"]);
        strict_record::<Toggle>(
            &valid["toggles"][0],
            &["language", "titles", "states", "events", "configured"],
        );
        strict_record::<Header>(&valid["headers"][0], &["language", "kind", "titles"]);
        strict_record::<Case>(
            &valid["content"][0],
            &["kind", "filename", "content", "mime", "bytes"],
        );
        for value in [serde_json::Value::Null, serde_json::json!([])] {
            let mut sibling = valid.clone();
            sibling["lifecycle"] = value;
            assert!(serde_json::from_value::<Corpus>(sibling).is_err());
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        use browser_records::{Case, Lifecycle, UrlEvent};
        strict_record::<Corpus>(&valid, &["content", "lifecycle"]);
        strict_record::<Case>(
            &valid["content"][0],
            &[
                "kind",
                "filename",
                "content",
                "renamed_filename",
                "edited_content",
                "language_class",
            ],
        );
        strict_record::<Lifecycle>(&valid["lifecycle"], &["contents", "independent", "events"]);
        strict_record::<UrlEvent>(&valid["lifecycle"]["events"][0], &["op", "id"]);
        for value in [serde_json::Value::Null, serde_json::json!([])] {
            let mut sibling = valid.clone();
            sibling["headers"] = value;
            assert!(serde_json::from_value::<Corpus>(sibling).is_err());
        }
    }
    serde_json::from_str(source).unwrap()
}
/// Supply a locale through the real dictionary owner.
/// # Panics
/// Panics if the controlled language host fails.
pub fn locale(i18n: &maestro_web::I18n, language: &str) {
    i18n.set_language(language).unwrap();
}
/// Complete admitted pointer activation.
#[must_use]
pub fn click(position: egui::Pos2) -> Vec<egui::Event> {
    crate::session_header_support::pointer(position, true)
        .into_iter()
        .chain(crate::session_header_support::pointer(position, false))
        .collect()
}

/// One complete keyboard activation without repeat or a modifier chord.
#[must_use]
pub fn key(key: egui::Key) -> Vec<egui::Event> {
    [true, false]
        .into_iter()
        .map(|pressed| egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        })
        .collect()
}
