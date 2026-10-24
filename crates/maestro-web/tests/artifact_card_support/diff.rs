//! Direct diff properties, local overflow and native retained selection.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    markdown_support::{Surface, check, js},
    tool_renderer_browser as drawing,
};
use maestro_web::{
    components::changes::{DiffProps, diff},
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
    reconcile_titles(&mut surface, &old, &mut state);
    failed_reconciliation(&mut surface, &old, &mut state);
}

/// Keep unchanged rows, local offsets and the native selection through reconciliation.
fn retained() {
    check(
        "artifactDiff===document.querySelector('.maestro-diff') && artifactScroll===document.querySelector('.maestro-diff-scroll') && artifactLine===artifactScroll.querySelector('pre') && artifactScroll.scrollTop===80 && getSelection().toString()==='  context'",
    );
}
/// Prepare a direct caller's optional title without changing its text operands.
fn titled(old_text: String, new_text: String, title: Option<&str>) -> ToolRenderResult {
    ToolRenderResult {
        content: diff(DiffProps {
            old_text,
            new_text,
            title: title.map(str::to_owned),
            class_name: "custom-token".into(),
        }),
        is_custom: true,
    }
}

/// Retain rows and selection through each optional title transition.
fn reconcile_titles(surface: &mut Surface, old: &str, state: &mut ToolRenderState) {
    for (text, title) in [
        (format!("{old}changed\nextra\n"), Some("Renamed 雪")),
        (format!("{old}shrunk\n"), None),
        (format!("{old}grown\nagain\n"), Some("Restored <&>")),
        (format!("{old}last\n"), Some("")),
    ] {
        let next = titled(old.to_owned(), text, title);
        drawing::draw(surface, &next, state);
        drawing::draw(surface, &next, state);
        retained();
        check(&format!(
            "artifactScroll.children.length==={}",
            if title == Some("Restored <&>") || title == Some("Renamed 雪") {
                102
            } else {
                101
            }
        ));
        if let Some(title) = title.filter(|title| !title.is_empty()) {
            check(&format!(
                "artifactDiff.querySelector('.maestro-diff-header').firstChild.textContent==={}",
                serde_json::to_string(title).unwrap()
            ));
        } else {
            check("artifactDiff.querySelector('.maestro-diff-header')===null");
        }
    }
}

/// Keep the existing failed update and recovery on the retained native diff.
fn failed_reconciliation(surface: &mut Surface, old: &str, state: &mut ToolRenderState) {
    let _ = js(
        "globalThis.diffBeforeFailure=artifactDiff.outerHTML;globalThis.diffFrame=artifactDiff.parentElement.parentElement;globalThis.diffFrameStyle=diffFrame.getAttribute('style');artifactScroll.appendChild=()=>{throw new Error('forced diff append failure')}",
    );
    let failed = titled(
        old.to_owned(),
        format!("{old}failure\nextra\n"),
        Some("Must roll back"),
    );
    let host = surface.host();
    let result = surface.draw(|ui, frame| {
        failed
            .render(ui, drawing::view(&host, frame), state)
            .response
    });
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("forced diff append failure")
    );
    retained();
    check(
        "artifactDiff.outerHTML===diffBeforeFailure && diffFrame.getAttribute('style')===diffFrameStyle && artifactDiff.querySelector('.maestro-diff-header')===null && document.querySelectorAll('.maestro-diff-header').length===0",
    );
    let _ = js("delete artifactScroll.appendChild");
    drawing::draw(surface, &failed, state);
    drawing::draw(surface, &failed, state);
    retained();
    check(
        "artifactScroll.children.length===102 && artifactDiff.querySelector('.maestro-diff-header').firstChild.textContent==='Must roll back'",
    );
    drop(std::mem::take(state));
    check("!artifactDiff.isConnected && document.querySelector('.maestro-diff')===null");
}
