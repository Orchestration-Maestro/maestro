//! Message-link behavior through the retained public control.
#![cfg(test)]
#[path = "support/brand.rs"]
pub mod brand;
mod effects_support;
mod session_links_support;
use session_links_support::*;

#[test]
fn copy_link_preserves_selected_identities() {
    check_urls(&[
        "ordinary",
        "unicode",
        "markup",
        "form-boundaries",
        "empty-identities",
        "whitespace-identities",
    ]);
}

#[test]
fn copy_link_selects_injected_and_current_locations() {
    check_urls(&[
        "file",
        "file-fragment",
        "injected",
        "injected-overrides",
        "injected-empty",
        "injected-fragment",
        "invalid-location",
        "injected-invalid-location",
    ]);
}
#[test]
fn copy_link_preserves_first_valueless_key_and_fragment() {
    check_urls(&[
        "gist-first",
        "gist-empty-value",
        "gist-duplicate-first-valued",
        "gist-duplicate-first-empty",
        "empty-key",
        "existing-selection",
        "fragment-only",
        "gist-hash",
        "query-selection-empty",
        "target-selection-empty",
        "selection-before-gist",
        "gist-before-selection",
    ]);
}
#[test]
fn copy_link_encodes_decoded_query_identity() {
    check_urls(&[
        "gist-ampersand",
        "gist-equals",
        "gist-plus",
        "gist-percent",
        "gist-controls",
        "gist-space",
        "gist-safe-punctuation",
    ]);
    for case in fixture()
        .urls
        .into_iter()
        .filter(|case| case.name.starts_with("gist-") && case.name != "gist-hash")
    {
        let original = url::Url::parse(&case.input.href).unwrap();
        let mut canvas = Canvas::new();
        canvas.input = case.input;
        canvas.activate().unwrap();
        let copied = url::Url::parse(&canvas.host.copies.borrow()[0]).unwrap();
        let pairs: Vec<_> = copied.query_pairs().collect();
        let keys: Vec<_> = original.query_pairs().collect();
        let first = keys
            .iter()
            .find(|(key, _)| keys.iter().find(|(k, _)| k == key).unwrap().1.is_empty())
            .unwrap();
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0].0, first.0);
        assert!(pairs[0].1.is_empty());
    }
}

#[test]
fn copy_link_uses_click_time_context() {
    let mut canvas = Canvas::new();
    canvas.activate().unwrap();
    let first = canvas.host.copies.borrow()[0].clone();
    for (leaf, href, base, entry) in [
        (
            "other",
            "https://example.test/view?old=value#anchor",
            None,
            "message",
        ),
        ("other", "file:///new.html", None, "message"),
        (
            "other",
            "file:///new.html",
            Some("https://share.test/view?gist"),
            "message",
        ),
        (
            "other",
            "file:///new.html",
            Some("https://share.test/view?gist"),
            "next",
        ),
    ] {
        canvas.input = Input {
            leaf: leaf.into(),
            href: href.into(),
            base: base.map(str::to_owned),
            entry: entry.into(),
        };
        canvas.activate().unwrap();
        let copies = canvas.host.copies.borrow();
        let url = url::Url::parse(copies.last().unwrap()).unwrap();
        let pairs: Vec<_> = url.query_pairs().collect();
        assert_eq!(pairs.iter().find(|(k, _)| k == "leafId").unwrap().1, leaf);
        assert_eq!(
            pairs.iter().find(|(k, _)| k == "targetId").unwrap().1,
            entry
        );
        assert_eq!(
            url.scheme(),
            if base.is_some() {
                "https"
            } else if href.starts_with("file:") {
                "file"
            } else {
                "https"
            }
        );
    }
    assert_eq!(canvas.host.copies.borrow()[0], first);
    settle(&canvas, 0, true);
    assert_eq!(canvas.host.live(), 1);
}

