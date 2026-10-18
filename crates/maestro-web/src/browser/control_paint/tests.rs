//! Error settlement through the shared painter's real raster callback.
use super::{Painter, Raster};

/// Produce a small controlled native raster.
fn raster(color: egui::Color32) -> Raster {
    Raster {
        rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(2., 2.)),
        image: egui::ColorImage::filled([2, 2], color),
    }
}

#[test]
fn failed_raster_replacement_retains_texture_and_retries_the_same_key() {
    let context = egui::Context::default();
    let mut painter = Painter::default();
    let mut calls = 0;
    let mut retained = None;
    for (key, fail, expected_calls) in [
        (1, false, 1),
        (2, true, 2),
        (1, false, 2),
        (2, false, 3),
        (2, false, 3),
    ] {
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            let result = painter.draw(ui, (key, 1.), |_| {
                calls += 1;
                prepare(key, fail)
            });
            if fail {
                assert_eq!(result.unwrap_err().raw_os_error(), Some(5));
            } else {
                result.unwrap();
            }
        });
        assert_eq!(calls, expected_calls);
        if fail {
            assert!(
                context
                    .tex_manager()
                    .read()
                    .meta(retained.unwrap())
                    .is_some()
            );
        } else {
            let id = painted_texture(&output, key);
            assert_eq!(*retained.get_or_insert(id), id);
        }
        output.textures_delta.clear();
    }
}

/// Preserve a typed raster failure before any replacement ownership changes.
fn prepare(key: u8, fail: bool) -> std::io::Result<Raster> {
    if fail {
        return Err(std::io::Error::from_raw_os_error(5));
    }
    Ok(raster(if key == 1 {
        egui::Color32::WHITE
    } else {
        egui::Color32::BLACK
    }))
}
/// Observe the submitted texture and any reached replacement pixels.
fn painted_texture(output: &egui::FullOutput, key: u8) -> egui::TextureId {
    let id = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Mesh(mesh) => Some(mesh.texture_id),
            _ => None,
        })
        .unwrap();
    if let Some(deltas) = output.textures_delta.set.get(&id) {
        let egui::ImageData::Color(image) = &deltas[0].image;
        assert_eq!(
            image.pixels[0],
            if key == 1 {
                egui::Color32::WHITE
            } else {
                egui::Color32::BLACK
            }
        );
    }
    id
}
