//! The actual caller-selected colors and retained outer style.
use super::{Frame, click, expected, prepared};
use egui::{Event, RawInput, Rect, Shape};
use maestro_theme::{BrandMode, BrandPresentation};
use maestro_web::export_html::template::{HeaderState, HeaderView, render_header};
use std::io;

/// Draw within a real frame, proving the caller's outer style survives.
fn draw(
    frame: &mut Frame,
    doc: &maestro_web::export_html::template::SessionDocument,
    state: &mut HeaderState,
    p: &BrandPresentation<'_>,
    events: Vec<Event>,
) -> io::Result<()> {
    frame.time += 0.05;
    let mut result = Ok(Vec::new());
    frame.output = frame.ctx.run_ui(
        RawInput {
            time: Some(frame.time),
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1800., 4000.),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            let before = ui.style().clone();
            result = render_header(
                ui,
                doc,
                state,
                HeaderView {
                    presentation: p,
                    localize_date: &|_| Ok("local date".into()),
                },
            );
            assert_eq!(ui.style().as_ref(), before.as_ref());
        },
    );
    frame.output.textures_delta.clear();
    result.map(|_| ())
}
/// Resolve one expected pack role without using the production painter.
fn color(p: &BrandPresentation<'_>, role: &str) -> egui::Color32 {
    egui::Color32::from_hex(p.colors[role]).unwrap()
}
/// Read the actual painted text color, including a toolkit override.
fn text_color(frame: &Frame, text: &str) -> egui::Color32 {
    frame
        .output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            Shape::Text(t) if t.galley.job.text == text => {
                Some(t.override_text_color.unwrap_or_else(|| {
                    let color = t.galley.job.sections[0].format.color;
                    if color == egui::Color32::PLACEHOLDER {
                        t.fallback_color
                    } else {
                        color
                    }
                }))
            }
            _ => None,
        })
        .unwrap()
}
/// Require panel/border paint rather than an unused style value.
fn panel(frame: &Frame, p: &BrandPresentation<'_>) {
    assert!(frame.output.shapes.iter().any(|s| matches!(&s.shape, Shape::Rect(r) if r.fill == color(p,"panel") && r.stroke.color == color(p,"border"))));
}
/// Exercise two real brand projections and control/selection states.
pub fn check() {
    let c = super::cases(|n| n == "populated").pop().unwrap();
    let e = expected(&c);
    let doc = prepared(&c);
    let pack = maestro_theme::load_brand_pack(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/brand/brand.json"),
        &maestro_theme::theme::NativeThemeOperations,
    )
    .unwrap();
    let mut prior = Vec::new();
    for mode in [BrandMode::Dark, BrandMode::Light] {
        let p = pack.presentation(mode).unwrap();
        let mut f = Frame::default();
        let mut state = HeaderState::default();
        draw(&mut f, &doc, &mut state, &p, vec![]).unwrap();
        let pos = f.at("[click to show parameters]");
        draw(&mut f, &doc, &mut state, &p, click(pos)).unwrap();
        draw(&mut f, &doc, &mut state, &p, vec![]).unwrap();
        for (label, role) in [
            (&*e.title, "accent"),
            ("Date:", "muted"),
            ("local date", "text"),
            ("required", "warning"),
            ("optional", "muted"),
        ] {
            assert_eq!(text_color(&f, label), color(&p, role));
        }
        panel(&f, &p);
        let current = vec![
            text_color(&f, &e.title),
            text_color(&f, "Date:"),
            text_color(&f, "local date"),
            text_color(&f, "required"),
        ];
        if !prior.is_empty() {
            assert!(prior.iter().zip(&current).all(|(a, b)| a != b));
        }
        prior = current;
        controls(&mut f, &doc, &mut state, &p);
        selection(&mut f, &doc, &mut state, &p);
    }
    let mut p = pack.presentation(BrandMode::Dark).unwrap();
    p.colors.insert("accent", "invalid");
    assert_eq!(
        draw(
            &mut Frame::default(),
            &doc,
            &mut HeaderState::default(),
            &p,
            vec![]
        )
        .unwrap_err()
        .kind(),
        io::ErrorKind::InvalidData
    );
}
/// Idle, hovered, focused and pressed controls carry the projected foreground.
fn controls(
    f: &mut Frame,
    doc: &maestro_web::export_html::template::SessionDocument,
    state: &mut HeaderState,
    p: &BrandPresentation<'_>,
) {
    let pos = f.at("Toggle thinking");
    for events in [
        vec![],
        vec![Event::PointerMoved(pos)],
        vec![super::pointer(pos, true)],
    ] {
        draw(f, doc, state, p, events).unwrap();
        assert_eq!(text_color(f, "Toggle thinking"), color(p, "text"));
    }
    draw(f, doc, state, p, vec![super::pointer(pos, false)]).unwrap();
    draw(
        f,
        doc,
        state,
        p,
        vec![Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    )
    .unwrap();
    assert!(f.ctx.memory(egui::Memory::focused).is_some());
    assert_eq!(text_color(f, "Toggle thinking"), color(p, "text"));
}
/// Selected text is actually painted with the supplied selection foreground.
fn selection(
    f: &mut Frame,
    doc: &maestro_web::export_html::template::SessionDocument,
    state: &mut HeaderState,
    p: &BrandPresentation<'_>,
) {
    let rect = f
        .labels()
        .into_iter()
        .find(|(s, _)| s.starts_with("Literal <script>"))
        .unwrap()
        .1;
    let start = rect.left_top() + egui::vec2(3., 6.);
    let end = start + egui::vec2(50., 0.);
    f.time += 1.;
    for events in [
        vec![Event::PointerMoved(start), super::pointer(start, true)],
        vec![Event::PointerMoved(end)],
        vec![super::pointer(end, false)],
        vec![Event::Copy],
    ] {
        draw(f, doc, state, p, events).unwrap();
    }
    assert!(!crate::copied(f).is_empty());
    assert!(f.output.shapes.iter().any(|s| matches!(&s.shape,Shape::Text(t) if t.galley.job.text.starts_with("Literal <script>") && t.galley.rows.iter().flat_map(|row| row.visuals.mesh.vertices.iter()).any(|v| v.color == color(p,"on-accent")))));
}