#[test]
fn copy_link_success_shows_check_for_1500_ms() {
    let mut canvas = Canvas::new();
    canvas.activate().unwrap();
    assert!(!check(&canvas.frame(vec![]).1));
    assert_eq!(canvas.host.live(), 0);
    settle(&canvas, 0, true);
    assert_eq!(canvas.host.live(), 1);
    assert_eq!(canvas.host.admissions.get(), 1);
    assert!(check(&canvas.frame(vec![]).1));
    canvas.host.advance(1499);
    assert!(check(&canvas.frame(vec![]).1));
    canvas.host.advance(1500);
    let frame = canvas.frame(vec![]).1;
    assert!(!check(&frame));
    assert!(link(&frame).is_some());
    assert_eq!(canvas.host.live(), 0);
}

#[test]
fn copy_link_repeated_success_restores_link() {
    let mut canvas = Canvas::new();
    canvas.activate().unwrap();
    settle(&canvas, 0, true);
    canvas.host.advance(500);
    canvas.activate().unwrap();
    canvas.activate().unwrap();
    settle(&canvas, 2, true);
    assert_eq!(canvas.host.live(), 1);
    canvas.host.advance(1000);
    settle(&canvas, 1, true);
    assert_eq!(canvas.host.admissions.get(), 3);
    assert_eq!(canvas.host.live(), 1);
    canvas.host.advance(1500);
    assert!(check(&canvas.frame(vec![]).1));
    canvas.activate().unwrap();
    settle(&canvas, 3, false);
    canvas.host.advance(2499);
    assert!(check(&canvas.frame(vec![]).1));
    canvas.host.advance(2500);
    assert!(!check(&canvas.frame(vec![]).1));
    canvas.host.advance(5000);
    assert_eq!(canvas.host.live(), 0);
}

#[test]
fn copy_link_failure_never_claims_success() {
    let mut canvas = Canvas::new();
    canvas.activate().unwrap();
    settle(&canvas, 0, false);
    assert!(!check(&canvas.frame(vec![]).1));
    assert_eq!(canvas.host.admissions.get(), 0);
    canvas.activate().unwrap();
    settle(&canvas, 1, true);
    canvas.host.advance(1000);
    canvas.activate().unwrap();
    settle(&canvas, 2, false);
    assert!(check(&canvas.frame(vec![]).1));
    assert_eq!(canvas.host.admissions.get(), 1);
    canvas.host.advance(1500);
    assert!(!check(&canvas.frame(vec![]).1));
}

#[test]
fn copy_link_drop_revokes_pending_feedback() {
    use maestro_web::export_html::template::CopyLinkState;
    let mut canvas = Canvas::new();
    canvas.activate().unwrap();
    let mut neighbor = Canvas::new();
    neighbor.host = canvas.host.clone();
    neighbor.state = CopyLinkState::new(neighbor.host.clone());
    neighbor.activate().unwrap();
    let old = std::mem::replace(&mut canvas.state, CopyLinkState::new(canvas.host.clone()));
    drop(old);
    settle(&canvas, 0, true);
    assert_eq!(canvas.host.admissions.get(), 0);
    canvas.activate().unwrap();
    settle(&canvas, 2, true);
    assert!(check(&canvas.frame(vec![]).1));
    let old = std::mem::replace(&mut canvas.state, CopyLinkState::new(canvas.host.clone()));
    drop(old);
    assert_eq!(canvas.host.live(), 0);
    canvas.activate().unwrap();
    settle(&canvas, 3, true);
    assert!(check(&canvas.frame(vec![]).1));
    settle(&neighbor, 1, true);
    assert_eq!(canvas.host.live(), 2);
    let host = canvas.host.clone();
    let weak = std::rc::Rc::downgrade(&host);
    drop(canvas);
    assert_eq!(host.live(), 1);
    assert!(check(&neighbor.frame(vec![]).1));
    drop(neighbor);
    assert_eq!(host.live(), 0);
    host.run_work();
    drop(host);
    assert!(weak.upgrade().is_none());
}

