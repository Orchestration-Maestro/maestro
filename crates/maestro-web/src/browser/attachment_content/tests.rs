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
    let record = std::rc::Rc::new(std::cell::RefCell::new(crate::Attachment {
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
