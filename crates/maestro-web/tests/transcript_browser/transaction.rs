//! Widget-derived native apertures and returned follow-frame paint.
use super::aggregate::{Language, Renderer, draw_frame, draw_frame_with_overlays};
use crate::reading_slots_support::{Slots, js};
use maestro_app::browser::components::messages::{
    CustomAgentMessages as BrowserMessages, UserMessageWithAttachments,
};
use maestro_app::{Agent, AgentMessage, NoCustomMessages};
use maestro_web::{AgentInterface, I18n};
use num_traits::ToPrimitive as _;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::prelude::*;

/// The public viewport used by the real aggregate witnesses.
fn viewport() -> egui::Rect {
    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 180.))
}
/// Both attachment branches register actual click records, and native hit testing exposes them.
/// # Panics
/// Panics when a tile lacks a clickable widget or its native ancestor captures the pointer.
#[wasm_bindgen]
pub async fn transcript_widget_apertures_open_preview_and_fallback_tiles() {
    let scene = Slots::new();
    let i18n = I18n::new(Rc::new(Language)).unwrap();
    let agent = tile_session();
    let mut interface = AgentInterface::new(scene.host.clone());
    interface.set_auto_scroll(false);
    interface.set_session(Some(agent)).unwrap();
    draw_frame(&scene, &mut interface, &i18n, viewport(), vec![])
        .unwrap()
        .textures_delta
        .clear();
    let tiles = scene.context.viewport(|v| {
        v.prev_pass
            .widgets
            .layers()
            .flat_map(|(_, widgets)| widgets.iter())
            .filter(|widget| {
                widget.sense.senses_click() && widget.rect.size() == egui::vec2(64., 64.)
            })
            .map(|widget| widget.rect.center())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        tiles.len(),
        2,
        "both production tile branches are recorded by egui"
    );
    let saved = super::pointer::capture();
    for point in tiles {
        js(
            &JsValue::NULL,
            &format!(
                "globalThis.transcriptTarget=[{},{}];globalThis.transcriptPointers=[]",
                point.x, point.y
            ),
        );
        assert_eq!(js(&JsValue::NULL, "return document.elementFromPoint(...transcriptTarget)===document.getElementById('canvas')").as_bool(), Some(true), "recorded tile must punch through the native viewport");
        super::pointer::click().await;
        let mut opened = vec![];
        for events in super::pointer::events() {
            let (mut output, overlays) =
                draw_frame_with_overlays(&scene, &mut interface, &i18n, viewport(), events)
                    .unwrap();
            output.textures_delta.clear();
            opened.extend(overlays);
        }
        assert_eq!(opened.len(), 1, "real tile gesture opens one overlay");
    }
    super::pointer::release(&saved);
    assert!(js(&JsValue::NULL, "const p=document.querySelector('.maestro-transcript p'),r=p.getBoundingClientRect();return document.elementFromPoint(r.x+4,r.y+r.height/2)!==document.getElementById('canvas')").as_bool().unwrap(), "non-clickable text remains native");
}

/// Returned tessellation aligns copy paint to the final native slot, without a warm-up frame.
/// # Panics
/// Panics when automatic following moves native content without its canvas controls.
#[wasm_bindgen]
pub async fn transcript_follow_commits_initial_paint_at_the_native_bottom() {
    let (scene, mut interface, i18n, mut output) = initial_follow_frame();
    let point = assert_follow_frame(
        &scene,
        &mut output,
        &mut crate::canvas_paint::Paint::default(),
    );
    let saved = super::pointer::capture();
    js(
        &point,
        "globalThis.transcriptTarget=[...value];globalThis.transcriptHit=document.elementFromPoint(...transcriptTarget)===document.getElementById('canvas')",
    );
    assert_eq!(
        js(&JsValue::NULL, "return transcriptHit").as_bool(),
        Some(true)
    );
    super::pointer::click().await;
    super::pointer::deliver_pointer(&scene, &mut interface, &i18n);
    assert_eq!(
        js(&JsValue::NULL, "return transcriptCopies.join('|')")
            .as_string()
            .as_deref(),
        Some("follow row 39\n"),
        "first input frame copies once without a warm-up draw"
    );
    super::pointer::release(&saved);
}

/// Real attachment-user input exercises both production tile branches.
fn tile_session() -> Rc<Agent<BrowserMessages<NoCustomMessages>>> {
    use maestro_app::browser::utils::attachment_utils::AttachmentType;
    let agent = Rc::new(Agent::<BrowserMessages<NoCustomMessages>>::default());
    let attachments = [true, false].map(|preview| maestro_web::Attachment {
        id: format!("tile-{preview}"),
        r#type: if preview {
            AttachmentType::Image
        } else {
            AttachmentType::Document
        },
        file_name: if preview {
            "preview.gif"
        } else {
            "fallback.txt"
        }
        .into(),
        mime_type: if preview { "image/gif" } else { "text/plain" }.into(),
        size: 1,
        content: "R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7".into(),
        extracted_text: None,
        preview: preview.then(|| "R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7".into()),
    });
    agent
        .state()
        .write()
        .unwrap()
        .set_messages(vec![AgentMessage::Custom(Arc::new(RwLock::new(
            BrowserMessages::UserWithAttachments(UserMessageWithAttachments {
                content: maestro_app::UserContent::Text("native selection region".into()),
                timestamp: 0.,
                attachments: Some(attachments.into()),
            }),
        )))]);
    agent
}