#[test]
fn copy_link_timer_failure_does_not_stick() {
    use maestro_web::browser::attachment_content::AttachmentUiError;
    let mut canvas = Canvas::new();
    canvas.activate().unwrap();
    *canvas.host.failure.borrow_mut() = Some(AttachmentUiError::Host("timer refused".into()));
    settle(&canvas, 0, true);
    assert_eq!(canvas.host.live(), 0);
    assert!(!check(&canvas.frame(vec![]).1));
    canvas.activate().unwrap();
    settle(&canvas, 1, true);
    assert!(check(&canvas.frame(vec![]).1));
    canvas.activate().unwrap();
    *canvas.host.failure.borrow_mut() = Some(AttachmentUiError::Host("timer refused".into()));
    settle(&canvas, 2, true);
    assert_eq!(canvas.host.live(), 0);
    assert!(!check(&canvas.frame(vec![]).1));
}

#[test]
fn copy_link_control_preserves_geometry_tooltip_and_glyph() {
    let fixture = fixture();
    let geometry = fixture.control.geometry;
    let catalog = maestro_web::icons::icons().unwrap();
    assert_eq!(
        catalog["Link"]
            .iter()
            .map(|(_, attrs)| match &attrs["d"] {
                maestro_web::icons::SvgValue::Text(text) => text.clone(),
                maestro_web::icons::SvgValue::Number(_) => panic!("path must be text"),
            })
            .collect::<Vec<_>>(),
        fixture.control.paths
    );
    for (index, identity) in fixture.control.identities.into_iter().enumerate() {
        let mut canvas = Canvas::new();
        canvas.layout = [
            egui::Layout::top_down(egui::Align::Min),
            egui::Layout::left_to_right(egui::Align::Center),
            egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
        ][index % 3];
        let message = egui::Rect::from_min_size(egui::pos2(10., 10.), egui::vec2(300., 100.));
        canvas.rect = egui::Rect::from_min_size(
            egui::pos2(
                message.right() - geometry.inset - geometry.width,
                message.top() + geometry.inset,
            ),
            egui::vec2(geometry.width, geometry.height),
        );
        canvas.input.entry = identity.input;
        let (response, output) = canvas.frame(vec![]);
        let response = response.unwrap();
        assert_eq!(response.rect.size(), egui::vec2(28., 28.));
        let bounds = link(&output).unwrap();
        assert_eq!(
            bounds.size(),
            egui::vec2(geometry.glyph_width, geometry.glyph_height)
        );
        assert_eq!(bounds.center(), response.rect.center());
        assert!(
            (response.rect.width() - 2. * geometry.padding - bounds.width() - 2.).abs()
                < f32::EPSILON
        );
        canvas.frame(pointer(canvas.rect.center(), true)).0.unwrap();
        let (response, output) = canvas.frame(pointer(canvas.rect.center(), false));
        assert!(response.unwrap().clicked());
        assert!(output.platform_output.events.iter().any(|event|matches!(event,egui::output::OutputEvent::Clicked(info) if info.label.as_deref()==Some(&fixture.control.title))));
        let url = url::Url::parse(&canvas.host.copies.borrow()[0]).unwrap();
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "targetId")
                .unwrap()
                .1,
            identity.expected
        );
        check_tooltip(&mut canvas, &fixture.control.title);
    }
}

