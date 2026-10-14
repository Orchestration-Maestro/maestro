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
