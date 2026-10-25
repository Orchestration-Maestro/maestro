//! Embedding surfaces, ownership and supplied presentation roles.
#![cfg(test)]
/// Controlled platform and genuine drawing fixtures.
use crate::attachment_support;
use attachment_support::*;
use maestro_web::{AttachmentOverlay, AttachmentTile};
use std::rc::Rc;
#[test]
fn attachment_surfaces_keep_fractional_rects_and_owned_content() {
    let host = Rc::new(Host::default());
    let record = attachment();
    record.write().unwrap().preview = Some("owned bytes".into());
    let mut tile = AttachmentTile::new(record, host.clone());
    let mut frame = Frame {
        screen: egui::Rect::from_min_size(egui::pos2(5.25, 12.5), egui::vec2(800., 600.)),
        density: 1.5,
        ..Frame::default()
    };
    frame.tile(&mut tile, vec![]).unwrap();
    let s = host.surfaces.borrow()[0].clone();
    drop(tile);
    assert_eq!(s.rect.min, egui::pos2(5.25, 12.5));
    assert_eq!(s.rect.max, egui::pos2(69.25, 76.5));
    assert_eq!(s.density.to_bits(), 1.5_f32.to_bits());
    assert!(s.clip.contains_rect(s.rect));
    assert!(
        matches!(s.content,Content::Image(ref text,_,_) if text=="data:image/png;base64,owned bytes")
    );
}
#[test]
fn attachment_modal_hides_and_restores_existing_surfaces() {
    let host = Rc::new(Host::default());
    let record = attachment();
    record.write().unwrap().preview = Some("animated-original".into());
    let mut tile = AttachmentTile::new(record.clone(), host.clone());
    let mut frame = Frame::default();
    frame.tile(&mut tile, vec![]).unwrap();
    let tile_id = host.surfaces.borrow()[0].id;
    let mut overlay = AttachmentOverlay::open(record, None, host.clone());
    let n = host.effects.borrow().len();
    frame.overlay(&mut overlay, vec![]).unwrap();
    let overlay_id = host.surfaces.borrow().last().unwrap().id;
    assert_ne!(tile_id, overlay_id);
    assert_eq!(
        &host.effects.borrow()[n..],
        &[format!("modal:{overlay_id:?}:true")]
    );
    let n = host.effects.borrow().len();
    drop(overlay);
    assert_eq!(
        &host.effects.borrow()[n..],
        &[
            format!("remove:{overlay_id:?}"),
            format!("modal:{overlay_id:?}:false")
        ]
    );
    frame.tile(&mut tile, vec![]).unwrap();
    let restored = host.surfaces.borrow().last().unwrap().clone();
    assert_eq!(restored.id, tile_id);
    assert!(
        matches!(restored.content,Content::Image(ref source,_,_) if source.ends_with(",animated-original"))
    );
}
#[test]
fn attachment_styles_follow_caller_brand_pack() {
    for (path, mode) in [
        (
            "/../../assets/brand/brand.json",
            maestro_theme::BrandMode::Dark,
        ),
        (
            "/../../assets/brand/brand.json",
            maestro_theme::BrandMode::Light,
        ),
        (
            "/../../assets/brand/packs/signal.json",
            maestro_theme::BrandMode::Dark,
        ),
    ] {
        let host = Rc::new(Host::default());
        let record = attachment();
        record.write().unwrap().preview = Some("unchanged-pixels".into());
        let mut tile = AttachmentTile::new(record.clone(), host.clone());
        let mut frame = Frame {
            pack: crate::support::brand::pack(&format!("{}{path}", env!("CARGO_MANIFEST_DIR")))
                .unwrap(),
            mode,
            ..Frame::default()
        };
        frame.tile(&mut tile, vec![]).unwrap();
        let p = frame.pack.presentation(mode).unwrap();
        let s = host.surfaces.borrow().last().unwrap().clone();
        assert_eq!(s.css, p.css_properties());
        assert!(
            matches!(s.content,Content::Image(ref data,_,_) if data.ends_with(",unchanged-pixels"))
        );
        let panel = egui::Color32::from_hex(p.colors["panel"]).unwrap();
        assert!(frame.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Rect(r) if r.fill==panel&&f64::from(r.corner_radius.nw).to_bits()==p.radii["panel"].to_bits())));
        record.write().unwrap().preview = None;
        frame.tile(&mut tile, vec![]).unwrap();
        let p = frame.pack.presentation(mode).unwrap();
        let muted = egui::Color32::from_hex(p.colors["muted"]).unwrap();
        assert!(frame.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(text) if text.galley.job.text=="note.txt"&&text.galley.job.sections[0].format.color==muted)));
        record.write().unwrap().mime_type = "application/pdf".into();
        let mut overlay = AttachmentOverlay::open(record, None, host);
        frame.overlay(&mut overlay, vec![]).unwrap();
        frame.overlay(&mut overlay, vec![]).unwrap();
        let p = frame.pack.presentation(mode).unwrap();
        let center = frame.label("PDF").center();
        let accent = egui::Color32::from_hex(p.colors["accent"]).unwrap();
        let selected_text = egui::Color32::from_hex(p.colors["on-accent"]).unwrap();
        assert!(frame.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == "PDF" && t.override_text_color.unwrap_or_else(|| { let color = t.galley.job.sections[0].format.color; if color == egui::Color32::PLACEHOLDER { t.fallback_color } else { color } }) == selected_text)));
        assert!(frame.shapes.iter().any(
            |s| matches!(&s.shape,egui::Shape::Rect(r) if r.rect.contains(center)&&r.fill==accent)
        ));
    }
}
#[test]
fn attachment_widgets_keep_the_public_embedding_paths() {
    let record = attachment();
    let host = Rc::new(Host::default());
    let mut tile: maestro_web::AttachmentTile = AttachmentTile::new(record.clone(), host.clone());
    assert!(!tile.show_delete);
    assert!(tile.on_delete.is_none());
    let mut frame = Frame::default();
    let guard = record.write().unwrap();
    assert!(matches!(
        frame.tile(&mut tile, vec![]),
        Err(maestro_web::dialogs::attachment_overlay::AttachmentUiError::BorrowConflict)
    ));
    assert!(host.surfaces.borrow().is_empty());
    drop(guard);
    frame.tile(&mut tile, vec![]).unwrap();
    let mut overlay: maestro_web::AttachmentOverlay = frame
        .tile(&mut tile, click(egui::pos2(32., 32.)))
        .unwrap()
        .unwrap();
    assert!(frame.overlay(&mut overlay, vec![]).unwrap());
    assert!(matches!(
        host.surfaces.borrow().last().unwrap().content,
        Content::Text(_, maestro_web::browser::attachment_content::TextView::Plain)
    ));
}

