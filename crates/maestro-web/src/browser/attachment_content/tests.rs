//! Native CSS projection assertions independent of a browser engine.
use super::{AttachmentContent, AttachmentSurface, ImageFit, placement::clip_path};
#[test]
fn browser_content_clip_paths_preserve_outer_rect_and_control_holes() {
    let surface = AttachmentSurface {
        id: egui::Id::new("clipping"),
        rect: egui::Rect::from_min_size(egui::pos2(10.25, 20.5), egui::vec2(64., 64.)),
        clip_rect: egui::Rect::from_min_max(egui::pos2(12.25, 22.5), egui::pos2(72.25, 82.5)),
        pixels_per_point: 1.5,
        opacity: 1.,
        occluders: vec![
            egui::Rect::from_min_size(egui::pos2(58.25, 16.5), egui::vec2(20., 20.)),
            egui::Rect::from_min_size(egui::pos2(10.25, 65.5), egui::vec2(64., 19.)),
        ],
        content: AttachmentContent::Image {
            source: "original",
            alt: "name",
            fit: ImageFit::Cover,
        },
    };
    assert_eq!(
        clip_path(&surface, (0.75, 0.5)),
        "path(evenodd,\"M1.5 1H46.5V31H1.5ZM36 1H46.5V8H36ZM1.5 22.5H46.5V31H1.5Z\")"
    );
}
#[test]
fn native_content_borders_belong_to_visible_owners() {
    for fit in [ImageFit::Cover, ImageFit::Contain] {
        let css = super::content_border(&AttachmentContent::Image {
            source: "image",
            alt: "name",
            fit,
        });
        assert_eq!(
            css.contains("border:1px solid var(--maestro-color-border)"),
            fit == ImageFit::Cover
        );
    }
    let record = std::sync::Arc::new(std::sync::RwLock::new(crate::Attachment {
        id: "id".into(),
        r#type: maestro_app::browser::utils::attachment_utils::AttachmentType::Document,
        file_name: "file".into(),
        mime_type: "application/pdf".into(),
        size: 0,
        content: String::new(),
        extracted_text: None,
        preview: None,
    }));
    for file_type in [
        super::FileType::Pdf,
        super::FileType::Docx,
        super::FileType::Excel,
    ] {
        let css = super::content_border(&AttachmentContent::Preview {
            file_type,
            attachment: record.clone(),
            reload: false,
        });
        assert_eq!(
            css.contains("border:1px solid var(--maestro-color-border)"),
            file_type != super::FileType::Excel
        );
    }
}
#[test]
fn placements_write_once_and_modal_changes_refresh_siblings_once() {
    let ids: Vec<_> = (0..20).map(egui::Id::new).collect();
    let mut writes = 0;
    let visits = std::cell::Cell::new(0);
    for id in &ids {
        let retained = ids
            .iter()
            .copied()
            .inspect(|_| visits.set(visits.get() + 1));
        let targets: Vec<_> = super::style_targets(retained, Some(*id), None, None).collect();
        assert_eq!(targets, vec![*id]);
        writes += targets.len();
    }
    assert_eq!(writes, 20);
    assert_eq!(visits.get(), 0);
    assert_eq!(
        super::style_targets(ids.iter().copied(), None, None, Some(ids[0])).collect::<Vec<_>>(),
        ids
    );
    assert!(
        super::style_targets(ids.iter().copied(), None, Some(ids[0]), Some(ids[0]))
            .next()
            .is_none()
    );
    assert_eq!(
        super::style_targets(ids.iter().copied(), None, Some(ids[0]), Some(ids[1]))
            .collect::<Vec<_>>(),
        ids
    );
    assert_eq!(
        super::style_targets(ids.iter().copied(), None, Some(ids[1]), None).collect::<Vec<_>>(),
        ids
    );
}