/// Check transformed click geometry and actual clipped paint against the native copy slot.
pub(crate) fn assert_follow_frame(
    scene: &Slots,
    output: &mut egui::FullOutput,
    paint: &mut crate::canvas_paint::Paint,
) -> js_sys::Array {
    let (coordinates, center) = copy_slot();
    assert!(
        viewport().contains(center),
        "native copy control follows to the bottom"
    );
    paint.update(output);
    let bounds = egui::Rect::from_center_size(center, egui::vec2(16., 16.));
    let ink = crate::canvas_paint::points(&scene.context, bounds)
        .into_iter()
        .zip(paint.region(&scene.context, &output.shapes, bounds))
        .filter(|(_, pixel)| pixel[3] > 0)
        .fold(egui::Rect::NOTHING, |rect, (position, _)| {
            rect.union(egui::Rect::from_min_max(position, position))
        });
    assert!(
        ink.is_finite(),
        "returned copy glyph must paint at its final native slot"
    );
    assert!(
        (ink.center().y - center.y).abs() <= 1.,
        "painted glyph/native vertical error must stay within one pixel: {ink:?} vs {center:?}"
    );
    output.textures_delta.clear();
    coordinates
}

/// Late geometry registration keeps focus order and keyboard activation through followed growth.
/// # Panics
/// Panics if repeated hit-rectangle registration changes Tab order or duplicates activation.
#[wasm_bindgen]
pub fn transcript_follow_geometry_preserves_tab_and_keyboard_activation() {
    let scene = Slots::new();
    let i18n = I18n::new(Rc::new(Language)).unwrap();
    maestro_web::register_message_renderer(
        "user",
        Rc::new(Renderer(Rc::new(RefCell::new(vec![])))),
    );
    let agent = code_session(2, |n| format!("```text\nkeyboard row {n}\n```"));
    let mut interface = AgentInterface::new(scene.host.clone());
    interface.set_session(Some(agent.clone())).unwrap();
    draw_frame(&scene, &mut interface, &i18n, viewport(), vec![])
        .unwrap()
        .textures_delta
        .clear();
    let ids = scene.context.viewport(|v| {
        v.prev_pass
            .widgets
            .layers()
            .flat_map(|(_, widgets)| widgets.iter())
            .filter(|w| w.sense.is_focusable())
            .map(|w| w.id)
            .collect::<Vec<_>>()
    });
    assert_eq!(ids.len(), 2);
    scene.context.memory_mut(|m| m.request_focus(ids[0]));
    agent
        .state()
        .read()
        .unwrap()
        .messages()
        .write()
        .unwrap()
        .push(AgentMessage::User(Arc::new(RwLock::new(
            maestro_app::UserMessage {
                content: maestro_app::UserContent::Text("```text\nkeyboard growth\n```".into()),
                timestamp: 0.,
            },
        ))));
    interface.set_session(Some(agent)).unwrap();
    key_frame(
        &scene,
        &mut interface,
        &i18n,
        egui::Key::Tab,
        egui::Modifiers::NONE,
    );
    assert_eq!(scene.context.memory(egui::Memory::focused), Some(ids[1]));
    keyboard_activation(&scene, &mut interface, &i18n);
    key_frame(
        &scene,
        &mut interface,
        &i18n,
        egui::Key::Tab,
        egui::Modifiers::SHIFT,
    );
    draw_frame(&scene, &mut interface, &i18n, viewport(), vec![])
        .unwrap()
        .textures_delta
        .clear();
    assert_eq!(scene.context.memory(egui::Memory::focused), Some(ids[0]));
}
/// Keep key down/up in one observed frame without a synthetic action-dispatch path.
fn key_frame(
    scene: &Slots,
    interface: &mut AgentInterface,
    i18n: &I18n,
    key: egui::Key,
    modifiers: egui::Modifiers,
) {
    let events = [true, false].map(|pressed| egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    });
    draw_frame(scene, interface, i18n, viewport(), events.into())
        .unwrap()
        .textures_delta
        .clear();
}

