//! Whole-control opacity through the retained public message-link control.
#![cfg(test)]
use crate::brand;
use crate::canvas_paint;
use crate::session_links_support;
use session_links_support::Canvas;

use maestro_theme::BrandMode;

/// Mount each glyph, settle its colors, and sample the requested visibility.
fn sample(
    canvas: &mut Canvas,
    copied: bool,
    pack: &str,
    mode: BrandMode,
    opacity: f32,
) -> egui::FullOutput {
    if copied {
        canvas.activate().unwrap();
        session_links_support::settle(canvas, 0, true);
    }
    canvas.hovered = false;
    canvas.time = 1.;
    canvas
        .pack_frame(vec![egui::Event::PointerGone], pack, mode)
        .0
        .unwrap();
    canvas.time = 2.;
    let hidden = canvas.pack_frame(vec![], pack, mode).1;
    if opacity == 0. {
        return hidden;
    }
    canvas.hovered = true;
    canvas.time = 3.;
    canvas.pack_frame(vec![], pack, mode).0.unwrap();
    canvas.time = if (opacity - 0.5).abs() < f32::EPSILON {
        3.043_995_100_277_799
    } else {
        4.
    };
    canvas.pack_frame(vec![], pack, mode).1
}
/// Selected source operands, independent of the composed image.
fn colors(pack: &str, mode: BrandMode, copied: bool) -> [egui::Color32; 3] {
    let pack = maestro_theme::load_brand_pack(pack, &brand::Pack).unwrap();
    let p = pack.presentation(mode).unwrap();
    let roles = if copied {
        ["success", "on-accent", "success"]
    } else {
        ["panel", "muted", "border"]
    };
    roles.map(|role| egui::Color32::from_hex(p.colors[role]).unwrap())
}
/// Source paint operands shared by the two coverage matrices.
fn variants() -> impl Iterator<Item = (bool, &'static str, BrandMode, f32)> {
    [false, true]
        .into_iter()
        .flat_map(|copied| ["brand.json", "signal.json"].map(move |pack| (copied, pack)))
        .flat_map(|(copied, pack)| {
            [BrandMode::Dark, BrandMode::Light].map(move |mode| (copied, pack, mode))
        })
        .flat_map(|(copied, pack, mode)| {
            [1., 1.25, 2.].map(move |density| (copied, pack, mode, density))
        })
}
/// Overlap alpha and independent native channels cover every source paint operand.
#[test]
fn copy_link_composes_background_border_and_glyph_once() {
    let mut combinations = 0;
    for (copied, pack, mode, density) in variants() {
        for opacity in [0., 0.5, 1.] {
            for clipped in [false, true] {
                composition_case((copied, pack, mode, density), opacity, clipped);
                combinations += 1;
            }
        }
    }
    assert_eq!(combinations, 144);
}

/// Compare one composed frame against native channels and its final clip.
fn composition_case(
    (copied, pack, mode, density): (bool, &str, BrandMode, f32),
    opacity: f32,
    clipped: bool,
) {
    let mut canvas = Canvas::new();
    canvas.context.set_pixels_per_point(density);
    if clipped {
        canvas.clip = egui::Rect::from_min_max(
            canvas.rect.min,
            egui::pos2(canvas.rect.center().x, canvas.rect.bottom()),
        );
    }
    let output = sample(&mut canvas, copied, pack, mode, opacity);
    let pixels = canvas
        .paint
        .region(&canvas.context, &output.shapes, canvas.rect);
    let peak = pixels.iter().map(|pixel| pixel[3]).max().unwrap();
    assert!(
        (f32::from(peak) - 255. * opacity).abs() <= 1.,
        "group opacity copied={copied} density={density} opacity={opacity}: {peak}"
    );
    if opacity > 0. {
        let [background, foreground, border] = colors(pack, mode, copied);
        session_links_support::assert_glyph(
            &canvas,
            &output,
            copied,
            [background, foreground],
            opacity,
        );
        let point = egui::pos2(canvas.rect.left() + 0.5 / density, canvas.rect.center().y);
        let actual = canvas.paint.pixel(&canvas.context, &output.shapes, point);
        for (a, b) in actual.into_iter().zip(border.to_array()) {
            assert!((a - f32::from(b) * opacity).abs() <= 1.1);
        }
        assert_eq!(
            output
                .shapes
                .iter()
                .filter(|s| matches!(s.shape, egui::Shape::Mesh(_)))
                .count(),
            1
        );
    }
    if clipped {
        assert_eq!(
            canvas
                .paint
                .pixel(
                    &canvas.context,
                    &output.shapes,
                    canvas.rect.right_center() - egui::vec2(2., 0.)
                )
                .map(f32::to_bits),
            [0; 4]
        );
    }
}

