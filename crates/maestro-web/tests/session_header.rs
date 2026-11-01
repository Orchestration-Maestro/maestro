//! Public identity drawing through the replaceable native reading boundary.
#![cfg(test)]
use crate::session_header_support;
use base64::Engine as _;
use maestro_theme::BrandMode;
use maestro_web::browser::attachment_content::AttachmentHostError;
use maestro_web::export_html::template::HeaderState;
use session_header_support::{Canvas, Event, Failure, Host, document, draw, pack, pointer};
use std::rc::Rc;
/// Draw one identity through the public caller and retain its actual factory.
#[test]
fn header_reading_admission_retains_owned_content_identity() {
    let host = Rc::new(Host::default());
    host.height.set(120.);
    let mut state = HeaderState::new(host.clone());
    let pack = pack();
    let presentation = pack.presentation(BrandMode::Dark).unwrap();
    let layout = egui::Layout::top_down(egui::Align::Min);
    let doc = document(&serde_json::json!("<owned 雪>"));
    draw(
        &mut state,
        &doc,
        &presentation,
        layout,
        egui::vec2(400., 40.),
    )
    .0
    .unwrap();
    let (id, first) = {
        let published = host.published.borrow();
        let (id, (content, rect)) = published.iter().next().unwrap();
        assert!((rect.height() - 40.).abs() < f32::EPSILON);
        (*id, Rc::clone(content))
    };
    drop(doc);
    assert_eq!(
        *host.events.borrow(),
        [Event::Measure(id), Event::Read(id), Event::Publish(id)]
    );
    host.events.borrow_mut().clear();
    draw(
        &mut state,
        &document(&serde_json::json!("<owned 雪>")),
        &presentation,
        layout,
        egui::vec2(400., 40.),
    )
    .0
    .unwrap();
    assert!(Rc::ptr_eq(&first, &host.published.borrow()[&id].0));
    assert_eq!(
        *host.events.borrow(),
        [Event::Measure(id), Event::Read(id), Event::Publish(id)]
    );
    draw(
        &mut state,
        &document(&serde_json::json!("changed")),
        &presentation,
        layout,
        egui::vec2(400., 40.),
    )
    .0
    .unwrap();
    assert!(!Rc::ptr_eq(&first, &host.published.borrow()[&id].0));
    check_metadata_replacement(&mut state, &host, &presentation);
}
/// Each retained owner releases only its own registry identity.
#[test]
fn header_instances_release_only_their_registered_surface() {
    let host = Rc::new(Host::default());
    let mut first = HeaderState::new(host.clone());
    let mut second = HeaderState::new(host.clone());
    let pack = pack();
    let presentation = pack.presentation(BrandMode::Light).unwrap();
    let doc = document(&serde_json::json!("same"));
    for state in [&mut first, &mut second] {
        draw(
            state,
            &doc,
            &presentation,
            egui::Layout::top_down(egui::Align::Min),
            egui::vec2(300., 40.),
        )
        .0
        .unwrap();
    }
    let ids: Vec<_> = host.published.borrow().keys().copied().collect();
    assert_eq!(ids.len(), 2);
    let slots = host.slot_ids.borrow();
    assert_eq!(slots.len(), 2);
    assert!(slots[0].iter().all(|id| !slots[1].contains(id)));
    drop(slots);
    host.events.borrow_mut().clear();
    drop(first);
    let remaining = *host.published.borrow().keys().next().unwrap();
    assert_eq!(host.published.borrow().len(), 1);
    let removed = *ids.iter().find(|id| **id != remaining).unwrap();
    assert_eq!(*host.events.borrow(), [Event::Remove(removed)]);
    let retained_alias = Rc::clone(&host.published.borrow()[&remaining].0);
    drop(second);
    assert!(host.published.borrow().is_empty());
    assert_eq!(
        *host.events.borrow(),
        [Event::Remove(removed), Event::Remove(remaining)]
    );
    drop(retained_alias);
    assert_eq!(host.events.borrow().len(), 2);
}
/// Caller layout never controls the reading viewport's layout direction.
#[test]
fn header_layouts_are_parent_independent() {
    let pack = pack();
    let presentation = pack.presentation(BrandMode::Dark).unwrap();
    for layout in [
        egui::Layout::top_down(egui::Align::Min),
        egui::Layout::left_to_right(egui::Align::Center),
        egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
    ] {
        for width in [0., 1., 300.] {
            let host = Rc::new(Host::default());
            host.height.set(120.);
            let mut state = HeaderState::new(host.clone());
            let (result, output) = draw(
                &mut state,
                &document(&serde_json::json!("reading identity")),
                &presentation,
                layout,
                egui::vec2(width, 40.),
            );
            result.unwrap();
            let published = host.published.borrow();
            let (_, rect) = published.values().next().unwrap();
            assert!(rect.width() >= 0. && rect.width() <= width);
            assert!((rect.height() - 40.).abs() < f32::EPSILON);
            assert!(
                output
                    .shapes
                    .iter()
                    .all(|s| !matches!(s.shape, egui::Shape::Text(_)))
            );
        }
    }
}
/// Only successful publication commits the feature's retained identity.
#[test]
fn header_native_failures_preserve_the_reached_phase() {
    let host = Rc::new(Host::default());
    host.height.set(120.);
    let mut state = HeaderState::new(host.clone());
    let pack = pack();
    let presentation = pack.presentation(BrandMode::Dark).unwrap();
    let layout = egui::Layout::top_down(egui::Align::Min);
    let size = egui::vec2(300., 40.);
    let invalid = document(&serde_json::json!(true));
    assert_eq!(
        draw(&mut state, &invalid, &presentation, layout, size)
            .0
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidData
    );
    assert!(host.events.borrow().is_empty());
    let mut with_date = document(&serde_json::json!("original"));
    with_date.data.get_mut("header").unwrap()["timestamp"] = serde_json::json!(1);
    let date_error = session_header_support::draw_view(
        &mut state,
        &with_date,
        &maestro_web::export_html::template::HeaderView {
            presentation: &presentation,
            localize_date: &|_| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "native date failure",
                ))
            },
        },
        layout,
        size,
    )
    .0
    .unwrap_err();
    assert_eq!(date_error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(host.events.borrow().is_empty());
    let original = document(&serde_json::json!("original"));
    draw(&mut state, &original, &presentation, layout, size)
        .0
        .unwrap();
    check_native_failures(&mut state, &host, &presentation);
}
/// Selected properties reach the native boundary without mutating caller style.
#[test]
fn header_scope_uses_shared_theme_and_preserves_caller_style() {
    let default = pack();
    let alternate = crate::support::brand::pack(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/brand/packs/signal.json"
    ))
    .unwrap();
    for pack in [&default, &alternate] {
        for mode in [BrandMode::Light, BrandMode::Dark] {
            let presentation = pack.presentation(mode).unwrap();
            check_control_states(&presentation);
            let host = Rc::new(Host::default());
            let mut state = HeaderState::new(host.clone());
            draw(
                &mut state,
                &document(&serde_json::json!("identity")),
                &presentation,
                egui::Layout::left_to_right(egui::Align::Center),
                egui::vec2(400., 40.),
            )
            .0
            .unwrap();
            let css = host.css.borrow();
            assert_eq!(css.len(), 2);
            for observed in css.iter() {
                assert_eq!(observed, &presentation.css_properties());
            }
        }
    }
}
/// Documents accepted without a header still draw the fallback identity.
#[test]
fn header_absent_from_loaded_document_renders_without_panicking() {
    let doc = maestro_web::export_html::template::load_document(
        &base64::engine::general_purpose::STANDARD.encode(r#"{"entries":[]}"#),
        "",
        None,
    )
    .unwrap();
    let host = Rc::new(Host::default());
    let mut state = HeaderState::new(host.clone());
    let pack = pack();
    let presentation = pack.presentation(BrandMode::Dark).unwrap();
    draw(
        &mut state,
        &doc,
        &presentation,
        egui::Layout::top_down(egui::Align::Min),
        egui::vec2(400., 200.),
    )
    .0
    .unwrap();
    assert_eq!(host.published.borrow().len(), 1);
}

/// The real public route paints only the three canvas controls.
#[test]
fn header_document_route_draws_all_metadata_and_controls() {
    let host = Rc::new(Host::default());
    host.height.set(300.);
    host.controls.set(true);
    let mut state = HeaderState::new(host.clone());
    let pack = pack();
    let presentation = pack.presentation(BrandMode::Dark).unwrap();
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/session_header.json")).unwrap();
    let row = corpus
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "populated")
        .unwrap();
    let doc = maestro_web::export_html::template::load_document(
        &base64::engine::general_purpose::STANDARD.encode(row["input"]["data"].to_string()),
        "",
        None,
    )
    .unwrap();
    let (result, output) = draw(
        &mut state,
        &doc,
        &presentation,
        egui::Layout::top_down(egui::Align::Min),
        egui::vec2(800., 600.),
    );
    assert!(result.unwrap().is_empty());
    session_header_support::assert_button_labels(&output.shapes, &row["output"]);
    let sizes = host.sizes.borrow();
    assert_eq!(sizes.len(), 3);
    assert!(sizes[0].1.x > sizes[1].1.x && sizes[1].1.x > sizes[2].1.x);
    assert_eq!(host.holes.borrow().len(), 3);
}