/// A recipe-owned ordinary toolkit button, with no native-control registration.
struct ButtonRecipe {
    /// Count invocations independently from action dispatch.
    draws: Rc<std::cell::Cell<usize>>,
    /// Count actual toolkit click responses.
    actions: Rc<std::cell::Cell<usize>>,
}
impl maestro_web::tools::types::ToolContent for ButtonRecipe {
    fn render(
        &self,
        ui: &mut egui::Ui,
        _: maestro_web::tools::types::ToolRenderView<'_>,
        _: &maestro_app::ToolRendererState,
    ) -> maestro_web::tools::types::ToolContentOutput {
        self.draws.set(self.draws.get() + 1);
        let response = ui.button("Recipe action");
        if response.clicked() {
            self.actions.set(self.actions.get() + 1);
        }
        ui.allocate_space(egui::vec2(20., 60.));
        maestro_web::tools::types::ToolContentOutput {
            response: Ok(response),
            overlays: vec![],
        }
    }
}
/// Return a fresh recipe while its counters remain with the caller.
struct ButtonRenderer(Rc<std::cell::Cell<usize>>, Rc<std::cell::Cell<usize>>);
impl maestro_web::MessageRenderer<BrowserMessages<NoCustomMessages>> for ButtonRenderer {
    fn render(
        &self,
        _: &AgentMessage<BrowserMessages<NoCustomMessages>>,
    ) -> Result<
        Box<dyn maestro_web::tools::types::ToolContent>,
        maestro_web::tools::types::ToolUiError,
    > {
        Ok(Box::new(ButtonRecipe {
            draws: self.0.clone(),
            actions: self.1.clone(),
        }))
    }
}
/// A new ordinary button receives final paint, a clipped native aperture and one real click.
/// # Panics
/// Panics if the transaction only handles registered reading controls or repeats a recipe.
#[wasm_bindgen]
pub async fn transcript_follow_commits_unregistered_recipe_button_geometry() {
    let scene = Slots::new();
    let i18n = I18n::new(Rc::new(Language)).unwrap();
    let draws = Rc::new(std::cell::Cell::new(0));
    let actions = Rc::new(std::cell::Cell::new(0));
    maestro_web::register_message_renderer(
        "user",
        Rc::new(ButtonRenderer(draws.clone(), actions.clone())),
    );
    let agent = code_session(3, |_| String::new());
    let mut interface = AgentInterface::new(scene.host.clone());
    interface.set_session(Some(agent)).unwrap();
    let mut output = draw_frame(&scene, &mut interface, &i18n, viewport(), vec![]).unwrap();
    assert_eq!(draws.get(), 3);
    let center = painted_button(&scene, &mut output);
    let saved = super::pointer::capture();
    js(
        &JsValue::NULL,
        &format!(
            "globalThis.transcriptTarget=[{},{}];globalThis.transcriptHit=document.elementFromPoint(...transcriptTarget)===document.getElementById('canvas')",
            center.x, center.y
        ),
    );
    assert_eq!(
        js(&JsValue::NULL, "return transcriptHit").as_bool(),
        Some(true)
    );
    super::pointer::click().await;
    for events in super::pointer::events() {
        draws.set(0);
        draw_frame(&scene, &mut interface, &i18n, viewport(), events)
            .unwrap()
            .textures_delta
            .clear();
        assert!(draws.get() <= 3, "no row is invoked twice");
    }
    assert_eq!(actions.get(), 1);
    super::pointer::release(&saved);
}