/// Read the actual composed texture identity, excluding the font atlas.
fn texture(output: &egui::FullOutput) -> egui::TextureId {
    let textures: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Mesh(mesh) => Some(mesh.texture_id),
            _ => None,
        })
        .collect();
    assert_eq!(textures.len(), 1);
    textures[0]
}

/// Unchanged and opacity-only frames neither rasterize nor upload again.
#[test]
fn copy_link_composition_reuses_and_releases_one_texture() {
    for copied in [false, true] {
        cached_control(copied);
    }
}
/// Both glyph producers retain one image during visibility-only frames.
fn cached_control(copied: bool) {
    let mut first = Canvas::new();
    let initial = sample(&mut first, copied, "brand.json", BrandMode::Dark, 1.);
    let id = texture(&initial);
    let uploads = first.paint.uploads(&initial.shapes);
    assert!(uploads > 0);
    if !copied {
        assert_eq!(uploads, 1);
    }
    for time in [5., 6.] {
        first.time = time;
        let frame = first.frame(vec![]).1;
        assert_eq!(texture(&frame), id);
        assert_eq!(first.paint.uploads(&frame.shapes), uploads);
    }
    first.hovered = false;
    first.time = 7.;
    first.frame(vec![]).0.unwrap();
    first.time = 7.043_995_100_277_799;
    let faded = first.frame(vec![]).1;
    assert_eq!(texture(&faded), id);
    assert_eq!(first.paint.uploads(&faded.shapes), uploads);
    let mut second = Canvas::new();
    second.context = first.context.clone();
    let frame = second.frame(vec![]).1;
    let sibling = texture(&frame);
    assert_ne!(id, sibling);
    assert_eq!(second.paint.uploads(&frame.shapes), 1);
    let again = second.frame(vec![]).1;
    assert_eq!(second.paint.uploads(&again.shapes), 1);
    assert_texture_remount(&mut first, id, sibling);
}
/// Drop one control without releasing its sibling, then mount clean paint.
fn assert_texture_remount(first: &mut Canvas, id: egui::TextureId, sibling: egui::TextureId) {
    let old = std::mem::replace(
        &mut first.state,
        maestro_web::export_html::template::CopyLinkState::new(first.host.clone()),
    );
    drop(old);
    assert!(first.context.tex_manager().read().meta(id).is_none());
    assert!(first.context.tex_manager().read().meta(sibling).is_some());
    first.hovered = true;
    let mounted = first.frame(vec![]).1;
    assert_ne!(texture(&mounted), id);
    let peak = first
        .paint
        .region(&first.context, &mounted.shapes, first.rect)
        .into_iter()
        .map(|p| p[3])
        .max()
        .unwrap();
    assert_eq!(peak, 255);
}

/// Native producers retain opaque coverage at fractional placements and final clips.
#[test]
fn copy_link_preserves_opaque_glyph_paint_across_density_and_clip() {
    for (copied, pack, mode, density) in variants() {
        for offset in [0., 0.375] {
            for clip in [0, 1, 2] {
                opaque_case((copied, pack, mode, density), offset, clip);
            }
        }
    }
}

/// Check one native opaque projection without replacing the caller clip.
fn opaque_case((copied, pack, mode, density): (bool, &str, BrandMode, f32), offset: f32, clip: u8) {
    let mut canvas = Canvas::new();
    canvas.context.set_pixels_per_point(density);
    canvas.rect = canvas.rect.translate(egui::vec2(offset, offset));
    let opaque = sample(&mut canvas, copied, pack, mode, 1.);
    let uploads = canvas.paint.uploads(&opaque.shapes);
    canvas.clip = match clip {
        0 => egui::Rect::EVERYTHING,
        1 => egui::Rect::from_min_max(
            canvas.rect.min,
            egui::pos2(canvas.rect.center().x, canvas.rect.bottom()),
        ),
        _ => egui::Rect::NOTHING,
    };
    let output = canvas.pack_frame(vec![], pack, mode).1;
    let [background, foreground, _] = colors(pack, mode, copied);
    if clip < 2 {
        session_links_support::assert_glyph(&canvas, &output, copied, [background, foreground], 1.);
        assert_eq!(canvas.paint.uploads(&output.shapes), uploads);
    } else {
        assert!(
            canvas
                .paint
                .region(&canvas.context, &output.shapes, canvas.rect)
                .iter()
                .all(|p| *p == [0; 4])
        );
    }
}