#[test]
fn reading_geometry_shares_fractional_axis_transform() {
    let transform =
        super::placement::Transform::new([41.5, 19.25, 250., 75.], [200, 100], 1.).unwrap();
    assert_eq!(
        transform.position(egui::pos2(10., 20.)).map(f64::to_bits),
        [54_f64, 34.25].map(f64::to_bits)
    );
    assert!((transform.height(90.) - 120.).abs() < f32::EPSILON);
    assert_eq!(
        transform.relative([54., 34.25, 30., 15.], [41.5, 19.25]),
        egui::Rect::from_min_size(egui::pos2(10., 20.), egui::vec2(24., 20.))
    );
    let unit = super::placement::Transform::new([0., 0., 100., 100.], [100, 100], 1.).unwrap();
    assert!((unit.height(90.) - 90.).abs() < f32::EPSILON);
    assert_eq!(
        unit.position(egui::pos2(10., 20.)).map(f64::to_bits),
        [10_f64, 20.].map(f64::to_bits)
    );
    for physical in [[0, 100], [200, 0]] {
        assert!(super::placement::Transform::new([0., 0., 1., 1.], physical, 1.).is_err());
    }
}

#[test]
fn reading_modal_style_targets_keep_reading_identity() {
    let reading = egui::Id::new("reading");
    let modal = egui::Id::new("modal");
    let other = egui::Id::new("other-modal");
    let ids = [reading, modal, other];
    assert_eq!(
        super::visibility_style(reading, None, None),
        "visibility:visible;z-index:1;"
    );
    assert_eq!(
        super::visibility_style(reading, Some(modal), Some(modal)),
        "visibility:visible;z-index:51;"
    );
    assert_eq!(
        super::visibility_style(reading, None, Some(reading)),
        "visibility:visible;z-index:51;"
    );
    assert_eq!(
        super::visibility_style(reading, Some(modal), Some(other)),
        "visibility:hidden;z-index:1;"
    );
    assert_eq!(
        super::visibility_style(reading, Some(modal), None),
        "visibility:visible;z-index:1;"
    );
    assert_eq!(
        super::style_targets(ids.into_iter(), Some(reading), None, None).collect::<Vec<_>>(),
        [reading]
    );
}

#[test]
fn reading_frame_clip_extents_keep_finite_bounds() {
    let reading = egui::Id::new("finite-reading");
    let content = super::ReadingContent {
        #[cfg(target_arch = "wasm32")]
        mount: std::rc::Rc::new(|_| unreachable!()),
    };
    let repaint = egui::Context::default();
    let surface = AttachmentSurface {
        id: reading,
        rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(50., 40.)),
        clip_rect: egui::Rect::EVERYTHING,
        pixels_per_point: 1.,
        opacity: 1.,
        occluders: vec![],
        content: AttachmentContent::Reading(super::ReadingPlacement {
            intrinsic_width: false,
            content: std::rc::Rc::new(content),
            phase: super::ReadingPhase::Publish,
            sizes: &[],
            modal: None,
            context: &repaint,
        }),
    };
    let drawable = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
    assert_eq!(
        super::placement::frame_clip_path(&surface, drawable),
        "path(evenodd,\"M0 0H800V600H0Z\")"
    );
    let mut surface = surface;
    surface.rect = egui::Rect::from_min_size(egui::pos2(-12.5, 18.25), egui::vec2(50., 40.));
    surface.occluders = vec![
        egui::Rect::from_min_max(egui::pos2(10., 20.), egui::pos2(30., 40.)),
        egui::Rect::from_min_max(egui::pos2(900., 700.), egui::pos2(950., 750.)),
    ];
    assert_eq!(
        super::placement::frame_clip_path(&surface, drawable),
        "path(evenodd,\"M12.5 -18.25H812.5V581.75H12.5ZM22.5 1.75H42.5V21.75H22.5Z\")"
    );
    surface.clip_rect = surface.rect;
    assert_eq!(
        super::placement::frame_clip_path(&surface, drawable),
        "path(evenodd,\"M12.5 0H50V40H12.5ZM22.5 1.75H42.5V21.75H22.5Z\")"
    );
    surface.clip_rect = egui::Rect::NOTHING;
    assert_eq!(
        super::placement::frame_clip_path(&surface, drawable),
        "path(evenodd,\"M0 0H0V0H0Z\")"
    );
    surface.clip_rect = egui::Rect::EVERYTHING;
    for size in [egui::vec2(0., 40.), egui::vec2(50., 0.)] {
        surface.rect = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
        assert_eq!(
            super::placement::frame_clip_path(&surface, drawable),
            "path(evenodd,\"M0 0H0V0H0Z\")"
        );
    }
}