/// A single live observation supplies holes by ID, even in reverse order.
#[test]
fn header_controls_use_one_fresh_geometry_snapshot() {
    let host = Rc::new(Host::default());
    host.height.set(300.);
    host.controls.set(true);
    let mut state = HeaderState::new(host.clone());
    let pack = pack();
    let p = pack.presentation(BrandMode::Dark).unwrap();
    draw(
        &mut state,
        &document(&serde_json::json!("geometry")),
        &p,
        egui::Layout::top_down(egui::Align::Min),
        egui::vec2(800., 600.),
    )
    .0
    .unwrap();
    assert_eq!(
        host.events
            .borrow()
            .iter()
            .filter(|e| matches!(e, Event::Read(_)))
            .count(),
        1
    );
    let published = host.published.borrow();
    let viewport = published.values().next().unwrap().1;
    let sizes = host.sizes.borrow();
    let holes = host.holes.borrow();
    for ((i, (_, size)), hole) in sizes.iter().enumerate().rev().zip(holes.iter()) {
        assert_eq!(
            hole.min,
            viewport.min + egui::vec2(10. + f32::from(u16::try_from(i).unwrap()) * 150., 5.)
        );
        assert_eq!(hole.size(), *size);
    }
}

/// Actual pointer activations emit requests without executing them.
#[test]
fn header_controls_emit_ordered_typed_actions() {
    use maestro_web::export_html::template::HeaderAction;
    let pack = pack();
    let p = pack.presentation(BrandMode::Dark).unwrap();
    let mut canvas = Canvas::new(&p);
    assert!(canvas.frame(vec![]).0.is_empty());
    let holes = canvas.host.holes.borrow().clone();
    let mut admitted = Vec::new();
    for rect in holes.iter().rev() {
        assert!(canvas.frame(pointer(rect.center(), true)).0.is_empty());
        admitted.extend(canvas.frame(pointer(rect.center(), false)).0);
    }
    assert_eq!(
        admitted,
        [
            HeaderAction::ToggleThinking,
            HeaderAction::ToggleTools,
            HeaderAction::DownloadSessionJson
        ]
    );
    for pressed in [true, false] {
        assert!(
            canvas
                .frame(pointer(egui::pos2(700., 250.), pressed))
                .0
                .is_empty()
        );
    }
}