#[test]
fn copy_link_control_hides_until_message_hover() {
    let mut canvas = Canvas::new();
    canvas.hovered = false;
    assert!(link(&canvas.frame(vec![]).1).is_none());
    canvas.time = 0.01;
    canvas.hovered = true;
    let start = canvas.frame(vec![]).1;
    canvas.time += f64::from(fixture().control.geometry.transition_seconds);
    let settled = canvas.frame(vec![]).1;
    assert!(link(&settled).is_some());
    let alpha = |output: &egui::FullOutput| {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(frame) if frame.rect == canvas.rect => Some(frame.fill.a()),
                _ => None,
            })
            .unwrap_or(0)
    };
    assert!(alpha(&start) < alpha(&settled));
    canvas.activate().unwrap();
    settle(&canvas, 0, true);
    canvas.time = 2.;
    canvas.hovered = false;
    canvas.frame(vec![]).0.unwrap();
    canvas.time += 0.15;
    assert!(
        canvas
            .frame(vec![])
            .1
            .shapes
            .iter()
            .all(|shape| matches!(shape.shape, egui::Shape::Noop))
    );
    canvas.time = 3.;
    canvas.hovered = true;
    canvas.frame(vec![]).0.unwrap();
    canvas.time += 0.15;
    assert!(check(&canvas.frame(vec![]).1));
    canvas.time = 4.;
    canvas.hovered = false;
    canvas.frame(vec![]).0.unwrap();
    canvas.time += 0.05;
    canvas.hovered = true;
    canvas.frame(vec![]).0.unwrap();
    canvas.time += 0.15;
    assert!(check(&canvas.frame(vec![]).1));
}

#[test]
fn copy_link_control_clips_pointer_and_keeps_keyboard_activation() {
    let mut canvas = Canvas::new();
    let full = canvas.rect;
    canvas.clip = egui::Rect::from_min_max(full.center(), full.max);
    assert_eq!(canvas.frame(vec![]).0.unwrap().rect, full);
    for (enabled, start, end) in [
        (true, full.min, full.center()),
        (true, full.center(), full.min),
        (false, full.center(), full.center()),
    ] {
        canvas.enabled = enabled;
        canvas.frame(vec![]).0.unwrap();
        canvas.frame(pointer(start, true)).0.unwrap();
        canvas.frame(pointer(end, false)).0.unwrap();
        assert!(canvas.host.copies.borrow().is_empty());
    }
    canvas.enabled = true;
    canvas.clip = egui::Rect::NOTHING;
    canvas.activate().unwrap();
    assert!(canvas.host.copies.borrow().is_empty());
    canvas.clip = egui::Rect::EVERYTHING;
    canvas.frame(pointer(full.center(), true)).0.unwrap();
    canvas.rect = full.translate(egui::vec2(100., 0.));
    canvas.frame(pointer(full.center(), false)).0.unwrap();
    assert!(canvas.host.copies.borrow().is_empty());
    canvas.frame(vec![]).0.unwrap();
    for pressed in [true, false] {
        canvas
            .frame(vec![egui::Event::PointerButton {
                pos: canvas.rect.center(),
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }])
            .0
            .unwrap();
    }
    assert!(canvas.host.copies.borrow().is_empty());
    canvas.covered = true;
    canvas.frame(vec![]).0.unwrap();
    canvas.frame(vec![]).0.unwrap();
    canvas.activate().unwrap();
    assert!(canvas.host.copies.borrow().is_empty());
    canvas.covered = false;
    canvas.clip = egui::Rect::NOTHING;
    let response = canvas.frame(vec![]).0.unwrap();
    response.request_focus();
    canvas
        .frame(vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }])
        .0
        .unwrap();
    assert_eq!(canvas.host.copies.borrow().len(), 1);
}

#[test]
fn copy_link_dispatches_only_its_target() {
    use maestro_web::export_html::template::CopyLinkState;
    let host = std::rc::Rc::new(effects_support::Host::default());
    let mut controls = [
        CopyLinkState::new(host.clone()),
        CopyLinkState::new(host.clone()),
    ];
    let context = egui::Context::default();
    let ids = ["x\" data-extra=\"yes", "line\r\nend"];
    for (index, target) in ids.iter().enumerate() {
        for down in [None, Some(true), Some(false)] {
            dispatch_frame(
                &context,
                &mut controls,
                down.map_or_else(Vec::new, |pressed| {
                    pointer(
                        egui::pos2(54. + f32::from(u16::try_from(index).unwrap()) * 100., 54.),
                        pressed,
                    )
                }),
            );
        }
        assert_eq!(host.copies.borrow().len(), index + 1);
        let url = url::Url::parse(&host.copies.borrow()[index]).unwrap();
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "targetId")
                .unwrap()
                .1,
            *target
        );
    }
}

