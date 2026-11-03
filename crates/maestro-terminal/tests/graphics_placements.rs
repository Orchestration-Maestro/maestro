//! Renderer projection exercised through terminal input.

use super::support::kitty_terminal::kitty_terminal as terminal;
use maestro_terminal::{Terminal, kitty::graphics::placements::KittyImageFormat};

#[test]
fn inline_rgba_images_expose_their_placement() {
    let mut terminal = terminal();
    terminal.write(b"\x1b_Ga=T,f=32,t=d,i=7,p=3,s=1,v=1,c=10,r=5,q=2;/wAA/w==\x1b\\");
    let mut descriptors = Vec::new();
    let placements = terminal.kitty_image_placements_with_data_filter(|descriptor| {
        descriptors.push(descriptor);
        true
    });
    assert_eq!(placements.len(), 1);
    let placement = &placements[0];
    assert_eq!((placement.image_id, placement.placement_id), (7, 3));
    assert_eq!((placement.image_width, placement.image_height), (1, 1));
    assert_eq!(placement.format, KittyImageFormat::Rgba);
    assert_eq!(placement.data, [255, 0, 0, 255]);
    assert_eq!(
        (placement.render.grid_cols, placement.render.grid_rows),
        (10, 5)
    );
    assert_eq!(descriptors[0].data_len, 4);
    assert_eq!(descriptors[0].data_fingerprint, placement.data_fingerprint);
    let filtered = terminal.kitty_image_placements_with_data_filter(|_| false);
    assert!(filtered[0].data.is_empty());
    assert_eq!(filtered[0].data_len, 4);
    assert_eq!(filtered[0].data_fingerprint, placement.data_fingerprint);
}

#[test]
fn image_digest_includes_the_entire_payload() {
    use base64::Engine;
    let mut terminal = terminal();
    let mut pixels = vec![1; 4096 * 4];
    let upload = |pixels: &[u8]| {
        format!(
            "\x1b_Ga=T,f=32,i=1,p=1,s=128,v=32,c=1,r=1,C=1,q=2;{}\x1b\\",
            base64::engine::general_purpose::STANDARD.encode(pixels)
        )
    };
    terminal.write(upload(&pixels).as_bytes());
    let first = terminal.kitty_image_placements_with_data_filter(|_| false);
    pixels[4096 + 123] = 2;
    terminal.write(upload(&pixels).as_bytes());
    let changed = terminal.kitty_image_placements_with_data_filter(|_| false);
    assert_ne!(first[0].data_fingerprint, changed[0].data_fingerprint);
}

#[test]
fn retransmission_invalidates_the_image_digest() {
    let mut terminal = terminal();
    terminal.write(b"\x1b_Ga=T,f=32,i=7,p=3,s=1,v=1,c=10,r=5,q=2;/wAA/w==\x1b\\");
    let first = terminal.kitty_image_placements_with_data_filter(|_| true);
    terminal.write(b"\x1b_Ga=t,f=32,i=7,s=1,v=1,q=2;AAAAAA==\x1b\\");
    let second = terminal.kitty_image_placements_with_data_filter(|_| true);
    let third = terminal.kitty_image_placements_with_data_filter(|_| false);
    assert_ne!(first[0].data_fingerprint, second[0].data_fingerprint);
    assert_eq!(second[0].data, [0; 4]);
    assert_eq!(second[0].data_fingerprint, third[0].data_fingerprint);
}

