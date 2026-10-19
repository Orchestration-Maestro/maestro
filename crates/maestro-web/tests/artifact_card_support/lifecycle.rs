//! Retained artifact templates through the shared disclosure and native lifetimes.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    markdown_support::{Surface, check, js, publish},
    tool_renderer_browser as drawing,
};
use maestro_web::{
    ArtifactsToolRenderer, ToolRenderer,
    tools::types::{ToolRenderResult, ToolRenderState},
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Transition {
    input: Pair,
    expected: Expected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pair {
    from: Template,
    to: Template,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Template {
    command: Option<String>,
    phase: String,
    present: Option<bool>,
    streaming: Option<bool>,
    content: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Expected {
    header_retained: bool,
    body_retained: Option<bool>,
    collapsed: Option<bool>,
    code_retained: Option<bool>,
}
fn prepare(template: &Template) -> ToolRenderResult {
    let params = template.command.as_ref().map_or_else(
        || template.present.unwrap_or(false).then(|| json!({})),
        |command| Some(json!({"command":command,"filename":"a.rs","content":template.content.as_deref().unwrap_or("literal"),"old_str":"old","new_str":"new"})),
    );
    let mut result = super::phase(&template.phase);
    if let Some(result) = &mut result {
        result
            .content
            .push(maestro_app::UserBlock::Text(maestro_app::TextContent {
                text: "R".into(),
                text_signature: None,
            }));
    }
    <ArtifactsToolRenderer as ToolRenderer<Value, Value>>::render(
        &ArtifactsToolRenderer::new(None),
        params.as_ref(),
        result.as_ref().map(Into::into),
        template.streaming.unwrap_or(false),
    )
    .unwrap()
}
/// Observe every recorded template transition and its continuously mounted children.
/// # Panics
/// Panics if template identity, focus or expansion differs from the reference.
pub fn templates() {
    let rows: Vec<Transition> =
        serde_json::from_str(include_str!("../fixtures/artifact_transitions.json")).unwrap();
    assert_eq!(rows.len(), 26);
    let mut surface = Surface::new();
    surface.time = Some(0.);
    for (index, row) in rows.into_iter().enumerate() {
        publish("artifactTransition", &index.to_string().into());
        let mut state = ToolRenderState::default();
        let before = prepare(&row.input.from);
        drawing::draw(&mut surface, &before, &mut state);
        drawing::draw(&mut surface, &before, &mut state);
        let _ = js(
            "globalThis.artifactBeforeCaption=document.querySelector('.maestro-tool-caption');globalThis.artifactBeforeCode=document.querySelector('.maestro-code')",
        );
        let before_collapsible = matches!(
            row.input.from.command.as_deref(),
            Some("create" | "rewrite" | "update" | "get" | "logs")
        ) && !(row.input.from.phase == "error"
            && matches!(row.input.from.command.as_deref(), Some("get" | "logs")));
        if before_collapsible {
            let at = drawing::glyphs(&surface)[0].center();
            drawing::click(&mut surface, at, &before, &mut state);
            surface.time = Some(surface.time.unwrap() + 0.3);
            drawing::draw(&mut surface, &before, &mut state);
        }
        let focused = surface.context.memory(|m| m.focused());
        let after = prepare(&row.input.to);
        drawing::draw(&mut surface, &after, &mut state);
        let response = drawing::draw(&mut surface, &after, &mut state);
        assert_eq!(
            js("artifactBeforeCaption===document.querySelector('.maestro-tool-caption')").as_bool(),
            Some(row.expected.header_retained),
            "transition {index}"
        );
        if let Some(expected) = row.expected.code_retained {
            assert_eq!(
                js("artifactBeforeCode===document.querySelector('.maestro-code')").as_bool(),
                Some(expected),
                "transition {index}"
            );
        }
        if let Some(collapsed) = row.expected.collapsed {
            let header_height = drawing::glyphs(&surface)[0].center().y * 2.;
            assert_eq!(
                response.rect.height() > header_height + 5.,
                !collapsed,
                "transition {index}: height {} header {header_height}",
                response.rect.height()
            );
        }
        if let Some(retained) = row.expected.body_retained {
            assert_eq!(
                surface.context.memory(|m| m.focused()) == focused,
                retained,
                "transition {index}"
            );
        }
        drop(state);
        check(
            "!artifactBeforeCaption.isConnected && document.querySelector('.maestro-tool-caption')===null",
        );
    }
}

/// Reversal samples exercise the shared transition through artifact content.
/// # Panics
/// Panics if measured height, margin or native lifetime changes.
pub fn reversals() {
    let template = Template {
        command: Some("create".into()),
        phase: "pending".into(),
        present: None,
        streaming: None,
        content: Some("line\n".repeat(100)),
    };
    let recipe = prepare(&template);
    let mut surface = Surface::new();
    surface.time = Some(0.);
    let mut state = ToolRenderState::default();
    let header = drawing::draw(&mut surface, &recipe, &mut state)
        .rect
        .height();
    let _ = js("globalThis.artifactTransitionCode=document.querySelector('.maestro-code')");
    let at = drawing::glyphs(&surface)[0].center();
    drawing::click(&mut surface, at, &recipe, &mut state);
    for (time, height, margin) in [(0., 0., 0.), (0.075, 426., 2.83905), (0.15, 426., 9.30674)] {
        surface.time = Some(time);
        let response = drawing::draw(&mut surface, &recipe, &mut state);
        assert!(
            (response.rect.height() - header - height - margin).abs() < 0.03,
            "entry {time}"
        );
        check("artifactTransitionCode===document.querySelector('.maestro-code')");
    }
    drawing::click(&mut surface, at, &recipe, &mut state);
    for (time, height, margin) in [(0.20, 426., 7.81032), (0.25, 426., 3.06921), (0.45, 0., 0.)] {
        surface.time = Some(time);
        let response = drawing::draw(&mut surface, &recipe, &mut state);
        assert!(
            (response.rect.height() - header - height - margin).abs() < 0.03,
            "reverse {time}"
        );
        check("artifactTransitionCode===document.querySelector('.maestro-code')");
    }
    drawing::click(&mut surface, at, &recipe, &mut state);
    surface.time = Some(0.75);
    let response = drawing::draw(&mut surface, &recipe, &mut state);
    assert!((response.rect.height() - header - 438.).abs() < 0.03);
    check("artifactTransitionCode===document.querySelector('.maestro-code')");
}

/// Independent rows release only disappearing code, console and diff owners.
/// # Panics
/// Panics if a surviving row loses its roots, selection or scroll state.
pub fn release() {
    use crate::support::{reading_view::view, tool_output::TextOnlyOutput as _};
    let mut surface = Surface::new();
    surface.time = Some(0.);
    let host = surface.host();
    let template = Template {
        command: Some("create".into()),
        phase: "success".into(),
        present: None,
        streaming: None,
        content: Some("literal".into()),
    };
    let first = prepare(&template);
    let second = prepare(&Template {
        command: Some("update".into()),
        ..template
    });
    let mut states = [ToolRenderState::default(), ToolRenderState::default()];
    {
        let mut draw = |surface: &mut Surface| {
            surface.draw(|ui, frame| {
                let first = first
                    .render(ui, view(&host, frame), &mut states[0])
                    .assert_no_overlays()
                    .unwrap();
                let second = second
                    .render(ui, view(&host, frame), &mut states[1])
                    .assert_no_overlays()
                    .unwrap();
                [first, second]
            })
        };
        draw(&mut surface);
        draw(&mut surface);
        let _ = js(
            "globalThis.artifactFirstCode=document.querySelector('.maestro-code');globalThis.artifactSecondDiff=document.querySelector('.maestro-diff');document.querySelectorAll('.maestro-tool-caption')[0].click()",
        );
        draw(&mut surface);
        surface.time = Some(0.3);
        let rows = draw(&mut surface);
        let header_height = drawing::glyphs(&surface)[0].center().y * 2.;
        assert!(rows[0].rect.height() > header_height + 12.);
        assert!((rows[1].rect.height() - header_height).abs() < 0.03);
        let _ = js("document.querySelectorAll('.maestro-tool-caption')[1].click()");
        draw(&mut surface);
        surface.time = Some(0.6);
        draw(&mut surface);
        check(
            "artifactFirstCode===document.querySelector('.maestro-code') && artifactSecondDiff===document.querySelector('.maestro-diff')",
        );
        let _ = js(
            "const range=document.createRange();range.selectNodeContents(artifactSecondDiff.querySelector('pre'));getSelection().removeAllRanges();getSelection().addRange(range);globalThis.artifactKeptSelection=getSelection().toString()",
        );
        assert!(!js("artifactKeptSelection").as_string().unwrap().is_empty());
    }
    drop(std::mem::take(&mut states[0]));
    check(
        "!artifactFirstCode.isConnected && artifactSecondDiff.isConnected && getSelection().toString()===artifactKeptSelection",
    );
    drawing::draw(&mut surface, &second, &mut states[1]);
    check(
        "artifactSecondDiff===document.querySelector('.maestro-diff') && getSelection().toString()===artifactKeptSelection",
    );
    drop(states);
    check("!artifactSecondDiff.isConnected && document.querySelector('.maestro-diff')===null");
    console_release(&mut surface);
}

fn console_release(surface: &mut Surface) {
    let params = json!({"command":"create","filename":"a.html","content":"listing"});
    let mut result = super::phase("success").unwrap();
    result
        .content
        .push(maestro_app::UserBlock::Text(maestro_app::TextContent {
            text: "log\n".repeat(100),
            text_signature: None,
        }));
    let prepare = |params: &Value| {
        <ArtifactsToolRenderer as ToolRenderer<Value, Value>>::render(
            &ArtifactsToolRenderer::new(None),
            Some(params),
            Some((&result).into()),
            false,
        )
        .unwrap()
    };
    let recipe = prepare(&params);
    let mut state = ToolRenderState::default();
    drawing::draw(surface, &recipe, &mut state);
    crate::document_card_support::browser::open(surface, &recipe, &mut state);
    let _ = js(
        "globalThis.artifactConsole=document.querySelector('.maestro-console');globalThis.artifactCode=document.querySelector('.maestro-code');artifactConsole.querySelector('.console-scroll').scrollTop=20",
    );
    drawing::draw(surface, &recipe, &mut state);
    check(
        "artifactConsole===document.querySelector('.maestro-console') && artifactConsole.querySelector('.console-scroll').scrollTop===20",
    );
    let mut params = params;
    params["content"] = "".into();
    let changed = prepare(&params);
    drawing::draw(surface, &changed, &mut state);
    check(
        "!artifactCode.isConnected && artifactConsole===document.querySelector('.maestro-console') && artifactConsole.querySelector('.console-scroll').scrollTop===20",
    );
    let _ = js("document.querySelector('.maestro-tool-caption').click()");
    drawing::draw(surface, &changed, &mut state);
    surface.time = Some(surface.time.unwrap() + 0.3);
    drawing::draw(surface, &changed, &mut state);
    check("artifactConsole===document.querySelector('.maestro-console')");
    let _ = js("document.querySelector('.maestro-tool-caption').click()");
    drawing::draw(surface, &changed, &mut state);
    surface.time = Some(surface.time.unwrap() + 0.3);
    drawing::draw(surface, &changed, &mut state);
    check(
        "artifactConsole===document.querySelector('.maestro-console') && artifactConsole.querySelector('.console-scroll').scrollTop===20",
    );
    drop(state);
    check("!artifactConsole.isConnected && document.querySelector('.maestro-console')===null");
}

/// Plain status changes reset every shared header caller, not their sibling bodies.
/// # Panics
/// Panics if a plain caption survives a status change or a collapsible one does not.
pub fn shared_headers() {
    use maestro_web::tools::renderer_registry::{ToolHeaderState, ToolHeaderText};
    use maestro_web::{
        CalculateRenderer, DefaultRenderer, render_collapsible_header, render_header,
    };
    let mut surface = Surface::new();
    let params: maestro_app::JsonObject =
        serde_json::from_value(json!({"expression":"1+1"})).unwrap();
    let json_params = Value::Object(params.clone());
    let success = super::phase("success").unwrap();
    let failure = super::phase("error").unwrap();
    let plain = |status, rich| ToolRenderResult {
        content: render_header(
            status,
            maestro_web::icons::icons()
                .unwrap()
                .shift_remove("Code")
                .unwrap(),
            if rich {
                ToolHeaderText::Content(maestro_web::artifact_pill("SDK.rs".into(), None))
            } else {
                ToolHeaderText::Text("SDK caption".into())
            },
        ),
        is_custom: false,
    };
    let pairs = [
        [
            DefaultRenderer
                .render(Some(&json_params), Some((&success).into()), false)
                .unwrap(),
            DefaultRenderer
                .render(Some(&json_params), Some((&failure).into()), false)
                .unwrap(),
        ],
        [
            CalculateRenderer
                .render(Some(&params), Some((&success).into()), false)
                .unwrap(),
            CalculateRenderer
                .render(Some(&params), Some((&failure).into()), false)
                .unwrap(),
        ],
        [
            plain(ToolHeaderState::Complete, false),
            plain(ToolHeaderState::Error, false),
        ],
        [
            plain(ToolHeaderState::Complete, true),
            plain(ToolHeaderState::Error, true),
        ],
    ];
    for pair in pairs {
        let mut state = ToolRenderState::default();
        drawing::draw(&mut surface, &pair[0], &mut state);
        drawing::draw(&mut surface, &pair[0], &mut state);
        let _ = js(
            "globalThis.artifactPlainCaption=document.querySelector('.maestro-tool-caption,.maestro-artifact-pill')",
        );
        drawing::draw(&mut surface, &pair[1], &mut state);
        drawing::draw(&mut surface, &pair[1], &mut state);
        check(
            "!artifactPlainCaption.isConnected && artifactPlainCaption!==document.querySelector('.maestro-tool-caption,.maestro-artifact-pill')",
        );
        assert!(surface.context.memory(|m| m.focused()).is_none());
    }
    let expanded = std::rc::Rc::new(std::cell::Cell::new(false));
    let mut state = ToolRenderState::default();
    for status in [
        ToolHeaderState::Inprogress,
        ToolHeaderState::Complete,
        ToolHeaderState::Error,
    ] {
        let recipe = ToolRenderResult {
            content: render_collapsible_header(
                status,
                maestro_web::icons::icons()
                    .unwrap()
                    .shift_remove("Code")
                    .unwrap(),
                ToolHeaderText::Text("retained SDK caption".into()),
                expanded.clone(),
            ),
            is_custom: false,
        };
        drawing::draw(&mut surface, &recipe, &mut state);
        if status == ToolHeaderState::Inprogress {
            let at = drawing::glyphs(&surface)[1].center();
            drawing::click(&mut surface, at, &recipe, &mut state);
            let _ =
                js("globalThis.artifactSDKCaption=document.querySelector('.maestro-tool-caption')");
        }
        check("artifactSDKCaption===document.querySelector('.maestro-tool-caption')");
        assert!(expanded.get());
        assert!(surface.context.memory(|m| m.focused()).is_some());
    }
}

/// Copy feedback survives collapse/reopen and ends only with its disappearing child.
/// # Panics
/// Panics if feedback, native roots or original copy bytes are lost.
pub async fn copy_feedback() {
    let mut surface = Surface::new();
    surface.time = Some(0.);
    let params = json!({"command":"create","filename":"feedback.html","content":"literal code"});
    let mut result = super::phase("success").unwrap();
    result
        .content
        .push(maestro_app::UserBlock::Text(maestro_app::TextContent {
            text: "literal logs".into(),
            text_signature: None,
        }));
    let recipe = <ArtifactsToolRenderer as ToolRenderer<Value, Value>>::render(
        &ArtifactsToolRenderer::new(None),
        Some(&params),
        Some((&result).into()),
        false,
    )
    .unwrap();
    let mut state = ToolRenderState::default();
    drawing::draw(&mut surface, &recipe, &mut state);
    crate::document_card_support::browser::open(&mut surface, &recipe, &mut state);
    let _ = js(
        "globalThis.feedbackCode=document.querySelector('.maestro-code');globalThis.feedbackConsole=document.querySelector('.maestro-console')",
    );
    for index in 0..2 {
        let at = crate::markdown_support::slot_center(index);
        drawing::click(&mut surface, at, &recipe, &mut state);
        crate::markdown_support::settle().await;
    }
    drawing::draw(&mut surface, &recipe, &mut state);
    assert_eq!(
        *surface.effects.copies.borrow(),
        ["literal code", "literal logs"]
    );
    assert!(surface.painted.iter().any(|(text, _)| text == "Copied!"));
    let live = || {
        surface
            .effects
            .timers
            .borrow()
            .iter()
            .filter(|timer| timer.get())
            .count()
    };
    assert_eq!(live(), 2);
    let _ = js("document.querySelector('.maestro-tool-caption').click()");
    drawing::draw(&mut surface, &recipe, &mut state);
    surface.time = Some(surface.time.unwrap() + 0.3);
    drawing::draw(&mut surface, &recipe, &mut state);
    assert!(!surface.painted.iter().any(|(text, _)| text == "Copied!"));
    let _ = js("document.querySelector('.maestro-tool-caption').click()");
    drawing::draw(&mut surface, &recipe, &mut state);
    surface.time = Some(surface.time.unwrap() + 0.3);
    drawing::draw(&mut surface, &recipe, &mut state);
    check(
        "feedbackCode===document.querySelector('.maestro-code') && feedbackConsole===document.querySelector('.maestro-console')",
    );
    assert!(surface.painted.iter().any(|(text, _)| text == "Copied!"));
    assert_eq!(
        surface
            .effects
            .timers
            .borrow()
            .iter()
            .filter(|timer| timer.get())
            .count(),
        2
    );
    let mut params = params;
    params["content"] = "".into();
    let changed = <ArtifactsToolRenderer as ToolRenderer<Value, Value>>::render(
        &ArtifactsToolRenderer::new(None),
        Some(&params),
        Some((&result).into()),
        false,
    )
    .unwrap();
    drawing::draw(&mut surface, &changed, &mut state);
    check(
        "!feedbackCode.isConnected && feedbackConsole===document.querySelector('.maestro-console')",
    );
    assert_eq!(
        surface
            .effects
            .timers
            .borrow()
            .iter()
            .filter(|timer| timer.get())
            .count(),
        1
    );
    assert!(surface.painted.iter().any(|(text, _)| text == "Copied!"));
    drop(state);
    check("!feedbackConsole.isConnected");
    assert!(
        surface
            .effects
            .timers
            .borrow()
            .iter()
            .all(|timer| !timer.get())
    );
}