#[test]
fn copy_link_check_uses_selected_typography() {
    for name in ["brand.json", "signal.json"] {
        let mut canvas = Canvas::new();
        canvas.context.all_styles_mut(|style| {
            style
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::monospace(40.));
        });
        canvas.activate().unwrap();
        settle(&canvas, 0, true);
        let output = canvas
            .pack_frame(vec![], name, maestro_theme::BrandMode::Dark)
            .1;
        let text = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "✓" => Some(text),
                _ => None,
            })
            .unwrap();
        let expected_font = egui::FontId::proportional(16.);
        assert_eq!(text.galley.job.sections[0].format.font_id, expected_font);
        let expected = canvas.context.fonts_mut(|fonts| {
            fonts.layout_no_wrap("✓".into(), expected_font, text.fallback_color)
        });
        assert_eq!(text.galley.size(), expected.size());
        assert_eq!(text.pos + text.galley.size() / 2., canvas.rect.center());
    }
}

#[test]
fn copy_link_border_switches_on_first_transition_frame() {
    for name in ["brand.json", "signal.json"] {
        for mode in [
            maestro_theme::BrandMode::Dark,
            maestro_theme::BrandMode::Light,
        ] {
            let pack = maestro_theme::load_brand_pack(name, &brand::Pack).unwrap();
            let presentation = pack.presentation(mode).unwrap();
            let mut canvas = Canvas::new();
            canvas.pack_frame(vec![], name, mode).0.unwrap();
            canvas.time = 0.01;
            for (events, role) in [
                (
                    vec![egui::Event::PointerMoved(canvas.rect.center())],
                    "accent",
                ),
                (vec![egui::Event::PointerMoved(egui::Pos2::ZERO)], "border"),
            ] {
                let output = canvas.pack_frame(events, name, mode).1;
                let border = painted_frame(&output, canvas.rect).stroke.color;
                assert_eq!(
                    border,
                    egui::Color32::from_hex(presentation.colors[role]).unwrap()
                );
                canvas.time += 0.15;
                canvas.pack_frame(vec![], name, mode).0.unwrap();
            }
            canvas.activate().unwrap();
            settle(&canvas, 0, true);
            let output = canvas.pack_frame(vec![], name, mode).1;
            let border = painted_frame(&output, canvas.rect).stroke.color;
            assert_eq!(
                border,
                egui::Color32::from_hex(presentation.colors["success"]).unwrap()
            );
        }
    }
}

#[test]
fn copy_link_paints_selected_pack_roles() {
    use maestro_theme::BrandMode;
    for name in ["brand.json", "signal.json"] {
        for mode in [BrandMode::Dark, BrandMode::Light] {
            check_pack(name, mode);
        }
    }
}
/// Inspect the actual full control decoration, not its supplied toolkit style.
fn painted_roles(output: &egui::FullOutput, rect: egui::Rect, colors: [egui::Color32; 2]) {
    let frame = painted_frame(output, rect);
    assert_eq!(frame.fill, colors[0]);
    assert_eq!(frame.stroke.color, colors[1]);
}

/// Inspect the frame submitted to the painter for the measured control slot.
fn painted_frame(output: &egui::FullOutput, rect: egui::Rect) -> &egui::epaint::RectShape {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(frame) if frame.rect == rect => Some(frame),
            _ => None,
        })
        .unwrap()
}