/// Build ordinary user history without requiring a second session fixture.
fn code_session(
    count: usize,
    text: impl Fn(usize) -> String,
) -> Rc<Agent<BrowserMessages<NoCustomMessages>>> {
    let agent = Rc::new(Agent::<BrowserMessages<NoCustomMessages>>::default());
    agent.state().write().unwrap().set_messages(
        (0..count)
            .map(|n| {
                AgentMessage::User(Arc::new(RwLock::new(maestro_app::UserMessage {
                    content: maestro_app::UserContent::Text(text(n)),
                    timestamp: 0.,
                })))
            })
            .collect(),
    );
    agent
}
/// Measure a recipe button's committed geometry and clipped raster paint.
fn painted_button(scene: &Slots, output: &mut egui::FullOutput) -> egui::Pos2 {
    let widget = scene.context.viewport(|v| {
        v.prev_pass
            .widgets
            .layers()
            .flat_map(|(_, widgets)| widgets.iter())
            .filter(|w| w.sense.senses_click())
            .last()
            .copied()
            .unwrap()
    });
    let center = scene
        .context
        .layer_transform_to_global(widget.layer_id)
        .unwrap_or_default()
        * widget.rect.center();
    assert!(viewport().contains(center));
    let mut paint = crate::canvas_paint::Paint::default();
    paint.update(output);
    assert!(
        paint
            .region(
                &scene.context,
                &output.shapes,
                egui::Rect::from_center_size(center, egui::vec2(8., 8.))
            )
            .iter()
            .any(|p| p[3] > 0)
    );
    assert!(
        paint
            .region(
                &scene.context,
                &output.shapes,
                egui::Rect::from_min_size(egui::pos2(0., 181.), egui::vec2(800., 8.))
            )
            .iter()
            .all(|p| p[3] == 0),
        "paint stays inside the final viewport"
    );
    output.textures_delta.clear();
    center
}

/// Activate the focused copy control once through its ordinary keyboard route.
fn keyboard_activation(scene: &Slots, interface: &mut AgentInterface, i18n: &I18n) {
    let saved = super::pointer::capture();
    key_frame(
        scene,
        interface,
        i18n,
        egui::Key::Enter,
        egui::Modifiers::NONE,
    );
    assert_eq!(
        js(&JsValue::NULL, "return transcriptCopies.join('|')")
            .as_string()
            .as_deref(),
        Some("keyboard row 1\n")
    );
    super::pointer::release(&saved);
}

/// Render actual overflowing history once, keeping its interface alive for the first gesture.
fn initial_follow_frame() -> (Slots, AgentInterface, I18n, egui::FullOutput) {
    let scene = Slots::new();
    let i18n = I18n::new(Rc::new(Language)).unwrap();
    let calls = Rc::new(RefCell::new(vec![]));
    maestro_web::register_message_renderer("user", Rc::new(Renderer(calls.clone())));
    let agent = code_session(40, |n| format!("```text\nfollow row {n}\n```"));
    let mut interface = AgentInterface::new(scene.host.clone());
    interface.set_session(Some(agent)).unwrap();
    let output = draw_frame(&scene, &mut interface, &i18n, viewport(), vec![]).unwrap();
    assert_eq!(calls.borrow().len(), 40, "each recipe runs once");
    (scene, interface, i18n, output)
}

