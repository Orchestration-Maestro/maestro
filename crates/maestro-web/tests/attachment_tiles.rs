//! Attachment tiles exercised through actual drawing and pointer input.
#![cfg(test)]
/// Controlled drawing and platform effects.
use crate::attachment_support;
use attachment_support::*;
use maestro_web::{AttachmentTile, browser::attachment_content::ImageFit};
use serde::Deserialize;
use std::rc::Rc;
/// Expected tile output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Expected {
    /// Complete preview URL.
    preview: Option<String>,
    /// Catalog glyph.
    icon: Option<String>,
    /// Complete clipped label.
    label: Option<String>,
    /// PDF-only badge.
    pdf_badge: bool,
    /// Remove control presence.
    remove: bool,
}
/// One source-derived tile case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Case {
    /// Case identity used for assertion context.
    name: String,
    /// Original attachment.
    input: Input,
    /// Whether deletion is enabled.
    show_delete: bool,
    /// Complete expected projection.
    expected: Expected,
}
/// Select distinct observations without duplicating each fixture's owner.
fn cases() -> Vec<Case> {
    serde_json::from_str(include_str!("fixtures/attachment_tiles.json")).unwrap()
}
#[test]
fn tile_preview_preserves_mime_badge_and_filename() {
    for c in cases().into_iter().filter(|c| c.expected.preview.is_some()) {
        let record = c.input.record();
        let name = record.borrow().file_name.clone();
        let host = Rc::new(Host::default());
        let mut tile = AttachmentTile::new(record, host.clone());
        tile.show_delete = c.show_delete;
        let mut frame = Frame::default();
        frame.tile(&mut tile, vec![]).unwrap();
        assert!(
            matches!(&host.surfaces.borrow().last().unwrap().content,Content::Image(s,alt,ImageFit::Cover) if Some(s)==c.expected.preview.as_ref()&&alt==&name),
            "{}",
            c.name
        );
        let occluders = host.surfaces.borrow().last().unwrap().occluders.clone();
        assert_eq!(occluders.len(), usize::from(c.expected.pdf_badge));
        if c.expected.pdf_badge {
            assert_eq!(occluders[0].size(), egui::vec2(64., 19.));
            let badge_pos = occluders[0].center();
            frame
                .tile(&mut tile, vec![egui::Event::PointerMoved(badge_pos)])
                .unwrap();
            assert_eq!(
                host.surfaces.borrow().last().unwrap().opacity.to_bits(),
                1_f32.to_bits()
            );
            assert!(frame.tile(&mut tile, click(badge_pos)).unwrap().is_none());
            let image_pos = occluders[0].left_top() - egui::vec2(-8., 8.);
            frame
                .tile(&mut tile, vec![egui::Event::PointerMoved(image_pos)])
                .unwrap();
            assert_eq!(
                host.surfaces.borrow().last().unwrap().opacity.to_bits(),
                0.8_f32.to_bits()
            );
            assert!(frame.tile(&mut tile, click(image_pos)).unwrap().is_some());
        }
        assert_eq!(
            frame.texts().contains(&"PDF".into()),
            c.expected.pdf_badge,
            "{}",
            c.name
        );
    }
}
#[test]
fn tile_fallback_keeps_its_own_spreadsheet_rules() {
    for c in cases().into_iter().filter(|c| c.expected.icon.is_some()) {
        let host = Rc::new(Host::default());
        let mut tile = AttachmentTile::new(c.input.record(), host.clone());
        let mut frame = Frame::default();
        frame.tile(&mut tile, vec![]).unwrap();
        let catalog = maestro_web::icons::icons().unwrap();
        let glyph = maestro_web::icons::icon(
            &catalog[c.expected.icon.as_ref().unwrap()],
            maestro_web::icons::IconSize::Md,
            egui::Color32::from_hex(frame.pack.presentation(frame.mode).unwrap().colors["muted"])
                .unwrap(),
            1.,
        )
        .unwrap();
        assert!(
            frame
                .images
                .iter()
                .any(|image| image.pixels == glyph.pixels),
            "{}",
            c.name
        );
        assert!(host.surfaces.borrow().is_empty());
    }
}
#[test]
fn tile_filename_preserves_utf16_length_boundaries() {
    for c in cases().into_iter().filter(|c| c.expected.label.is_some()) {
        let mut tile = AttachmentTile::new(c.input.record(), Rc::new(Host::default()));
        let mut frame = Frame::default();
        frame.tile(&mut tile, vec![]).unwrap();
        assert_eq!(frame.texts(), vec![c.expected.label.unwrap()], "{}", c.name);
    }
}
#[test]
fn tile_hover_and_remove_keep_pointer_capabilities() {
    let record = attachment();
    record.borrow_mut().preview = Some("thumb".into());
    let host = Rc::new(Host::default());
    host.hover.set(true);
    let mut tile = AttachmentTile::new(record, host.clone());
    tile.show_delete = true;
    let mut frame = Frame::default();
    frame.tile(&mut tile, vec![]).unwrap();
    let initial = host.surfaces.borrow().last().unwrap().clone();
    assert_eq!(initial.rect.size(), egui::vec2(64., 64.));
    assert_eq!(initial.opacity.to_bits(), (1.0_f32).to_bits());
    assert!(frame.images.is_empty());
    assert!(initial.occluders.is_empty());
    frame
        .tile(
            &mut tile,
            vec![egui::Event::PointerMoved(initial.rect.center())],
        )
        .unwrap();
    assert_eq!(
        host.surfaces.borrow().last().unwrap().opacity.to_bits(),
        (0.8_f32).to_bits()
    );
    assert_eq!(frame.images.len(), 1);
    assert_eq!(
        host.surfaces.borrow().last().unwrap().occluders,
        vec![egui::Rect::from_min_size(
            initial.rect.right_top() + egui::vec2(-16., -4.),
            egui::vec2(20., 20.)
        )]
    );
    host.hover.set(false);
    frame
        .tile(&mut tile, vec![egui::Event::PointerGone])
        .unwrap();
    assert_eq!(
        host.surfaces.borrow().last().unwrap().opacity.to_bits(),
        (1.0_f32).to_bits()
    );
    assert_eq!(frame.images.len(), 1);
    tile.show_delete = false;
    frame.tile(&mut tile, vec![]).unwrap();
    assert!(frame.images.is_empty());
}
#[test]
fn tile_click_opens_with_the_same_attachment_handle() {
    let record = attachment();
    let host = Rc::new(Host::default());
    let mut tile = AttachmentTile::new(record.clone(), host.clone());
    let mut frame = Frame::default();
    frame.tile(&mut tile, vec![]).unwrap();
    let mut overlay = frame
        .tile(&mut tile, click(egui::pos2(32., 32.)))
        .unwrap()
        .expect("tile opens");
    record.borrow_mut().extracted_text = Some("edited after opening".into());
    drop(tile);
    frame.overlay(&mut overlay, vec![]).unwrap();
    assert!(
        matches!(&host.surfaces.borrow().last().unwrap().content,Content::Text(s,_) if s=="edited after opening")
    );
}
#[test]
fn tile_delete_does_not_open_the_overlay() {
    let calls = Rc::new(std::cell::Cell::new(0));
    for c in cases().into_iter().filter(|c| c.expected.remove) {
        let mut tile = AttachmentTile::new(c.input.record(), Rc::new(Host::default()));
        tile.show_delete = c.show_delete;
        let count = calls.clone();
        tile.on_delete = Some(Rc::new(move || count.set(count.get() + 1)));
        let mut frame = Frame::default();
        frame.tile(&mut tile, vec![]).unwrap();
        let before = calls.get();
        assert!(
            frame
                .tile(&mut tile, click(egui::pos2(58., 6.)))
                .unwrap()
                .is_none()
        );
        assert_eq!(calls.get(), before + 1, "{}", c.name);
        tile.on_delete = None;
        assert!(
            frame
                .tile(&mut tile, click(egui::pos2(58., 6.)))
                .unwrap()
                .is_none()
        );
        assert_eq!(calls.get(), before + 1);
        tile.show_delete = false;
        assert!(
            frame
                .tile(&mut tile, click(egui::pos2(32., 32.)))
                .unwrap()
                .is_some()
        );
        assert_eq!(calls.get(), before + 1);
    }
    assert!(calls.get() > 0);
}
#[test]
fn tile_replacement_reuses_and_releases_dom_surfaces() {
    let record = attachment();
    record.borrow_mut().preview = Some("first".into());
    let host = Rc::new(Host::default());
    let mut tile = AttachmentTile::new(record, host.clone());
    let mut frame = Frame::default();
    frame.tile(&mut tile, vec![]).unwrap();
    let id = host.surfaces.borrow()[0].id;
    let next = attachment();
    next.borrow_mut().preview = Some("second".into());
    tile.attachment = next.clone();
    frame.tile(&mut tile, vec![]).unwrap();
    let second = host.surfaces.borrow().last().unwrap().clone();
    assert_eq!(second.id, id);
    assert!(matches!(second.content,Content::Image(s,_,_) if s.ends_with(",second")));
    next.borrow_mut().preview = None;
    let n = host.effects.borrow().len();
    frame.tile(&mut tile, vec![]).unwrap();
    assert_eq!(&host.effects.borrow()[n..], &[format!("remove:{id:?}")]);
    let n = host.effects.borrow().len();
    drop(tile);
    assert_eq!(&host.effects.borrow()[n..], &[format!("remove:{id:?}")]);
}
#[test]
fn tile_wide_caption_is_elided_inside_padded_bounds() {
    let record = attachment();
    record.borrow_mut().file_name = "WWWWWWWWWW".into();
    let mut tile = AttachmentTile::new(record, Rc::new(Host::default()));
    let mut frame = Frame::default();
    frame.tile(&mut tile, vec![]).unwrap();
    let text = frame
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some((s.clip_rect, t)),
            _ => None,
        })
        .unwrap();
    assert!(text.1.galley.elided);
    assert!(text.1.galley.size().x <= 48.);
    assert!(
        egui::Rect::from_min_max(egui::pos2(8., 8.), egui::pos2(56., 56.)).contains_rect(text.0)
    );
}
#[test]
fn tile_remove_keeps_visible_brand_boundary() {
    let record = attachment();
    record.borrow_mut().mime_type = "application/pdf".into();
    record.borrow_mut().preview = Some("pdf".into());
    let mut tile = AttachmentTile::new(record, Rc::new(Host::default()));
    tile.show_delete = true;
    let mut frame = Frame::default();
    frame.tile(&mut tile, vec![]).unwrap();
    let p = frame.pack.presentation(frame.mode).unwrap();
    let border = egui::Color32::from_hex(p.colors["border"]).unwrap();
    assert!(frame.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Circle(c) if c.radius.to_bits() == 10_f32.to_bits() && c.stroke.color == border && c.stroke.width.to_bits() == 1_f32.to_bits())));
}
#[test]
fn tile_badge_tessellation_shares_tile_contour_in_both_packs() {
    for pack in ["packs/signal.json", "brand.json"] {
        let record = attachment();
        record.borrow_mut().mime_type = "application/pdf".into();
        record.borrow_mut().preview = Some("pdf".into());
        let host = Rc::new(Host::default());
        let mut tile = AttachmentTile::new(record, host.clone());
        let mut frame = Frame {
            pack: maestro_theme::load_brand_pack(
                &format!("{}/../../assets/brand/{pack}", env!("CARGO_MANIFEST_DIR")),
                &maestro_theme::theme::NativeThemeOperations,
            )
            .unwrap(),
            ..Frame::default()
        };
        frame.tile(&mut tile, vec![]).unwrap();
        let background = egui::Color32::from_hex(
            frame.pack.presentation(frame.mode).unwrap().colors["background"],
        )
        .unwrap();
        let badge = host.surfaces.borrow().last().unwrap().occluders[0];
        let actual = frame
            .shapes
            .iter()
            .find(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.fill == background))
            .unwrap()
            .clone();
        let mut outline = frame
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Rect(r) if r.rect.size() == egui::vec2(64., 64.) => Some(r.clone()),
                _ => None,
            })
            .unwrap();
        outline.stroke = egui::Stroke::NONE;
        outline.fill = background;
        let expected = egui::epaint::ClippedShape {
            clip_rect: badge,
            shape: egui::Shape::Rect(outline),
        };
        let actual = frame.ctx.tessellate(vec![actual], 1.);
        let expected = frame.ctx.tessellate(vec![expected], 1.);
        assert_eq!(actual.len(), 1, "{pack}");
        assert_eq!(expected.len(), 1, "{pack}");
        let clip = actual[0].clip_rect;
        let egui::epaint::Primitive::Mesh(actual) = &actual[0].primitive else {
            panic!("badge must tessellate to triangles");
        };
        let egui::epaint::Primitive::Mesh(expected) = &expected[0].primitive else {
            panic!("tile must tessellate to triangles");
        };
        assert!(!actual.indices.is_empty());
        assert_eq!(actual.indices, expected.indices, "{pack}");
        assert_eq!(actual.vertices, expected.vertices, "{pack}");
        assert_eq!(clip, badge, "badge region: {pack}");
    }
}

