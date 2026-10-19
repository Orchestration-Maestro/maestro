//! Direct diff properties, local overflow and native retained selection.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    markdown_support::{Surface, check, js},
    tool_renderer_browser as drawing,
};
use maestro_web::{
    components::diff::{DiffProps, diff},
    tools::types::{ToolRenderResult, ToolRenderState},
};

fn recipe(old_text: String, new_text: String) -> ToolRenderResult {
    ToolRenderResult {
        content: diff(DiffProps {
            old_text,
            new_text,
            title: Some("Caller <&> 雪".into()),
            class_name: "custom-token".into(),
        }),
        is_custom: true,
    }
}
/// Exercise all direct-call rows, then real local scrolling and retained edits.
/// # Panics
/// Panics when title, count colors, selection or native identity differs.
pub fn run() {
    super::browser::diffs();
    let mut surface = Surface::new();
    surface.pack = surface.alternate.clone();
    let old = "context\n".repeat(100);
    let new = format!("{old}added\n");
    let recipe = recipe(old.clone(), new);
    let mut state = ToolRenderState::default();
    drawing::draw(&mut surface, &recipe, &mut state);
    drawing::draw(&mut surface, &recipe, &mut state);
    check("document.querySelector('.maestro-diff-counts').children.length===3");
    check("document.querySelector('.maestro-diff-counts').textContent==='+1 / -0'");
    check(
        "(()=>{const n=document.querySelector('.maestro-diff-counts');return getComputedStyle(n.children[0]).color!==getComputedStyle(n).color && getComputedStyle(n.children[2]).color!==getComputedStyle(n).color && getComputedStyle(n.children[0]).color!==getComputedStyle(n.children[2]).color})()",
    );
    check(
        "(()=>{const n=document.querySelector('.maestro-diff-scroll');return n.clientHeight===384 && n.scrollHeight>n.clientHeight && getComputedStyle(n).overflow==='auto'})()",
    );
    let _ = js(
        "globalThis.artifactDiff=document.querySelector('.maestro-diff');globalThis.artifactScroll=artifactDiff.querySelector('.maestro-diff-scroll');globalThis.artifactLine=artifactScroll.querySelector('pre');artifactScroll.scrollTop=80;const range=document.createRange();range.selectNodeContents(artifactLine);getSelection().removeAllRanges();getSelection().addRange(range)",
    );
    drawing::draw(&mut surface, &recipe, &mut state);
    check(
        "artifactDiff===document.querySelector('.maestro-diff') && artifactScroll.scrollTop===80 && getSelection().toString()==='  context'",
    );
    let changed = self::recipe(old.clone(), format!("{old}changed\n"));
    drawing::draw(&mut surface, &changed, &mut state);
    drawing::draw(&mut surface, &changed, &mut state);
    check(
        "artifactDiff===document.querySelector('.maestro-diff') && artifactScroll===document.querySelector('.maestro-diff-scroll') && artifactLine===artifactScroll.querySelector('pre') && artifactScroll.scrollTop===80 && getSelection().toString()==='  context'",
    );
    check("artifactScroll.lastElementChild.textContent==='+ changed'");
    drop(state);
    check("!artifactDiff.isConnected && document.querySelector('.maestro-diff')===null");
}
