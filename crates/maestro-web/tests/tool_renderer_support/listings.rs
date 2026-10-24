//! Literal and encoded listing routes and independently retained default subsections.
#![cfg(test)]
use super::{browser, message};
use crate::{
    markdown_support::{check, first_slot_center, js, settle},
    tool_renderers_browser::surface,
};
use maestro_web::{
    render_tool,
    tools::types::{ToolParams, ToolRenderState},
};
use serde_json::Value;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub async fn tool_output_uses_literal_listing_without_changing_encoded_listings() {
    let mut surface = surface();
    let mut result = message::<Value>(None);
    result.content.truncate(1);
    let maestro_app::UserBlock::Text(text) = &mut result.content[0] else {
        unreachable!()
    };
    text.text = "YWJj".into();
    let recipe = render_tool::<Value, Value>("literal", None, Some(&result), false).unwrap();
    let mut row = ToolRenderState::default();
    browser::draw(&mut surface, &recipe, &mut row);
    check("document.querySelector('.maestro-code code').textContent === 'YWJj'");
    browser::click(&mut surface, first_slot_center(), &recipe, &mut row);
    settle().await;
    assert_eq!(&*surface.effects.copies.borrow(), &["YWJj"]);
    drop(row);
    let mut encoded = maestro_web::components::source::CodeBlock::new(surface.host());
    encoded.code = "YWJj".into();
    surface.draw(|ui, view| encoded.render(ui, view).unwrap());
    check("document.querySelector('.maestro-code code').textContent === 'abc'");
    let center = first_slot_center();
    surface.click(center, |ui, view| encoded.render(ui, view).unwrap());
    settle().await;
    assert_eq!(&surface.effects.copies.borrow()[1..], &["abc"]);
}

#[wasm_bindgen]
pub fn default_subsections_release_removed_native_roots() {
    let mut surface = surface();
    let mut row = ToolRenderState::default();
    let input = serde_json::json!({"value":42});
    let result = message::<Value>(None);
    for (params, output, count) in [
        (None, None, 0),
        (Some(&input), None, 1),
        (Some(&input), Some(&result), 2),
        (None, Some(&result), 1),
        (None, None, 0),
    ] {
        let recipe =
            render_tool("unregistered", params.map(ToolParams::json), output, false).unwrap();
        browser::draw(&mut surface, &recipe, &mut row);
        assert_eq!(
            js("document.querySelectorAll('.maestro-code').length").as_f64(),
            Some(f64::from(count))
        );
        match count {
            2 => check("document.querySelectorAll('.maestro-code')[0] === keptInput"),
            1 if output.is_some() => check(
                "!keptInput.isConnected && document.querySelector('.maestro-code') === keptOutput",
            ),
            0 if output.is_none() => {
                let _ = js(
                    "if(globalThis.keptOutput && keptOutput.isConnected) throw new Error('output retained')",
                );
            }
            _ => {}
        }
        if count == 1 && params.is_some() {
            let _ = js("globalThis.keptInput=document.querySelector('.maestro-code')");
        }
        if count == 2 {
            let _ = js("globalThis.keptOutput=document.querySelectorAll('.maestro-code')[1]");
        }
    }
    super::reset();
    maestro_web::register_tool_renderer(
        "route",
        maestro_web::tools::types::ToolRendererBinding::from_raw(std::rc::Rc::new(
            super::Renderer(std::rc::Rc::new(
                |_: Option<&Value>,
                 _: Option<maestro_web::tools::types::ToolResultMessageView<'_>>,
                 _| Ok(super::content("custom", true)),
            )),
        )),
    );
    maestro_web::set_show_json_mode(true);
    let forced =
        render_tool::<Value, Value>("route", Some(ToolParams::json(&input)), None, false).unwrap();
    browser::draw(&mut surface, &forced, &mut row);
    check("document.querySelectorAll('.maestro-code').length === 1");
    maestro_web::set_show_json_mode(false);
    let custom = render_tool::<Value, Value>("route", None, None, false).unwrap();
    browser::draw(&mut surface, &custom, &mut row);
    check(
        "document.querySelectorAll('.maestro-code, .maestro-tool-caption, .maestro-tool-label').length === 0",
    );
    super::reset();
}
