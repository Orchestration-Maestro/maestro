//! Real native text and final-clip observations for document cards.
#![cfg(all(test, target_arch = "wasm32"))]
use super::{Row, fixture, prepare};
use crate::{
    markdown_support::{Surface, check, js, publish},
    tool_renderer_browser as drawing,
};
use maestro_web::tools::types::{ToolRenderResult, ToolRenderState};

pub fn rows(select: impl Fn(&Row) -> bool) {
    let mut surface = Surface::new();
    let rows: Vec<_> = fixture().into_iter().filter(select).collect();
    assert!(!rows.is_empty());
    for row in rows {
        if let Some(language) = &row.input.language {
            surface.i18n.set_language(language).unwrap();
        }
        observe(&mut surface, &row);
    }
}
pub fn open(surface: &mut Surface, recipe: &ToolRenderResult, state: &mut ToolRenderState) {
    let at = drawing::glyphs(surface)[0].center();
    drawing::click(surface, at, recipe, state);
    surface.time = Some(surface.time.unwrap_or(0.) + 0.3);
    drawing::draw(surface, recipe, state);
}
pub fn observe(surface: &mut Surface, row: &Row) {
    surface.time.get_or_insert(0.);
    let prepared = prepare(row);
    let Some(expected) = &row.expected else {
        assert!(prepared.is_err(), "{}", row.id);
        return;
    };
    let recipe = prepared.unwrap();
    assert!(!recipe.is_custom);
    let mut state = ToolRenderState::default();
    drawing::draw(surface, &recipe, &mut state);
    drawing::draw(surface, &recipe, &mut state);
    publish(
        "documentExpected",
        &serde_json::to_string(expected).unwrap().into(),
    );
    assert_eq!(
        js("document.querySelector('.maestro-tool-caption').textContent")
            .as_string()
            .unwrap(),
        expected.caption,
        "{}",
        row.id
    );
    let glyphs = drawing::glyphs(surface);
    let header = if expected.collapsible {
        2 + expected.spinner
    } else {
        1 + expected.spinner
    };
    if expected.attachments.is_empty() {
        assert_eq!(glyphs.len(), header);
    } else {
        assert!(glyphs.len() >= header);
    }
    let pack = surface.pack.clone();
    let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
    drawing::has_role(
        surface,
        glyphs[0],
        match expected.status.as_str() {
            "complete" => "success",
            "error" => "error",
            "inprogress" => "text",
            _ => panic!("unknown status"),
        },
        &presentation,
    );
    if expected.collapsible {
        open(surface, &recipe, &mut state);
    }
    observe_body();
    drop(state);
    check(
        "document.querySelectorAll('.maestro-tool-caption,.maestro-tool-url,.maestro-console,.maestro-code').length===0",
    );
}
fn observe_body() {
    check(
        "JSON.stringify([...document.querySelectorAll('.maestro-code code')].map(n=>({code:n.textContent,language:[...n.classList].find(c=>c.startsWith('language-')).slice(9)})))===JSON.stringify(JSON.parse(documentExpected).blocks)",
    );
    check(
        "JSON.stringify([...document.querySelectorAll('.maestro-console pre')].map(n=>({content:n.textContent,variant:n.classList.contains('maestro-console-error')?'error':'default'})))===JSON.stringify(JSON.parse(documentExpected).console)",
    );
    check(
        "(document.querySelector('.maestro-tool-url')?.textContent??null)===JSON.parse(documentExpected).url",
    );
    check(
        "document.querySelectorAll('.maestro-tool-caption script,.maestro-code script,.maestro-tool-url script').length===0",
    );
}