/// Compare the full native text extent, including ink outside the unchanged input slot.
fn text_pixels(
    canvas: &Canvas,
    output: &egui::FullOutput,
    symbol: &str,
    font: egui::FontId,
    colors: [egui::Color32; 2],
) -> usize {
    let (paint, native) = session_links_support::native_text(canvas, symbol, font, colors[1]);
    let bounds = native[0]
        .shape
        .visual_bounding_rect()
        .union(canvas.rect)
        .expand(1.);
    let mut outside = 0;
    for point in canvas_paint::points(&canvas.context, bounds) {
        let glyph = paint.pixel(&canvas.context, &native, point);
        let actual = canvas.paint.pixel(&canvas.context, &output.shapes, point);
        if !canvas.rect.contains(point) {
            if glyph[3].round() > 0. {
                outside += 1;
            }
            for (a, b) in actual.into_iter().zip(glyph) {
                assert!(
                    (a.round() - b.round()).abs() <= 1.,
                    "native overflow differs"
                );
            }
        } else if canvas.rect.shrink(4.).contains(point) {
            let background = colors[0].to_array().map(f32::from);
            for (i, a) in actual.into_iter().enumerate() {
                let expected = glyph[i] + background[i] * (1. - glyph[3] / 255.);
                assert!(
                    (a.round() - expected.round()).abs() <= 1.,
                    "native selected text differs"
                );
            }
        }
    }
    outside
}

/// Live selected fonts and arbitrary supplied symbols retain the native paint extent.
#[test]
fn copy_link_composition_uses_live_selected_typography() {
    for name in ["brand.json", "signal.json"] {
        for registered in [false, true] {
            typography_case(name, registered);
        }
    }
}
/// Exercise each selected font route through live symbol/type-size changes.
fn typography_case(name: &str, registered: bool) {
    let pack = maestro_theme::load_brand_pack(name, &brand::Pack).unwrap();
    let p = pack.presentation(BrandMode::Dark).unwrap();
    let family = p.fonts[&p.typography["body"].font].family.clone();
    let native_family = if registered {
        egui::FontFamily::Name(family.clone().into())
    } else {
        egui::FontFamily::Proportional
    };
    let mut canvas = Canvas::new();
    canvas.context.all_styles_mut(|s| {
        s.text_styles
            .insert(egui::TextStyle::Button, egui::FontId::monospace(40.));
    });
    if registered {
        register_font(&canvas.context, &family, false);
    }
    canvas.activate().unwrap();
    session_links_support::settle(&canvas, 0, true);
    canvas
        .styled_frame(vec![egui::Event::PointerGone], &p)
        .0
        .unwrap();
    canvas.time = 1.;
    for (symbol, size) in [
        ("✓", 16.),
        ("", 16.),
        ("OK", 16.),
        ("W", 16.),
        ("WWWW", 16.),
        ("✓", 64.),
    ] {
        let mut typography = p.typography.clone();
        typography.get_mut("body").unwrap().size = f64::from(size);
        let mut selected = pack.presentation(BrandMode::Dark).unwrap();
        selected.typography = &typography;
        selected.glyphs.get_mut("passed").unwrap().symbol = symbol;
        let (response, output) = canvas.styled_frame(vec![], &selected);
        assert_eq!(response.unwrap().rect, canvas.rect);
        let [background, foreground, _] = colors(name, BrandMode::Dark, true);
        let outside = text_pixels(
            &canvas,
            &output,
            symbol,
            egui::FontId::new(size, native_family.clone()),
            [background, foreground],
        );
        if symbol == "WWWW" || size > 16. {
            assert!(
                outside > 0,
                "wide or large text must retain out-of-slot ink"
            );
        }
    }
    if registered {
        assert_font_replacement(&mut canvas, &pack, &family);
    }
}

/// A same-name native font replacement invalidates the retained layout.
fn assert_font_replacement(canvas: &mut Canvas, pack: &maestro_theme::BrandPack, family: &str) {
    let mut replacement = pack.presentation(BrandMode::Dark).unwrap();
    replacement.glyphs.get_mut("passed").unwrap().symbol = "W";
    let before = canvas.styled_frame(vec![], &replacement).1;
    let uploads = canvas.paint.uploads(&before.shapes);
    let old = canvas
        .paint
        .region(&canvas.context, &before.shapes, canvas.rect.expand(20.));
    register_font(&canvas.context, family, true);
    let output = canvas.styled_frame(vec![], &replacement).1;
    assert_eq!(canvas.paint.uploads(&output.shapes), uploads + 1);
    assert_ne!(
        old,
        canvas
            .paint
            .region(&canvas.context, &output.shapes, canvas.rect.expand(20.))
    );
    let background = egui::Color32::from_hex(replacement.colors["success"]).unwrap();
    let foreground = egui::Color32::from_hex(replacement.colors["on-accent"]).unwrap();
    text_pixels(
        canvas,
        &output,
        "W",
        egui::FontId::new(16., egui::FontFamily::Name(family.into())),
        [background, foreground],
    );
}