/// Real button paint proves selected roles in resting, hover, press and focus states.
fn check_control_states(p: &maestro_theme::BrandPresentation<'_>) {
    let mut canvas = Canvas::new(p);
    let resting = canvas.frame(vec![]).1;
    let rect = *canvas.host.holes.borrow().last().unwrap();
    check_control_colors(&resting, rect, p, "border");
    canvas.frame(vec![egui::Event::PointerMoved(rect.center())]);
    check_control_colors(&canvas.frame(vec![]).1, rect, p, "accent");
    canvas.frame(pointer(rect.center(), true));
    check_control_colors(&canvas.frame(vec![]).1, rect, p, "accent");
    canvas.frame(pointer(rect.center(), false));
    canvas.frame(vec![
        egui::Event::PointerMoved(egui::pos2(700., 500.)),
        egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    check_control_colors(&canvas.frame(vec![]).1, rect, p, "accent");
}
/// Compare the actual full button frame and text with the selected roles.
fn check_control_colors(
    output: &egui::FullOutput,
    rect: egui::Rect,
    p: &maestro_theme::BrandPresentation<'_>,
    border: &str,
) {
    let color = |role: &str| egui::Color32::from_hex(p.colors[role]).unwrap();
    let shape = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(r) if r.rect == rect => Some(r),
            _ => None,
        })
        .unwrap();
    assert_eq!(shape.fill, color("panel"));
    assert_eq!(shape.stroke.color, color(border));
    let text = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(t) if t.galley.text() == "Toggle thinking" => Some(t),
            _ => None,
        })
        .unwrap();
    assert_eq!(text.fallback_color, color("text"));
}