#[test]
fn only_observed_empty_storage_skips_image_queries() {
    let mut terminal = terminal();
    assert!(
        terminal
            .kitty_image_placements_with_data_filter(|_| panic!("empty storage"))
            .is_empty()
    );
    terminal.write(b"\x1b_Ga=t,f=24,i=1,s=1,v=2;////////\x1b\\");
    assert!(
        terminal
            .kitty_image_placements_with_data_filter(|_| panic!("no placement"))
            .is_empty()
    );
    terminal.write(b"plain text");
    assert!(
        terminal
            .kitty_image_placements_with_data_filter(|_| panic!("proven empty"))
            .is_empty()
    );
    terminal.write(b"\x1b[H\x1b_Ga=p,i=1,p=1,c=1,r=1,C=1\x1b\\");
    let first = terminal.kitty_image_placements_with_data_filter(|_| true);
    assert_eq!(first.len(), 1);
    terminal.resize(10, 5, 12, 24).unwrap();
    let resized = terminal.kitty_image_placements_with_data_filter(|_| false);
    assert_eq!(resized[0].render.pixel_width, 12);
    assert_eq!(resized[0].data_fingerprint, first[0].data_fingerprint);
    for _ in 0..20 {
        terminal.write(b"line\r\n");
    }
    assert!(
        terminal
            .kitty_image_placements_with_data_filter(|_| panic!("offscreen"))
            .is_empty()
    );
    terminal.scroll_viewport(maestro_terminal::terminal::ScrollViewport::Top);
    assert_eq!(
        terminal
            .kitty_image_placements_with_data_filter(|_| false)
            .len(),
        1
    );
    terminal.write(b"\x1b_Ga=d,d=A\x1b\\");
    assert!(
        terminal
            .kitty_image_placements_with_data_filter(|_| panic!("deleted"))
            .is_empty()
    );
}

#[test]
fn unicode_placeholders_resolve_image_geometry() {
    let mut terminal = terminal();
    terminal.write(b"\x1b_Gq=2,a=t,f=32,s=1,v=1,i=1193046;/wAA/w==\x1b\\");
    terminal.write(b"\x1b_Gq=2,a=p,U=1,i=1193046,c=2,r=1\x1b\\");
    assert!(
        terminal
            .kitty_image_placements_with_data_filter(|_| panic!("prototype"))
            .is_empty()
    );
    terminal.write(
        "\x1b[2;3H\x1b[38;2;18;52;86m\u{10eeee}\u{0305}\u{0305}\u{10eeee}\u{0305}\u{030d}\x1b[0m"
            .as_bytes(),
    );
    let placements = terminal.kitty_image_placements_with_data_filter(|_| true);
    assert_eq!(placements.len(), 1);
    let placement = &placements[0];
    assert_eq!(placement.image_id, 1_193_046);
    assert_ne!(placement.placement_id, 0);
    assert_eq!(placement.data, [255, 0, 0, 255]);
    assert_eq!(
        (placement.render.viewport_col, placement.render.viewport_row),
        (2, 1)
    );
    assert_eq!(
        (placement.render.grid_cols, placement.render.grid_rows),
        (2, 1)
    );
    assert_eq!(
        (placement.render.pixel_width, placement.render.pixel_height),
        (16, 16)
    );
}

#[test]
fn local_graphics_media_remain_available() {
    let terminal = terminal();
    assert_eq!(
        terminal.screens.active().kitty_images.image_limits,
        maestro_terminal::kitty::graphics::image::Limits::ALL
    );
}