/// Observe the native endpoint independently of the canvas producer.
fn copy_slot() -> (js_sys::Array, egui::Pos2) {
    let coordinates = js(
        &JsValue::NULL,
        "const r=[...document.querySelectorAll('.maestro-transcript .control-slot')].at(-1).getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]",
    );
    let coordinates = js_sys::Array::from(&coordinates);
    let center = egui::pos2(
        coordinates.get(0).as_f64().unwrap().to_f32().unwrap(),
        coordinates.get(1).as_f64().unwrap().to_f32().unwrap(),
    );
    (coordinates, center)
}
/// Registered input geometry follows the observed native offset in the first returned frame.
/// # Panics
/// Panics at the geometry commit boundary when hit records retain the old scroll offset.
#[wasm_bindgen]
pub fn transcript_follow_widget_records_match_final_native_scroll() {
    let (scene, _interface, _language, mut output) = initial_follow_frame();
    output.textures_delta.clear();
    let (_, center) = copy_slot();
    let widgets = scene.context.viewport(|v| {
        v.prev_pass
            .widgets
            .layers()
            .flat_map(|(_, widgets)| widgets.iter())
            .filter(|w| w.sense.senses_click())
            .copied()
            .collect::<Vec<_>>()
    });
    assert!(
        widgets.iter().any(|w| scene
            .context
            .layer_transform_to_global(w.layer_id)
            .unwrap_or_default()
            .mul_rect(w.rect)
            .center()
            .distance(center)
            <= 1.),
        "final native slot must have a committed click record within one pixel"
    );
}

/// Focus traverses retained controls stepwise; outside focus restores ordinary visible-only drawing.
/// # Panics
/// Panics if culling drops a focus target, repeats a row or admits more than three extra rows.
#[wasm_bindgen]
pub fn transcript_focus_walks_culled_rows_without_retaining_other_rows() {
    let scene = Slots::new();
    let language = I18n::new(Rc::new(Language)).unwrap();
    let calls = Rc::new(RefCell::new(vec![]));
    maestro_web::register_message_renderer("user", Rc::new(Renderer(calls.clone())));
    let (outside, focus_outside) = sibling_focus(&scene.context);
    let mut interface = AgentInterface::new(scene.host.clone());
    interface
        .set_session(Some(code_session(7, |n| {
            format!("```text\ntraversal row {n}\n```")
        })))
        .unwrap();
    let (ids, visible) = reconcile_seven_rows(&scene, &mut interface, &language, (&calls, outside));
    scene.context.memory_mut(|m| m.request_focus(ids[3]));
    draw_frame(&scene, &mut interface, &language, viewport(), vec![])
        .unwrap()
        .textures_delta
        .clear();
    let visited = calls.take();
    assert!(
        visited.iter().any(|row| row.contains("traversal row 3")),
        "focused row was actually drawn"
    );
    assert_focus_visits(&visited, &visible);
    assert!(
        scene
            .context
            .viewport(|v| v.prev_pass.widgets.get(ids[3]).is_some()),
        "re-admitting a culled row must retain its original widget identity"
    );
    assert_eq!(scene.context.memory(egui::Memory::focused), Some(ids[3]));
    traverse_focus(&scene, &mut interface, &language, &ids, &calls);
    focus_outside.store(true, std::sync::atomic::Ordering::SeqCst);
    let visible = visible_rows();
    draw_frame(&scene, &mut interface, &language, viewport(), vec![])
        .unwrap()
        .textures_delta
        .clear();
    assert_eq!(scene.context.memory(egui::Memory::focused), Some(outside));
    assert_eq!(
        calls.take(),
        visible,
        "outside focus admits no extra transcript rows"
    );
}
/// Focus is the only exception: at most three extra rows, each still invoked once.
fn assert_focus_visits(calls: &[String], visible: &[String]) {
    assert_eq!(
        calls.len(),
        calls.iter().collect::<std::collections::HashSet<_>>().len(),
        "one recipe invocation per admitted row"
    );
    assert!(
        calls.iter().filter(|row| !visible.contains(row)).count() <= 3,
        "only the current focused row and its nearest known focusable neighbours may be added: calls={calls:?} visible={visible:?}"
    );
}