#[test]
fn user_attachment_tiles_wrap_as_single_fixed_items() {
    use maestro_web::{UserMessage, UserMessageInput};
    let host = Rc::new(Host::default());
    let records: Vec<_> = (0..14)
        .map(|index| {
            let record = attachment();
            record.borrow_mut().file_name = format!("wrapped-{index}.png");
            record.borrow_mut().preview = Some("AA==".into());
            record
        })
        .collect();
    let content = maestro_app::UserContent::Text(String::new());
    let mut row = UserMessage::new(host.clone());
    let mut frame = Frame::default();
    let input = UserMessageInput::WithAttachments {
        content: &content,
        attachments: Some(&records),
    };
    let output =
        crate::document_card_support::frame_owner(&mut frame, &host, vec![], |ui, view| {
            row.render(ui, input, view)
        });
    output.response.unwrap();
    assert!(output.overlays.is_empty());
    let tiles: Vec<_> = host
        .surfaces
        .borrow()
        .iter()
        .filter(|s| matches!(s.content, Content::Image(..)))
        .map(|s| s.rect)
        .collect();
    assert_eq!(tiles.len(), 14);
    for rect in &tiles {
        assert_eq!(rect.size(), egui::vec2(64., 64.));
        assert!(frame.screen.contains_rect(*rect));
    }
    assert!(tiles[13].top() > tiles[0].bottom());
    let output = crate::document_card_support::frame_owner(
        &mut frame,
        &host,
        click(tiles[13].center()),
        |ui, view| row.render(ui, input, view),
    );
    output.response.unwrap();
    assert_eq!(output.overlays.len(), 1);
}