#[test]
fn attachment_color_errors_use_shared_presentation_route() {
    use maestro_web::dialogs::attachment_overlay::AttachmentUiError;
    let host = Rc::new(Host::default());
    let record = attachment();
    record.write().unwrap().preview = Some("preview".into());
    let mut tile = AttachmentTile::new(record, host.clone());
    let mut overlay = AttachmentOverlay::open(attachment(), None, host.clone());
    let mut frame = Frame::default();
    let tile_result = frame.run(vec![], |ctx, i, p| {
        let mut p = p.clone();
        p.colors.insert("background", "not-a-color");
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("bad-color"),
            egui::UiBuilder::new().max_rect(ctx.content_rect()),
        );
        tile.render(&mut ui, i, &p)
    });
    assert!(matches!(
        tile_result,
        Err(AttachmentUiError::Presentation(_))
    ));
    assert!(host.surfaces.borrow().is_empty());
    let overlay_result = frame.run(vec![], |ctx, i, p| {
        let mut p = p.clone();
        p.colors.insert("background", "not-a-color");
        overlay.render(ctx, i, &p)
    });
    assert!(matches!(
        overlay_result,
        Err(AttachmentUiError::Presentation(_))
    ));
    assert!(host.surfaces.borrow().is_empty());
    frame.tile(&mut tile, vec![]).unwrap();
    assert_eq!(host.surfaces.borrow().len(), 1);
}
