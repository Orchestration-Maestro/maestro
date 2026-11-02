//! Public card preparation and default registration contracts.
#![cfg(all(test, not(target_arch = "wasm32")))]
use crate::recipes;
use maestro_app::JsonObject;
use maestro_web::tools::{
    renderer_registry::TOOL_RENDERERS,
    types::{ToolParams, ToolRendererBinding},
};
use maestro_web::{
    BashRenderer, ToolRenderer, get_tool_renderer, register_tool_renderer, render_tool,
    set_show_json_mode,
};
use serde_json::{Value, json};
use std::rc::Rc;

#[test]
fn bash_default_registration_respects_replacement_removal_and_json_mode() {
    let original = get_tool_renderer::<JsonObject, Rc<dyn Fn()>>("bash").unwrap();
    assert!(get_tool_renderer::<JsonObject, Value>("calculate").is_none());
    assert!(get_tool_renderer::<JsonObject, Value>("get_current_time").is_none());
    let again = get_tool_renderer::<JsonObject, Rc<dyn Fn()>>("bash").unwrap();
    assert!(Rc::ptr_eq(&original, &again));
    let args: JsonObject = serde_json::from_value(json!({"command":"owned"})).unwrap();
    let result = recipes::message(Some(Rc::new(|| {}) as Rc<dyn Fn()>));
    let replacement: Rc<dyn ToolRenderer<JsonObject, Rc<dyn Fn()>>> =
        Rc::new(recipes::Renderer(Rc::new(|_, _, _| {
            Ok(recipes::content("replacement", true))
        })));
    register_tool_renderer("bash", ToolRendererBinding::from_raw(replacement.clone()));
    assert!(Rc::ptr_eq(
        &replacement,
        &get_tool_renderer::<JsonObject, Rc<dyn Fn()>>("bash").unwrap()
    ));
    assert!(
        render_tool(
            "bash",
            Some(ToolParams::object(&args)),
            Some(&result),
            false
        )
        .unwrap()
        .is_custom
    );
    set_show_json_mode(true);
    assert!(
        !render_tool(
            "bash",
            Some(ToolParams::object(&args)),
            Some(&result),
            false
        )
        .unwrap()
        .is_custom
    );
    assert!(Rc::ptr_eq(
        &replacement,
        &get_tool_renderer::<JsonObject, Rc<dyn Fn()>>("bash").unwrap()
    ));
    set_show_json_mode(false);
    drop(TOOL_RENDERERS.with(|all| all.borrow_mut().shift_remove("bash")));
    assert!(
        !render_tool(
            "bash",
            Some(ToolParams::object(&args)),
            Some(&result),
            false
        )
        .unwrap()
        .is_custom
    );
    assert!(get_tool_renderer::<JsonObject, Value>("bash").is_none());
    assert!(
        !original
            .render(Some(&args), Some((&result).into()), false)
            .unwrap()
            .is_custom
    );
}

#[test]
fn tool_card_recipes_own_borrowed_fields_without_reading_details() {
    use maestro_web::{CalculateRenderer, GetCurrentTimeRenderer};
    recipes::reset();
    let details: Rc<dyn Fn()> = Rc::new(|| panic!("details were executed"));
    let released = Rc::downgrade(&details);
    let result = recipes::message(Some(details));
    let args: JsonObject = serde_json::from_value(
        json!({"command":"command", "expression":"expression", "timezone":"timezone"}),
    )
    .unwrap();
    let renderers: [&dyn ToolRenderer<JsonObject, Rc<dyn Fn()>>; 3] =
        [&BashRenderer, &CalculateRenderer, &GetCurrentTimeRenderer];
    let owned: Vec<_> = renderers
        .into_iter()
        .map(|renderer| {
            renderer
                .render(Some(&args), Some((&result).into()), false)
                .unwrap()
        })
        .collect();
    drop(result);
    drop(args);
    assert!(
        released.upgrade().is_none(),
        "recipes retained unused native details"
    );
    assert!(owned.iter().all(|recipe| !recipe.is_custom));
    // The failed result paints each tool's glyph in the error state, with the borrowed command,
    // expression and time zone kept after their owners were dropped.
    let views: Vec<_> = owned.iter().map(recipes::view).collect();
    assert_eq!(
        views,
        [
            json!({"state": "Error", "glyph": "SquareTerminal",
                "caption": {"key": "Running command...", "suffix": ""},
                "body": {"console": "> command\n\none\ntwo", "variant": "error"}}),
            json!({"state": "Error", "glyph": "Calculator", "caption": {"text": "expression"},
                "body": {"error": "one\ntwo"}}),
            json!({"state": "Error", "glyph": "Clock",
                "caption": {"key": "Getting current time in", "suffix": " timezone"},
                "body": {"error": "one\ntwo"}}),
        ]
    );
}