/// Metadata changes replace the factory even when the title is unchanged.
fn check_metadata_replacement(
    state: &mut HeaderState,
    host: &Host,
    presentation: &maestro_theme::BrandPresentation<'_>,
) {
    let id = *host.published.borrow().keys().next().unwrap();
    let layout = egui::Layout::top_down(egui::Align::Min);
    let previous = Rc::clone(&host.published.borrow()[&id].0);
    let mut same_identity = document(&serde_json::json!("changed"));
    same_identity.global_stats.models.push("new model".into());
    draw(
        state,
        &same_identity,
        presentation,
        layout,
        egui::vec2(400., 40.),
    )
    .0
    .unwrap();
    assert!(!Rc::ptr_eq(&previous, &host.published.borrow()[&id].0));
}

/// Reached host failures preserve the original factory and publication.
fn check_native_failures(
    state: &mut HeaderState,
    host: &Host,
    presentation: &maestro_theme::BrandPresentation<'_>,
) {
    let layout = egui::Layout::top_down(egui::Align::Min);
    let size = egui::vec2(300., 40.);
    let original = document(&serde_json::json!("original"));
    let (id, first, rect) = {
        let published = host.published.borrow();
        let (id, (content, rect)) = published.iter().next().unwrap();
        (*id, Rc::clone(content), *rect)
    };
    for phase in [Failure::Measure, Failure::Read, Failure::Publish] {
        host.events.borrow_mut().clear();
        host.failure.set(Some(phase));
        let error = draw(
            state,
            &document(&serde_json::json!("changed")),
            presentation,
            layout,
            size,
        )
        .0
        .unwrap_err();
        assert!(matches!(
            error
                .get_ref()
                .unwrap()
                .downcast_ref::<AttachmentHostError>(),
            Some(AttachmentHostError::BorrowConflict)
        ));
        let expected = match phase {
            Failure::Measure => vec![Event::Measure(id)],
            Failure::Read => vec![Event::Measure(id), Event::Read(id)],
            Failure::Publish => vec![Event::Measure(id), Event::Read(id), Event::Publish(id)],
        };
        assert_eq!(*host.events.borrow(), expected);
        assert!(Rc::ptr_eq(&host.published.borrow()[&id].0, &first));
        assert_eq!(host.published.borrow()[&id].1, rect);
        host.failure.set(None);
        host.events.borrow_mut().clear();
        draw(state, &original, presentation, layout, size)
            .0
            .unwrap();
        assert!(Rc::ptr_eq(host.admissions.borrow().last().unwrap(), &first));
        assert_eq!(
            *host.events.borrow(),
            [Event::Measure(id), Event::Read(id), Event::Publish(id)]
        );
    }
}

/// Adjacent background below a clipped control cannot activate any action.
#[test]
fn header_pointer_rejects_adjacent_clipped_background() {
    let pack = pack();
    let p = pack.presentation(BrandMode::Dark).unwrap();
    for index in 0..3 {
        let mut canvas = Canvas::new(&p);
        canvas.host.height.set(12.);
        canvas.frame(vec![]);
        let rect = canvas.host.holes.borrow()[index];
        let pos = egui::pos2(rect.center().x, rect.top() + 8.);
        canvas.frame(pointer(pos, true));
        assert!(
            canvas.frame(pointer(pos, false)).0.is_empty(),
            "action {index}"
        );
    }
}

