//! Native body identity survives updates but not disappearance or final release.
#![cfg(test)]
use super::disclosure::{Child, draw};
use crate::{
    markdown_support::{Surface, check, js},
    tool_renderer_browser as browser,
};
use maestro_app::JsonObject;
use maestro_web::{
    BashRenderer, CalculateRenderer, ExpandableSection, ToolRenderer,
    tools::types::{ToolRenderState, ToolResultMessageView},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub fn lifetimes() {
    let mut surface = Surface::new();
    let mut state = ToolRenderState::default();
    console(&mut surface, &mut state);
    error(&mut surface, &mut state);
    let replacement = crate::recipes::content("replacement", true);
    browser::draw(&mut surface, &replacement, &mut state);
    check("!savedCaption.isConnected");
    drop(state);
    release_child(&mut surface);
}

fn console(surface: &mut Surface, state: &mut ToolRenderState) {
    let mut args: JsonObject =
        serde_json::from_value(serde_json::json!({"command":"first"})).unwrap();
    let recipe = <BashRenderer as ToolRenderer<JsonObject, ()>>::render(
        &BashRenderer,
        Some(&args),
        None,
        false,
    )
    .unwrap();
    args.insert("command".into(), serde_json::json!("second"));
    browser::draw(surface, &recipe, state);
    check("document.querySelector('.maestro-console pre').textContent==='> first'");
    let _ = js(
        "globalThis.savedConsole=document.querySelector('.maestro-console'); globalThis.savedCaption=document.querySelector('.maestro-tool-caption')",
    );
    let recipe = <BashRenderer as ToolRenderer<JsonObject, ()>>::render(
        &BashRenderer,
        Some(&args),
        None,
        false,
    )
    .unwrap();
    browser::draw(surface, &recipe, state);
    check(
        "document.querySelector('.maestro-console')===savedConsole && document.querySelector('.maestro-console pre').textContent==='> second'",
    );
    let recipe =
        <BashRenderer as ToolRenderer<JsonObject, ()>>::render(&BashRenderer, None, None, false)
            .unwrap();
    browser::draw(surface, &recipe, state);
    check(
        "!savedConsole.isConnected && document.querySelector('.maestro-tool-caption')===savedCaption",
    );
}
fn error(surface: &mut Surface, state: &mut ToolRenderState) {
    let mut args: JsonObject =
        serde_json::from_value(serde_json::json!({"expression":"literal expression"})).unwrap();
    let mut result = crate::recipes::message::<()>(None);
    let recipe = CalculateRenderer
        .render(
            Some(&args),
            Some(ToolResultMessageView::from(&result)),
            false,
        )
        .unwrap();
    browser::draw(surface, &recipe, state);
    check(
        "!savedCaption.isConnected && document.querySelector('.maestro-tool-caption').textContent==='literal expression' && document.querySelector('.maestro-message-error').textContent==='one\\ntwo'",
    );
    let _ = js(
        "globalThis.savedError=document.querySelector('.maestro-message-error'); globalThis.savedErrorCaption=document.querySelector('.maestro-tool-caption')",
    );
    args.insert("expression".into(), serde_json::json!("changed expression"));
    let recipe = CalculateRenderer
        .render(Some(&args), Some((&result).into()), false)
        .unwrap();
    browser::draw(surface, &recipe, state);
    check(
        "document.querySelector('.maestro-message-error')===savedError && document.querySelector('.maestro-tool-caption')===savedErrorCaption && savedErrorCaption.textContent==='changed expression'",
    );
    result.is_error = false;
    let recipe = CalculateRenderer
        .render(Some(&args), Some((&result).into()), false)
        .unwrap();
    browser::draw(surface, &recipe, state);
    check(
        "!savedError.isConnected && !savedErrorCaption.isConnected && document.querySelector('.maestro-tool-caption').textContent==='changed expression = one\\ntwo'",
    );
    let _ = js("globalThis.savedCaption=document.querySelector('.maestro-tool-caption')");
    browser::draw(surface, &recipe, state);
    check("document.querySelector('.maestro-tool-caption')===savedCaption");
}
fn release_child(surface: &mut Surface) {
    let calls = Rc::new(Cell::new(0));
    let value = Rc::new(RefCell::new("captured".into()));
    let released = Rc::downgrade(&value);
    let mut section =
        ExpandableSection::new(surface.host(), vec![Box::new(Child { calls, value })]);
    section.summary = "release".into();
    section.default_expanded = true;
    draw(surface, &mut section);
    let _ = js(
        "globalThis.savedChild=document.querySelector('.maestro-console'); globalThis.savedSummary=document.querySelector('.maestro-text-action')",
    );
    super::disclosure::toggle(surface, &mut section);
    check("!savedChild.isConnected");
    assert!(released.upgrade().is_some());
    drop(section);
    assert!(
        released.upgrade().is_none(),
        "containing owner retained captured children"
    );
    check("!savedSummary.isConnected");
    let _ = js("savedSummary.click()");
    check("document.querySelectorAll('.maestro-console,.maestro-tool-caption').length===0");
}