/// Install a genuine sibling toolkit control outside transcript ownership.
fn sibling_focus(context: &egui::Context) -> (egui::Id, Arc<std::sync::atomic::AtomicBool>) {
    use std::sync::atomic::{AtomicBool, Ordering};
    let outside = egui::Id::new("transcript sibling control");
    let focus_outside = Arc::new(AtomicBool::new(false));
    context.on_begin_pass(
        "sibling focus",
        Arc::new({
            let focus_outside = focus_outside.clone();
            move |ui| {
                let response = ui.interact(
                    egui::Rect::from_min_size(egui::pos2(770., 2.), egui::vec2(20., 20.)),
                    outside,
                    egui::Sense::click(),
                );
                if focus_outside.load(Ordering::SeqCst) {
                    response.request_focus();
                }
            }
        }),
    );
    (outside, focus_outside)
}
/// Traverse every culled row in both directions while checking the bounded visit exception.
fn traverse_focus(
    scene: &Slots,
    interface: &mut AgentInterface,
    language: &I18n,
    ids: &[egui::Id],
    calls: &RefCell<Vec<String>>,
) {
    for target in [2, 1, 0, 1, 2, 3, 4, 5, 6, 5, 4, 3, 2, 1, 0] {
        let current = scene.context.memory(egui::Memory::focused).unwrap();
        let previous = ids.iter().position(|id| *id == current).unwrap();
        let visible = visible_rows();
        key_frame(
            scene,
            interface,
            language,
            egui::Key::Tab,
            if target < previous {
                egui::Modifiers::SHIFT
            } else {
                egui::Modifiers::NONE
            },
        );
        assert_focus_visits(&calls.take(), &visible);
        let visible = visible_rows();
        draw_frame(scene, interface, language, viewport(), vec![])
            .unwrap()
            .textures_delta
            .clear();
        assert_focus_visits(&calls.take(), &visible);
        assert_eq!(
            scene.context.memory(egui::Memory::focused),
            Some(ids[target])
        );
    }
}

/// Native focus reveal can scroll between frames; observe each frame's actual visible slots.
fn visible_rows() -> Vec<String> {
    let indices: Vec<usize> = serde_json::from_str(&js(&JsValue::NULL,
        "const root=document.querySelector('.maestro-transcript'),v=root.parentElement.getBoundingClientRect();return JSON.stringify([...root.querySelectorAll('.maestro-transcript-slot')].flatMap((s,i)=>{const r=s.getBoundingClientRect();return r.top<=v.bottom && r.bottom>=v.top?[i]:[]}))"
    ).as_string().unwrap()).unwrap();
    indices
        .into_iter()
        .map(|n| format!("```text\ntraversal row {n}\n```"))
        .collect()
}

/// Establish focus provenance for all seven rows, then measure the no-focus culling control.
fn reconcile_seven_rows(
    scene: &Slots,
    interface: &mut AgentInterface,
    language: &I18n,
    (calls, outside): (&RefCell<Vec<String>>, egui::Id),
) -> (Vec<egui::Id>, Vec<String>) {
    draw_frame(scene, interface, language, viewport(), vec![])
        .unwrap()
        .textures_delta
        .clear();
    assert_eq!(
        calls.take().len(),
        7,
        "rows are unknown/non-focusable until first drawn; initial reconciliation draws all seven"
    );
    let ids = scene.context.viewport(|v| {
        v.prev_pass
            .widgets
            .layers()
            .flat_map(|(_, widgets)| widgets.iter())
            .filter(|w| w.sense.is_focusable() && w.id != outside)
            .map(|w| w.id)
            .collect::<Vec<_>>()
    });
    assert_eq!(ids.len(), 7);
    draw_frame(scene, interface, language, viewport(), vec![])
        .unwrap()
        .textures_delta
        .clear();
    let visible = calls.take();
    assert_eq!(
        visible,
        visible_rows(),
        "no-focus control uses the measured native viewport"
    );
    (ids, visible)
}