/// A gesture at a previous slot cannot activate a control moved this frame.
#[test]
fn header_pointer_rejects_previous_slot_after_move() {
    let pack = pack();
    let p = pack.presentation(BrandMode::Dark).unwrap();
    for index in 0..3 {
        let mut canvas = Canvas::new(&p);
        canvas.frame(vec![]);
        let pos = canvas.host.holes.borrow()[index].center();
        canvas.frame(pointer(pos, true));
        canvas.host.offset.set(egui::vec2(0., 100.));
        assert!(
            canvas.frame(pointer(pos, false)).0.is_empty(),
            "action {index}"
        );
    }
}

/// Movement into a clipped control cannot admit a press begun on background.
#[test]
fn header_pointer_rejects_outside_press_released_inside() {
    let pack = pack();
    let p = pack.presentation(BrandMode::Dark).unwrap();
    for index in 0..3 {
        let mut canvas = Canvas::new(&p);
        canvas.host.height.set(12.);
        canvas.frame(vec![]);
        let rect = canvas.host.holes.borrow()[index];
        let outside = egui::pos2(rect.center().x, rect.top() + 8.);
        let inside = outside - egui::vec2(0., 2.);
        canvas.frame(pointer(outside, true));
        assert!(
            canvas.frame(pointer(inside, false)).0.is_empty(),
            "action {index}"
        );
    }
}

/// A press follows its control identity when the slot moves before release.
#[test]
fn header_pointer_accepts_inside_release_after_slot_move() {
    use maestro_web::export_html::template::HeaderAction;
    let pack = pack();
    let p = pack.presentation(BrandMode::Dark).unwrap();
    for (index, action) in [
        HeaderAction::ToggleThinking,
        HeaderAction::ToggleTools,
        HeaderAction::DownloadSessionJson,
    ]
    .into_iter()
    .enumerate()
    {
        let mut canvas = Canvas::new(&p);
        canvas.frame(vec![]);
        let pos = canvas
            .host
            .holes
            .borrow()
            .iter()
            .rev()
            .nth(index)
            .unwrap()
            .center();
        canvas.frame(pointer(pos, true));
        canvas.host.offset.set(egui::vec2(0., 100.));
        assert_eq!(
            canvas.frame(pointer(pos + egui::vec2(0., 100.), false)).0,
            [action]
        );
    }
}

/// Releasing on nearby background rejects the gesture and consumes its ownership.
#[test]
fn header_pointer_rejects_inside_press_released_outside() {
    let pack = pack();
    let p = pack.presentation(BrandMode::Dark).unwrap();
    for index in 0..3 {
        let mut canvas = Canvas::new(&p);
        canvas.host.height.set(12.);
        canvas.frame(vec![]);
        let rect = canvas.host.holes.borrow()[index];
        let outside = egui::pos2(rect.center().x, rect.top() + 8.);
        let inside = outside - egui::vec2(0., 2.);
        canvas.frame(pointer(inside, true));
        assert!(
            canvas.frame(pointer(outside, false)).0.is_empty(),
            "action {index}"
        );
        assert!(canvas.frame(pointer(inside, false)).0.is_empty());
    }
}

/// Keyboard activation remains independent of pointer location.
#[test]
fn header_keyboard_activation_ignores_pointer_geometry() {
    use maestro_web::export_html::template::HeaderAction;
    let pack = pack();
    let p = pack.presentation(BrandMode::Dark).unwrap();
    let mut canvas = Canvas::new(&p);
    canvas.frame(vec![]);
    let key = |key, pressed| egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    canvas.frame(vec![
        egui::Event::PointerMoved(egui::pos2(700., 500.)),
        key(egui::Key::Tab, true),
    ]);
    canvas.frame(vec![key(egui::Key::Tab, false)]);
    assert_eq!(
        canvas.frame(vec![key(egui::Key::Enter, true)]).0,
        [HeaderAction::ToggleThinking]
    );
}