/// Upload one raw pixel from a regular file through the same APC route.
fn file_upload(terminal: &mut Terminal, action: &str) {
    use base64::Engine;
    let path = std::env::temp_dir().join(format!(
        "maestro-graphics-placement-{}-{action}.rgba",
        std::process::id()
    ));
    std::fs::write(&path, [255, 0, 0, 255]).unwrap();
    let encoded =
        base64::engine::general_purpose::STANDARD.encode(path.as_os_str().as_encoded_bytes());
    terminal.write(
        format!("\x1b_Ga={action},f=32,t=f,i=9,p=4,s=1,v=1,c=10,r=5,q=2;{encoded}\x1b\\")
            .as_bytes(),
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn file_backed_rgba_images_expose_their_placement() {
    let mut terminal = terminal();
    file_upload(&mut terminal, "T");
    terminal.write(b"\x1b_Ga=p,U=1,i=9,c=10,r=5\x1b\\");
    let placements = terminal.kitty_image_placements_with_data_filter(|_| true);
    assert_eq!(placements.len(), 1);
    assert_eq!((placements[0].image_id, placements[0].placement_id), (9, 4));
    assert_eq!(placements[0].format, KittyImageFormat::Rgba);
    assert_eq!(placements[0].data, [255, 0, 0, 255]);
    assert_eq!(
        (
            placements[0].render.grid_cols,
            placements[0].render.grid_rows
        ),
        (10, 5)
    );
}

#[test]
fn uploaded_files_can_be_positioned_later() {
    let mut terminal = terminal();
    file_upload(&mut terminal, "t");
    assert!(
        terminal
            .kitty_image_placements_with_data_filter(|_| panic!("upload only"))
            .is_empty()
    );
    terminal.write(b"\x1b_Ga=p,i=9,p=5,c=10,r=5,C=1,q=2\x1b\\");
    let placements = terminal.kitty_image_placements_with_data_filter(|_| true);
    assert_eq!(placements.len(), 1);
    assert_eq!((placements[0].image_id, placements[0].placement_id), (9, 5));
    assert_eq!(placements[0].data, [255, 0, 0, 255]);
}

#[test]
fn projection_sorts_all_placements_and_keeps_image_metadata() {
    let mut terminal = terminal();
    terminal.write(b"\x1b_Ga=T,f=24,i=1,p=1,s=1,v=2,c=1,r=1,z=3,C=1;////////\x1b\\");
    terminal.write(b"\x1b_Ga=p,i=1,p=2,c=2,r=2,z=-3,X=1,Y=2,C=1\x1b\\");
    let placements = terminal.kitty_image_placements_with_data_filter(|_| true);
    assert_eq!(placements.len(), 2);
    assert_eq!((placements[0].placement_id, placements[0].z), (2, -3));
    assert_eq!((placements[1].placement_id, placements[1].z), (1, 3));
    assert_eq!((placements[0].x_offset, placements[0].y_offset), (1, 2));
    assert_eq!(placements[0].format, KittyImageFormat::Rgb);
    assert_eq!(
        (placements[0].image_width, placements[0].image_height),
        (1, 2)
    );
    assert_eq!(placements[0].data, [255; 6]);
    assert_eq!(
        placements[0].data_fingerprint,
        placements[1].data_fingerprint
    );
}

#[test]
fn projection_exposes_decoded_pixels_not_compressed_bytes() {
    let mut terminal = terminal();
    terminal.write(b"\x1b_Ga=T,f=24,o=z,i=1,p=1,s=1,v=1,c=1,r=1;eAEBAwD8//8AAAMAAQA=\x1b\\");
    let placements = terminal.kitty_image_placements_with_data_filter(|_| true);
    assert_eq!(placements.len(), 1);
    assert_eq!(placements[0].format, KittyImageFormat::Rgb);
    assert_eq!(placements[0].data, [255, 0, 0]);
    assert_eq!(placements[0].data_len, 3);
}

#[test]
fn projection_refreshes_on_screen_switches_and_resets() {
    let mut terminal = terminal();
    let upload = b"\x1b_Ga=T,f=32,i=1,p=1,s=1,v=1,c=1,r=1,C=1;/wAA/w==\x1b\\";
    terminal.write(upload);
    let main = terminal.kitty_image_placements_with_data_filter(|_| true);
    terminal.write(b"\x1b[?1049h");
    assert!(
        terminal
            .kitty_image_placements_with_data_filter(|_| panic!("fresh screen"))
            .is_empty()
    );
    terminal.write(b"\x1b_Ga=T,f=32,i=1,p=1,s=1,v=1,c=1,r=1,C=1;AAAAAA==\x1b\\");
    let alternate = terminal.kitty_image_placements_with_data_filter(|_| true);
    assert_ne!(main[0].data_fingerprint, alternate[0].data_fingerprint);
    terminal.write(b"\x1b[?1049l");
    let restored = terminal.kitty_image_placements_with_data_filter(|_| false);
    assert_eq!(main[0].data_fingerprint, restored[0].data_fingerprint);
    terminal.write(b"\x1bc");
    assert!(
        terminal
            .kitty_image_placements_with_data_filter(|_| panic!("reset"))
            .is_empty()
    );
    terminal.write(upload);
    let reset = terminal.kitty_image_placements_with_data_filter(|_| true);
    assert_eq!(main[0].data_fingerprint, reset[0].data_fingerprint);
}

#[test]
fn placeholder_projection_fills_cells_after_resize_and_keeps_z_order() {
    let mut terminal = terminal();
    terminal.write(b"\x1b_Ga=T,f=32,i=1,p=1,s=1,v=1,c=1,r=1,z=3,C=1;/wAA/w==\x1b\\");
    terminal.write(b"\x1b_Ga=p,U=1,i=1,p=2,c=2,r=1,z=-3\x1b\\");
    terminal.write("\x1b[2;3H\x1b[38;2;0;0;1m\x1b[58;2;0;0;2m\u{10eeee}\u{305}\u{305}\u{10eeee}\u{305}\u{30d}\x1b[0m".as_bytes());
    let first = terminal.kitty_image_placements_with_data_filter(|_| false);
    assert_eq!(first.len(), 2);
    assert_eq!((first[0].z, first[1].z), (-3, 3));
    assert_eq!(
        (first[0].render.pixel_width, first[0].render.pixel_height),
        (16, 16)
    );
    assert_eq!(
        (first[0].render.source_width, first[0].render.source_height),
        (1, 1)
    );
    let identity = first[0].placement_id;
    terminal.resize(10, 5, 8, 24).unwrap();
    let resized = terminal.kitty_image_placements_with_data_filter(|_| true);
    assert_eq!(resized[0].placement_id, identity);
    assert_eq!(
        (
            resized[0].render.pixel_width,
            resized[0].render.pixel_height
        ),
        (16, 24)
    );
    assert_eq!((resized[0].x_offset, resized[0].y_offset), (0, 0));
    assert_eq!(resized[0].data_fingerprint, first[1].data_fingerprint);
    for _ in 0..20 {
        terminal.write(b"line\r\n");
    }
    assert!(
        terminal
            .kitty_image_placements_with_data_filter(|_| panic!("offscreen"))
            .is_empty()
    );
    terminal.scroll_viewport(maestro_terminal::terminal::ScrollViewport::Top);
    let restored = terminal.kitty_image_placements_with_data_filter(|_| false);
    assert_eq!(restored.len(), 2);
    assert_eq!(restored[0].placement_id, identity);
}

#[test]
fn placeholder_projection_keeps_constructor_only_placements() {
    let mut terminal = Terminal::new(10, 5, 100_000).unwrap();
    terminal.enable_kitty_graphics();
    terminal.write(b"\x1b_Ga=t,f=32,i=1,s=1,v=1;/wAA/w==\x1b\\");
    terminal.write(b"\x1b_Ga=p,U=1,i=1\x1b\\");
    terminal.write("\x1b[38;2;0;0;1m\u{10eeee}".as_bytes());
    let placements = terminal.kitty_image_placements_with_data_filter(|_| true);
    assert_eq!(placements.len(), 1);
    assert_eq!(
        (
            placements[0].render.pixel_width,
            placements[0].render.pixel_height
        ),
        (1, 1)
    );
    assert_eq!(
        (
            placements[0].render.source_width,
            placements[0].render.source_height
        ),
        (1, 1)
    );
    assert_eq!((placements[0].x_offset, placements[0].y_offset), (0, 0));
}

#[test]
fn placeholder_projection_clips_source_fragments_to_the_prototype_grid() {
    use base64::Engine;
    let mut terminal = terminal();
    let data = base64::engine::general_purpose::STANDARD.encode([0; 60]);
    terminal.write(format!("\x1b_Ga=t,f=32,i=1,s=5,v=3;{data}\x1b\\").as_bytes());
    terminal.write(b"\x1b_Ga=p,U=1,i=1,c=3,r=2\x1b\\");
    terminal.write("\x1b[38;2;0;0;1m\u{10eeee}\u{30d}\u{30d}\u{10eeee}\u{10eeee}".as_bytes());
    let placements = terminal.kitty_image_placements_with_data_filter(|_| false);
    assert_eq!(placements.len(), 1);
    let render = placements[0].render;
    assert_eq!((render.grid_cols, render.grid_rows), (2, 1));
    assert_eq!((render.pixel_width, render.pixel_height), (16, 16));
    assert_eq!(
        (
            render.source_x,
            render.source_y,
            render.source_width,
            render.source_height
        ),
        (1, 1, 3, 1)
    );
    terminal.write("\r\n\u{10eeee}\u{30e}\u{305}\r\n\u{10eeee}\u{305}\u{310}".as_bytes());
    assert_eq!(
        terminal
            .kitty_image_placements_with_data_filter(|_| false)
            .len(),
        1
    );
}