/// Draw two controls under a clickable containing message.
fn dispatch_frame(
    context: &egui::Context,
    controls: &mut [maestro_web::export_html::template::CopyLinkState; 2],
    events: Vec<egui::Event>,
) {
    use maestro_web::export_html::template::{CopyLinkView, render_copy_link_button};
    let pack = maestro_theme::load_brand_pack("brand.json", &brand::Pack).unwrap();
    let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
    let ids = ["x\" data-extra=\"yes", "line\r\nend"];
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800., 600.),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            let parent = ui.interact(
                egui::Rect::from_min_size(egui::pos2(20., 20.), egui::vec2(400., 200.)),
                egui::Id::new("message"),
                egui::Sense::click(),
            );
            for (i, state) in controls.iter_mut().enumerate() {
                render_copy_link_button(
                    ui,
                    ids[i],
                    state,
                    CopyLinkView {
                        presentation: &presentation,
                        current_leaf_id: "branch",
                        location_href: "https://example.test/view",
                        share_base_url: None,
                        rect: egui::Rect::from_min_size(
                            egui::pos2(40. + f32::from(u16::try_from(i).unwrap()) * 100., 40.),
                            egui::vec2(28., 28.),
                        ),
                        message_hovered: true,
                    },
                )
                .unwrap();
            }
            assert!(!parent.clicked());
        },
    );
    output.textures_delta.clear();
}

/// Inspect each actually painted state in one selected pack and mode.
fn check_pack(name: &str, mode: maestro_theme::BrandMode) {
    let pack = maestro_theme::load_brand_pack(name, &brand::Pack).unwrap();
    let p = pack.presentation(mode).unwrap();
    let color = |role: &str| egui::Color32::from_hex(p.colors[role]).unwrap();
    let mut canvas = Canvas::new();
    canvas.hovered = false;
    assert!(link(&canvas.pack_frame(vec![], name, mode).1).is_none());
    canvas.time = 1.;
    canvas.hovered = true;
    canvas.pack_frame(vec![], name, mode).0.unwrap();
    canvas.time += 0.15;
    let normal = canvas.pack_frame(vec![], name, mode).1;
    painted_roles(&normal, canvas.rect, [color("panel"), color("border")]);
    assert!(canvas.pixels.contains(&color("muted")));
    canvas.time = 2.;
    canvas
        .pack_frame(
            vec![egui::Event::PointerMoved(canvas.rect.center())],
            name,
            mode,
        )
        .0
        .unwrap();
    canvas.time += 0.15;
    let hovered = canvas.pack_frame(vec![], name, mode).1;
    painted_roles(&hovered, canvas.rect, [color("accent"), color("accent")]);
    assert!(canvas.pixels.contains(&color("on-accent")));
    let response = canvas
        .pack_frame(vec![egui::Event::PointerGone], name, mode)
        .0
        .unwrap();
    response.request_focus();
    canvas.time = 3.;
    canvas.pack_frame(vec![], name, mode).0.unwrap();
    canvas.time += 0.15;
    painted_roles(
        &canvas.pack_frame(vec![], name, mode).1,
        canvas.rect,
        [color("accent"), color("accent")],
    );
    canvas.activate().unwrap();
    settle(&canvas, 0, true);
    canvas.time = 4.;
    canvas.pack_frame(vec![], name, mode).0.unwrap();
    canvas.time += 0.15;
    let copied = canvas.pack_frame(vec![], name, mode).1;
    painted_roles(&copied, canvas.rect, [color("success"), color("success")]);
    let text = copied
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == p.glyphs["passed"].symbol => {
                Some(text)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(text.fallback_color, color("on-accent"));
}

/// Observe the actual tooltip after the toolkit hover delay.
fn check_tooltip(canvas: &mut Canvas, title: &str) {
    canvas.frame(vec![egui::Event::PointerGone]).0.unwrap();
    canvas.time = 1.;
    canvas
        .frame(vec![egui::Event::PointerMoved(canvas.rect.center())])
        .0
        .unwrap();
    canvas.time = 2.;
    canvas.frame(vec![]).0.unwrap();
    let (_, output) = canvas.frame(vec![]);
    assert!(
        output.shapes.iter().any(
            |shape| matches!(&shape.shape,egui::Shape::Text(text) if text.galley.text()==title)
        )
    );
}