/// A disabled caller suppresses every header control's pointer activation.
#[test]
fn header_pointer_rejects_disabled_controls() {
    let pack = pack();
    let p = pack.presentation(BrandMode::Dark).unwrap();
    for (index, (press_enabled, release_enabled)) in
        (0..3).flat_map(|i| [(false, false), (false, true), (true, false)].map(|ends| (i, ends)))
    {
        let mut canvas = Canvas::new(&p);
        canvas.frame_with(vec![], press_enabled, |_| {});
        let pos = canvas.host.holes.borrow()[index].center();
        assert!(
            canvas
                .frame_with(pointer(pos, true), press_enabled, |_| {})
                .0
                .is_empty()
        );
        assert!(
            canvas
                .frame_with(pointer(pos, false), release_enabled, |_| {})
                .0
                .is_empty(),
            "action {index}"
        );
    }
}

/// Complete disclosure preparation changes the retained factory, not raw unknown fields.
#[test]
fn header_native_disclosures_use_owned_preparation() {
    let host = Rc::new(Host::default());
    host.height.set(120.);
    let mut state = HeaderState::new(host.clone());
    let pack = pack();
    let presentation = pack.presentation(BrandMode::Dark).unwrap();
    let mut doc = document(&serde_json::json!("disclosure owner"));
    let mut show = |doc: &maestro_web::export_html::template::SessionDocument| {
        draw(
            &mut state,
            doc,
            &presentation,
            egui::Layout::top_down(egui::Align::Min),
            egui::vec2(300., 40.),
        )
        .0
        .unwrap();
        Rc::clone(&host.published.borrow().values().next().unwrap().0)
    };
    let original = show(&doc);
    doc.data
        .insert("systemPrompt".into(), serde_json::json!("changed prompt"));
    let prompt = show(&doc);
    assert!(!Rc::ptr_eq(&original, &prompt));
    doc.data.insert("tools".into(),serde_json::json!([{"name":"owned <tool>","description":"d","parameters":{"properties":{"x":{"type":"string"}}}}]));
    let tools = show(&doc);
    assert!(!Rc::ptr_eq(&prompt, &tools));
    doc.data["tools"][0]["parameters"]["properties"]["x"]["description"] =
        serde_json::json!("changed parameter");
    let parameter = show(&doc);
    assert!(!Rc::ptr_eq(&tools, &parameter));
    doc.data["tools"][0]["unknown"] = serde_json::json!({"kept":true});
    assert!(Rc::ptr_eq(&parameter, &show(&doc)));
    drop(doc);
    assert!(Rc::ptr_eq(
        &parameter,
        &host.published.borrow().values().next().unwrap().0
    ));
}

/// Reached preparation and transaction failures leave the published factory intact.
#[test]
fn header_native_disclosure_failures_keep_published_owner() {
    let host = Rc::new(Host::default());
    host.height.set(120.);
    let mut state = HeaderState::new(host.clone());
    let pack = pack();
    let presentation = pack.presentation(BrandMode::Dark).unwrap();
    let layout = egui::Layout::top_down(egui::Align::Min);
    let size = egui::vec2(300., 40.);
    let mut doc = document(&serde_json::json!("published"));
    draw(&mut state, &doc, &presentation, layout, size)
        .0
        .unwrap();
    let published = Rc::clone(&host.published.borrow().values().next().unwrap().0);
    host.events.borrow_mut().clear();
    doc.data
        .insert("systemPrompt".into(), serde_json::json!(true));
    assert_eq!(
        draw(&mut state, &doc, &presentation, layout, size)
            .0
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidData
    );
    assert!(host.events.borrow().is_empty());
    doc.data
        .insert("systemPrompt".into(), serde_json::json!("changed"));
    for failure in [Failure::Measure, Failure::Read, Failure::Publish] {
        host.failure.set(Some(failure));
        assert!(
            draw(&mut state, &doc, &presentation, layout, size)
                .0
                .is_err()
        );
        assert!(Rc::ptr_eq(
            &published,
            &host.published.borrow().values().next().unwrap().0
        ));
    }
    host.failure.set(None);
    doc.data
        .insert("systemPrompt".into(), serde_json::json!("admitted prompt"));
    draw(&mut state, &doc, &presentation, layout, size)
        .0
        .unwrap();
    assert!(Rc::ptr_eq(
        &published,
        &host.published.borrow().values().next().unwrap().0
    ));
}
