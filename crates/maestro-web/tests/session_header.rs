//! Public identity drawing through the replaceable native reading boundary.
#![cfg(test)]
mod session_header_support;
use maestro_theme::BrandMode;
use maestro_web::browser::attachment_content::AttachmentUiError;
use maestro_web::export_html::template::HeaderState;
use session_header_support::{Event, Failure, Host, document, draw, pack};
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
    let original = document(&serde_json::json!("original"));
    draw(&mut state, &original, &presentation, layout, size)
        .0
        .unwrap();
    let (id, first, rect) = {
        let published = host.published.borrow();
        let (id, (content, rect)) = published.iter().next().unwrap();
        (*id, Rc::clone(content), *rect)
    };
    for phase in [Failure::Measure, Failure::Read, Failure::Publish] {
        host.events.borrow_mut().clear();
        host.failure.set(Some(phase));
        let error = draw(
            &mut state,
            &document(&serde_json::json!("changed")),
            &presentation,
            layout,
            size,
        )
        .0
        .unwrap_err();
        assert!(matches!(
            error.get_ref().unwrap().downcast_ref::<AttachmentUiError>(),
            Some(AttachmentUiError::BorrowConflict)
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
        draw(&mut state, &original, &presentation, layout, size)
            .0
            .unwrap();
        assert!(Rc::ptr_eq(host.admissions.borrow().last().unwrap(), &first));
        assert_eq!(
            *host.events.borrow(),
            [Event::Measure(id), Event::Read(id), Event::Publish(id)]
        );
    }
}
/// Selected properties reach the native boundary without mutating caller style.
#[test]
fn header_scope_uses_shared_theme_and_preserves_caller_style() {
    let default = pack();
    let alternate = maestro_theme::load_brand_pack(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/brand/packs/signal.json"
        ),
        &maestro_theme::theme::NativeThemeOperations,
    )
    .unwrap();
    for pack in [&default, &alternate] {
        for mode in [BrandMode::Light, BrandMode::Dark] {
            let presentation = pack.presentation(mode).unwrap();
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
