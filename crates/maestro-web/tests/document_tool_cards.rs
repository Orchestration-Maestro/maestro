//! Native preparation and independent opened-owner transfer for document cards.
#![cfg(test)]
use crate::{document_card_support as cards, recipes};
use maestro_app::JsonObject;
use maestro_web::{
    get_tool_renderer, register_tool_renderer, render_tool, set_show_json_mode,
    tools::{
        renderer_registry::TOOL_RENDERERS,
        types::{ToolParams, ToolRendererBinding},
    },
};
use serde_json::{Value, json};
use std::rc::Rc;

/// The tools index (index.ts:2-3,10) initializes document cards before bash.
#[test]
fn document_cards_register_once_with_replacement_removal_and_json_mode() {
    let original_order =
        TOOL_RENDERERS.with(|all| all.borrow().keys().cloned().collect::<Vec<_>>());
    assert_eq!(
        original_order,
        ["javascript_repl", "extract_document", "bash"]
    );
    for name in ["javascript_repl", "extract_document"] {
        let original = get_tool_renderer::<JsonObject, Value>(name).unwrap();
        assert!(Rc::ptr_eq(
            &original,
            &get_tool_renderer::<JsonObject, Value>(name).unwrap()
        ));
        let replacement: Rc<dyn maestro_web::ToolRenderer> =
            Rc::new(recipes::Renderer(Rc::new(|_, _, _| {
                Ok(recipes::content("replacement", true))
            })));
        register_tool_renderer(name, ToolRendererBinding::from_raw(replacement.clone()));
        let params = JsonObject::new();
        assert!(
            render_tool::<_, Value>(name, Some(ToolParams::object(&params)), None, false)
                .unwrap()
                .is_custom
        );
        set_show_json_mode(true);
        assert!(
            !render_tool::<_, Value>(name, Some(ToolParams::object(&params)), None, false)
                .unwrap()
                .is_custom
        );
        set_show_json_mode(false);
        assert!(Rc::ptr_eq(
            &replacement,
            &get_tool_renderer::<JsonObject, Value>(name).unwrap()
        ));
        assert_eq!(
            TOOL_RENDERERS.with(|all| all.borrow().keys().cloned().collect::<Vec<_>>()),
            original_order
        );
        drop(TOOL_RENDERERS.with(|all| all.borrow_mut().shift_remove(name)));
        assert!(get_tool_renderer::<JsonObject, Value>(name).is_none());
        assert!(
            !render_tool::<JsonObject, Value>(name, None, None, false)
                .unwrap()
                .is_custom
        );
        let position = original_order.iter().position(|n| n == name).unwrap();
        let previous = TOOL_RENDERERS.with(|all| {
            all.borrow_mut().shift_insert(
                position,
                name.into(),
                ToolRendererBinding::from_raw(original),
            )
        });
        drop(previous);
    }
}

#[test]
fn document_renderers_keep_model_json_and_native_operands() {
    use maestro_app::browser::tools::{
        extract_document::{ExtractDocumentParams, ExtractDocumentResult},
        javascript_repl::{JavaScriptReplParams, JavaScriptReplToolResult},
    };
    for name in ["javascript_repl", "extract_document"] {
        let params = json!({"title":"title", "code":"YWJj", "url":"url", "unused":true});
        let model: JsonObject = serde_json::from_value(params.clone()).unwrap();
        assert!(
            !render_tool::<_, Value>(name, Some(ToolParams::object(&model)), None, false)
                .unwrap()
                .is_custom
        );
        assert!(
            !render_tool::<_, Value>(name, Some(ToolParams::json(&params)), None, false)
                .unwrap()
                .is_custom
        );
    }
    let repl =
        get_tool_renderer::<JavaScriptReplParams, JavaScriptReplToolResult>("javascript_repl")
            .unwrap();
    let result = recipes::message(Some(JavaScriptReplToolResult { files: None }));
    assert!(
        !repl
            .render(
                Some(&JavaScriptReplParams {
                    title: "owned".into(),
                    code: "YWJj".into()
                }),
                Some((&result).into()),
                false
            )
            .unwrap()
            .is_custom
    );
    let extract =
        get_tool_renderer::<ExtractDocumentParams, ExtractDocumentResult>("extract_document")
            .unwrap();
    let result = recipes::message(Some(ExtractDocumentResult {
        extracted_text: "text".into(),
        file_name: "雪.txt".into(),
        format: "ßé".into(),
        size: 1280,
    }));
    assert!(
        !extract
            .render(
                Some(&ExtractDocumentParams { url: "url".into() }),
                Some((&result).into()),
                false
            )
            .unwrap()
            .is_custom
    );
}

#[test]
fn extraction_metadata_preserves_names_case_units_and_rounding() {
    // Exact native text is observed by the same named browser witness.
    let rows: Vec<_> = cards::fixture()
        .into_iter()
        .filter(|r| matches!(r.group.as_str(), "metadata" | "malformed"))
        .collect();
    assert!(!rows.is_empty());
    for row in rows {
        assert!(row.test.is_none());
        let prepared = cards::prepare(&row);
        assert_eq!(prepared.is_ok(), row.expected.is_some(), "{}", row.id);
        if let Ok(recipe) = prepared {
            assert!(!recipe.is_custom);
        }
    }
}

#[test]
fn document_field_policy_keeps_missing_null_falsy_and_composites() {
    let rows: Vec<_> = cards::fixture()
        .into_iter()
        .filter(|r| r.group == "domains")
        .collect();
    assert_eq!(rows.len(), 135);
    for row in rows {
        assert!(!cards::prepare(&row).unwrap().is_custom, "{}", row.id);
    }
}
