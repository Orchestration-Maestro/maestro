//! Measured streaming cursor placement and native animation in both brand packs.
use super::aggregate::Language;
use super::{check, js};
use crate::reading_slots_support::Slots;
use maestro_app::{AgentMessage, NoCustomMessages};
use maestro_web::tools::types::ToolRenderView;
use maestro_web::{I18n, StreamingMessageContainer, StreamingMessageProps};
use std::{
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::prelude::*;

/// Measure the actual native cursor, surrounding canvas layout and animated muted-role paint.
/// # Panics
/// Panics when geometry, theme color or native pulse differs from the public stream presentation.
#[wasm_bindgen]
pub fn transcript_placeholder_keeps_measured_geometry_and_pulse_in_each_theme() {
    for pack in ["brand.json", "signal.json"] {
        for mode in [
            maestro_theme::BrandMode::Dark,
            maestro_theme::BrandMode::Light,
        ] {
            let mut scene = Slots::new();
            scene.pack =
                maestro_theme::load_brand_pack(pack, &crate::support::brand::Pack).unwrap();
            scene.mode = mode;
            let i18n = I18n::new(Rc::new(Language)).unwrap();
            let mut stream = StreamingMessageContainer::<NoCustomMessages>::new(scene.host.clone());
            let placeholder = draw(&scene, &i18n, &mut stream);
            assert!(
                (placeholder.height() - 28.).abs() < 0.1,
                "{}",
                placeholder.height()
            );
            measure(&scene, placeholder);
            stream
                .set_message(
                    Some(AgentMessage::Assistant(Arc::new(RwLock::new(
                        maestro_app::faux_assistant_message(
                            "assistant before cursor",
                            maestro_app::FauxAssistantMessageOptions::default(),
                        ),
                    )))),
                    true,
                )
                .unwrap();
            let row = draw(&scene, &i18n, &mut stream);
            measure(&scene, row);
            check(
                &JsValue::NULL,
                "const text=document.querySelector('.maestro-markdown').getBoundingClientRect(),cursor=document.querySelector('.maestro-transcript-cursor').getBoundingClientRect();const scale=document.querySelector('#canvas').getBoundingClientRect().width/800;return Math.abs((cursor.top-text.bottom)/scale-12)<0.2",
            );
        }
    }
}

/// Draw the public stream without a second runtime or inferred CSS-only dimensions.
fn draw(scene: &Slots, i18n: &I18n, stream: &mut StreamingMessageContainer) -> egui::Rect {
    let host: Rc<dyn maestro_web::browser::attachment_content::AttachmentHost> = scene.host.clone();
    let p = scene.pack.presentation(scene.mode).unwrap();
    let mut rect = egui::Rect::NOTHING;
    scene
        .context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 600.),
                )),
                ..Default::default()
            },
            |ui| {
                rect = stream
                    .render(
                        ui,
                        StreamingMessageProps {
                            is_streaming: true,
                            ..Default::default()
                        },
                        ToolRenderView {
                            host: &host,
                            i18n,
                            presentation: &p,
                            highlighter: &crate::highlighter_support::NoHighlight,
                            modal: None,
                        },
                    )
                    .response
                    .unwrap()
                    .rect;
            },
        )
        .textures_delta
        .clear();
    rect
}

/// Read native dimensions and pause the real animation at its reference phase boundaries.
fn measure(scene: &Slots, row: egui::Rect) {
    let cursor = js(
        &JsValue::NULL,
        "return document.querySelector('.maestro-transcript-cursor')",
    );
    let color = scene.pack.presentation(scene.mode).unwrap().colors["muted"].to_owned();
    check(
        &cursor,
        &format!(
            "const r=value.getBoundingClientRect(),canvas=document.querySelector('#canvas').getBoundingClientRect(),scale=canvas.width/800;const probe=document.createElement('span');probe.style.color='{color}';document.body.append(probe);const paint=getComputedStyle(probe).color;probe.remove();const animation=value.getAnimations()[0];animation.pause();const opacity=[0,1000,2000].map(time=>{{animation.currentTime=time;return Number(getComputedStyle(value).opacity)}});return Math.abs(r.width/scale-8)<0.1 && Math.abs(r.height/scale-16)<0.1 && Math.abs((r.left-canvas.left)/scale-({}+16))<0.2 && Math.abs((canvas.top+{}*scale-r.bottom)/scale-12)<0.2 && getComputedStyle(value).backgroundColor===paint && opacity[0]===1 && opacity[1]===0.5 && opacity[2]===1",
            row.left(),
            row.bottom()
        ),
    );
}