/// Install an existing native font under the selected family without changing its name.
fn register_font(context: &egui::Context, name: &str, alternate: bool) {
    let mut definitions = egui::FontDefinitions::default();
    let source = if alternate {
        egui::FontFamily::Monospace
    } else {
        egui::FontFamily::Proportional
    };
    let data = definitions.families[&source].clone();
    definitions
        .families
        .insert(egui::FontFamily::Name(name.into()), data);
    context.set_fonts(definitions);
}

/// Changed raster operands refresh pixels; preparation failures leave the retained image owned.
#[test]
fn copy_link_composition_refreshes_only_changed_raster_inputs() {
    let pack = maestro_theme::load_brand_pack("brand.json", &brand::Pack).unwrap();
    let mut p = pack.presentation(BrandMode::Dark).unwrap();
    let mut canvas = Canvas::new();
    let initial = canvas.styled_frame(vec![], &p).1;
    let id = texture(&initial);
    let mut uploads = canvas.paint.uploads(&initial.shapes);
    for (role, replacement) in [
        ("panel", "background"),
        ("muted", "text"),
        ("border", "success"),
    ] {
        p.colors.insert(role, p.colors[replacement]);
        canvas.time += 1.;
        canvas.styled_frame(vec![], &p).0.unwrap();
        canvas.time += 1.;
        let frame = canvas.styled_frame(vec![], &p).1;
        assert_eq!(texture(&frame), id);
        assert_eq!(canvas.paint.uploads(&frame.shapes), uploads + 1);
        uploads += 1;
    }
    uploads = assert_radii(&mut canvas, &pack, uploads);
    for geometry in [
        canvas.rect.translate(egui::vec2(0.375, 0.375)),
        egui::Rect::from_min_size(canvas.rect.min, egui::vec2(34., 30.)),
    ] {
        canvas.rect = geometry;
        let frame = canvas.styled_frame(vec![], &p).1;
        assert!(canvas.paint.uploads(&frame.shapes) > uploads);
        uploads = canvas.paint.uploads(&frame.shapes);
    }
    canvas.context.set_pixels_per_point(2.);
    let frame = canvas.styled_frame(vec![], &p).1;
    assert!(canvas.paint.uploads(&frame.shapes) > uploads);
    uploads = canvas.paint.uploads(&frame.shapes);
    canvas.viewport.x = 60.;
    let frame = canvas.styled_frame(vec![], &p).1;
    assert!(canvas.paint.uploads(&frame.shapes) > uploads);
    uploads = canvas.paint.uploads(&frame.shapes);
    p.colors.insert("background", "invalid");
    let error = canvas.styled_frame(vec![], &p).0.unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(canvas.context.tex_manager().read().meta(id).is_some());
    p.colors.insert("background", p.colors["panel"]);
    let recovered = canvas.styled_frame(vec![], &p).1;
    assert_eq!(canvas.paint.uploads(&recovered.shapes), uploads);
    canvas.viewport.x = 800.;
    let restored = canvas.styled_frame(vec![], &p).1;
    assert_eq!(canvas.paint.uploads(&restored.shapes), uploads + 1);
}

/// Radius normalization preserves fractional inputs and the source circle boundary.
fn assert_radii(canvas: &mut Canvas, pack: &maestro_theme::BrandPack, mut uploads: usize) -> usize {
    let p = pack.presentation(BrandMode::Dark).unwrap();
    let mut contours = Vec::new();
    let mut fractional = Vec::new();
    for radius in [0., 1., 1.5, 2., 4., 14., 40.] {
        let mut radii = p.radii.clone();
        radii.insert("control".into(), radius);
        let mut selected = pack.presentation(BrandMode::Dark).unwrap();
        selected.radii = &radii;
        let frame = canvas.styled_frame(vec![], &selected).1;
        assert!(canvas.paint.uploads(&frame.shapes) > uploads);
        uploads = canvas.paint.uploads(&frame.shapes);
        let pixels = canvas
            .paint
            .region(&canvas.context, &frame.shapes, canvas.rect);
        if radius == 0. {
            assert_eq!(pixels[0][3], 255);
        }
        if radius >= 14. {
            contours.push(pixels);
        } else if (1. ..=2.).contains(&radius) {
            fractional.push(pixels);
        }
    }
    assert_ne!(fractional[0], fractional[1]);
    assert_ne!(fractional[1], fractional[2]);
    assert_eq!(contours[0], contours[1]);
    uploads
}
