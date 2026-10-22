//! Behaviour of the host graphics cache and its wire encoding.

use std::collections::HashSet;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};

#[cfg(not(target_arch = "wasm32"))]
use super::ClippedPlacement;
#[cfg(unix)]
use super::encode_kitty_regular_file;
use super::{
    HostCellSize, HostGraphicsCache, HostPlacement, HostSourceKey, HostView, Rect, WindowSize,
    clipped_placement, direct_file_command,
    identity::{host_image_id, host_image_id_for_signature, host_placement_id, kitty_format_code},
};
use crate::kitty::graphics::placements::{KittyImageFormat, KittyImagePlacement};
use crate::kitty::graphics::render::PlacementRenderInfo;

/// Bit the caller sets on an image identity it reserves itself.
const PANE_GRAPHICS_IMAGE_ID_BIT: u32 = 1 << 31;

/// Transport budget the daemon reserves for one graphics transaction.
const GRAPHICS_TRANSACTION_BUDGET: usize = 31_457_280;

/// Terminal image placement used by the cache cases.
fn test_placement(viewport_col: i64, viewport_row: i64) -> HostPlacement {
    HostPlacement {
        pane_id: 1,
        host_image_id: None,
        area: Rect::new(0, 0, 20, 10),
        cell_size: HostCellSize {
            width_px: 10,
            height_px: 10,
        },
        source_key: HostSourceKey::Terminal {
            pane_id: 1,
            image_id: 7,
        },
        scrollback_offset: 0,
        placement: KittyImagePlacement {
            image_id: 7,
            placement_id: 3,
            z: 0,
            x_offset: 0,
            y_offset: 0,
            image_width: 30,
            image_height: 30,
            format: KittyImageFormat::Rgba,
            data_len: 30 * 30 * 4,
            data_fingerprint: 42,
            data: vec![255; 30 * 30 * 4],
            render: PlacementRenderInfo {
                pixel_width: 0,
                pixel_height: 0,
                grid_cols: 3,
                grid_rows: 3,
                viewport_col,
                viewport_row,
                viewport_visible: true,
                source_x: 0,
                source_y: 0,
                source_width: 0,
                source_height: 0,
            },
        },
    }
}

/// Pane layer placement of the same pixels as [`test_placement`].
fn pane_layer_placement(viewport_col: i64, viewport_row: i64) -> HostPlacement {
    let mut placement = test_placement(viewport_col, viewport_row);
    placement.source_key = HostSourceKey::PaneLayer {
        pane_id: placement.pane_id,
        layer_id: "primary".into(),
    };
    placement
}

/// One frame through the cache: the terminal path without pane layers, the
/// pane-layer path with them.
fn update(cache: &mut HostGraphicsCache, placements: &[HostPlacement], replay: bool) -> Vec<u8> {
    let mut live = cache
        .sources
        .keys()
        .filter(|source| matches!(source, HostSourceKey::PaneLayer { .. }))
        .cloned()
        .collect::<HashSet<_>>();
    live.extend(
        placements
            .iter()
            .filter(|placement| matches!(placement.source_key, HostSourceKey::PaneLayer { .. }))
            .map(|placement| placement.source_key.clone()),
    );
    if live.is_empty() {
        cache.update_terminal(placements, replay)
    } else {
        if replay {
            cache.request_placement_replay();
        }
        drain(cache, placements, &live)
    }
}

/// Run layer updates until the whole frame is on the host.
fn drain(
    cache: &mut HostGraphicsCache,
    placements: &[HostPlacement],
    live: &HashSet<HostSourceKey>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        let encoded = cache.update_layers(placements, live, None);
        bytes.extend(encoded.bytes);
        if !encoded.incomplete {
            return bytes;
        }
    }
}

/// Append one frame to a recorded transcript.
fn record(transcript: &mut Vec<u8>, bytes: &[u8]) {
    transcript.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    transcript.extend_from_slice(bytes);
}

/// FNV-1a digest of a recorded transcript.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Whether the bytes carry a command with the given fragment.
fn shows(bytes: &[u8], fragment: &str) -> bool {
    String::from_utf8_lossy(bytes).contains(fragment)
}

#[test]
fn empty_areas_have_no_fallback_cell_geometry() {
    assert_eq!(
        HostCellSize::fallback_for_area(Rect::new(0, 0, 80, 24)),
        HostCellSize {
            width_px: 8,
            height_px: 16,
        }
    );
    assert!(!HostCellSize::fallback_for_area(Rect::default()).is_known());
}

#[test]
fn window_dimensions_divide_into_cells() {
    let area = Rect::new(0, 0, 80, 24);
    let window = |columns, rows, width, height| WindowSize {
        columns,
        rows,
        width,
        height,
    };

    assert_eq!(
        HostCellSize::from_window(window(80, 24, 800, 480), area),
        Some(HostCellSize {
            width_px: 10,
            height_px: 20,
        })
    );
    assert_eq!(
        HostCellSize::from_window(window(200, 24, 100, 480), area),
        Some(HostCellSize {
            width_px: 1,
            height_px: 20,
        }),
        "a window narrower than its columns keeps one pixel per cell"
    );
    for unusable in [
        window(0, 24, 800, 480),
        window(80, 0, 800, 480),
        window(80, 24, 0, 480),
        window(80, 24, 800, 0),
    ] {
        assert_eq!(HostCellSize::from_window(unusable, area), None);
    }
    assert!(
        !HostCellSize::from_window(window(80, 24, 800, 480), Rect::default())
            .expect("a reported window")
            .is_known(),
        "an empty area has no usable cell size"
    );
}

#[test]
fn terminal_only_updates_preserve_the_wire_transcript() {
    let mut cache = HostGraphicsCache::default();
    let mut transcript = Vec::new();

    record(
        &mut transcript,
        &update(&mut cache, &[test_placement(0, 0)], false),
    );
    record(
        &mut transcript,
        &update(&mut cache, &[test_placement(0, 0)], false),
    );
    record(
        &mut transcript,
        &update(&mut cache, &[test_placement(-1, 2)], false),
    );
    record(
        &mut transcript,
        &update(&mut cache, &[test_placement(-1, 2)], true),
    );

    let mut changed = test_placement(-1, 2);
    changed.placement.data_fingerprint = 43;
    record(&mut transcript, &update(&mut cache, &[changed], false));
    record(&mut transcript, &update(&mut cache, &[], false));

    assert_eq!(transcript.len(), 10_084);
    assert_eq!(fnv1a(&transcript), 0xc5bd_83e4_b039_870e);
}

#[test]
fn terminal_placement_keys_keep_their_wire_identity() {
    let placement = test_placement(0, 0);
    let layer = HostSourceKey::PaneLayer {
        pane_id: placement.pane_id,
        layer_id: "primary".into(),
    };

    assert_eq!(
        host_placement_id(&placement.source_key, &placement.placement),
        249_690
    );
    assert_eq!(host_placement_id(&layer, &placement.placement), 773_910);
    assert_eq!(
        host_image_id(placement.pane_id, &placement.placement),
        174_084
    );
    assert_eq!(
        host_image_id_for_signature(
            placement.pane_id,
            super::identity::image_signature(
                &placement,
                kitty_format_code(placement.placement.format)
            ),
        ),
        174_084
    );
}

#[cfg(unix)]
#[test]
fn file_commands_encode_paths_and_request_replies() {
    let mut bytes = Vec::new();
    encode_kitty_regular_file(
        &mut bytes,
        b"\x1b[2;3H",
        "a=T,f=32,s=3,v=2,i=42,p=7,c=3,r=2,z=0,C=1,q=0",
        "/private/frame",
    );
    assert_eq!(
        String::from_utf8(bytes.clone()).unwrap(),
        "\x1b7\x1b[2;3H\x1b_Ga=T,f=32,s=3,v=2,i=42,p=7,c=3,r=2,z=0,C=1,q=0,t=f;\
         L3ByaXZhdGUvZnJhbWU=\x1b\\\x1b8"
    );
    assert_eq!(bytes.len(), 85);
}

#[test]
fn clipped_file_images_transmit_at_the_final_cursor_position() {
    let mut placement = pane_layer_placement(-1, 2);
    placement.area = Rect::new(10, 4, 8, 6);
    let command = direct_file_command(&placement, PANE_GRAPHICS_IMAGE_ID_BIT | 9).unwrap();

    assert_eq!(command.leading, b"\x1b[7;11H");
    assert_eq!(
        command.control,
        "a=T,f=32,s=30,v=30,i=2147483657,p=773910,c=2,r=3,z=0,C=1,q=0,x=10,w=20,h=30"
    );
    assert!(direct_file_command(&pane_layer_placement(30, 0), 9).is_none());
}

#[test]
fn positive_viewport_offsets_keep_visible_image_geometry() {
    let placement = test_placement(2, 2);
    let (clipped, format_code) = clipped_placement(&placement).expect("visible placement");

    assert_eq!(format_code, 32);
    assert_eq!(
        clipped,
        super::ClippedPlacement {
            x: 2,
            y: 2,
            cols: 3,
            rows: 3,
            source_x: 0,
            source_y: 0,
            source_width: 30,
            source_height: 30,
            x_offset: 0,
            y_offset: 0,
        }
    );
}

#[test]
fn negative_viewport_offsets_crop_image_pixels() {
    let mut placement = test_placement(-1, -1);
    placement.placement.x_offset = 3;
    placement.placement.y_offset = 4;
    let (clipped, _) = clipped_placement(&placement).expect("partially visible placement");

    assert_eq!(
        (clipped.x, clipped.y, clipped.cols, clipped.rows),
        (0, 0, 2, 2)
    );
    assert_eq!(
        (
            clipped.source_x,
            clipped.source_y,
            clipped.source_width,
            clipped.source_height
        ),
        (10, 10, 20, 20)
    );
    assert_eq!(
        (clipped.x_offset, clipped.y_offset),
        (0, 0),
        "a cropped side drops its in-cell offset"
    );
}

#[test]
fn moving_an_uploaded_image_sends_only_placement() {
    let mut cache = HostGraphicsCache::default();
    let first = update(&mut cache, &[test_placement(0, 0)], false);
    assert!(shows(&first, "a=t"), "first frame uploads the pixels");
    assert!(shows(&first, "a=p"), "first frame displays the placement");
    assert!(update(&mut cache, &[test_placement(0, 0)], false).is_empty());

    let mut changed = test_placement(0, 0);
    changed.placement.z = 1;
    for placement in [changed, test_placement(0, 1)] {
        let bytes = update(&mut cache, &[placement], false);
        assert!(!shows(&bytes, "a=t"), "moved placement is not re-uploaded");
        assert!(shows(&bytes, "a=p"), "moved placement is displayed");
    }
}

#[test]
fn switching_views_replays_unchanged_placements() {
    let mut cache = HostGraphicsCache::default();
    update(&mut cache, &[test_placement(0, 0)], false);
    assert_eq!(cache.placements.len(), 1);
    let bytes = update(&mut cache, &[test_placement(0, 0)], true);
    assert!(!shows(&bytes, "a=t"));
    assert!(shows(&bytes, "a=p"));
    assert_eq!(cache.placements.len(), 1);
}

#[test]
fn surface_reset_removes_and_restores_graphics() {
    let mut cache = HostGraphicsCache::default();
    update(&mut cache, &[test_placement(0, 0)], false);
    assert_eq!((cache.images.len(), cache.placements.len()), (1, 1));
    let mut bytes = cache.clear_bytes();
    bytes.extend(update(&mut cache, &[test_placement(0, 0)], false));
    assert!(shows(&bytes, "a=d,d=I"), "reset deletes the retained image");
    assert!(shows(&bytes, "a=t"), "reset re-uploads the pixels");
    assert!(shows(&bytes, "a=p"), "reset displays the placement again");
    assert_eq!((cache.images.len(), cache.placements.len()), (1, 1));
}

#[test]
fn scrolling_replays_image_positions() {
    let mut cache = HostGraphicsCache::default();
    update(&mut cache, &[test_placement(0, 0)], false);
    let mut scrolled = test_placement(0, 0);
    scrolled.scrollback_offset = 3;
    let bytes = update(&mut cache, &[scrolled], false);
    assert!(!shows(&bytes, "a=t"));
    assert!(shows(&bytes, "a=p"));
}

#[test]
fn empty_pixels_do_not_count_as_an_upload() {
    let mut cache = HostGraphicsCache::default();
    let mut placement = test_placement(0, 0);
    placement.placement.data.clear();

    assert!(update(&mut cache, &[placement], false).is_empty());
    assert!(cache.images.is_empty());
    assert!(cache.placements.is_empty());
}

#[test]
fn identical_pixels_share_one_host_upload() {
    let mut cache = HostGraphicsCache::default();
    update(&mut cache, &[test_placement(0, 0)], false);
    assert_eq!(cache.images.len(), 1);
    assert_eq!(cache.placements.len(), 1);

    let mut same_image_new_source_id = test_placement(0, 0);
    same_image_new_source_id.placement.image_id = 8;
    same_image_new_source_id.placement.placement_id = 4;
    same_image_new_source_id.placement.data.clear();
    let reused = update(&mut cache, &[same_image_new_source_id], false);

    assert!(!shows(&reused, "a=t"), "shared pixels are not re-uploaded");
    assert!(shows(&reused, "a=p"), "the shared image is displayed");
    assert_eq!(cache.images.len(), 1);
    assert_eq!(cache.placements.len(), 1);
}

#[test]
fn replacing_a_layer_never_paints_an_empty_intermediate_frame() {
    let mut cache = HostGraphicsCache::default();
    let mut first = pane_layer_placement(0, 0);
    first.host_image_id = Some(PANE_GRAPHICS_IMAGE_ID_BIT | 7);
    update(&mut cache, &[first], false);

    let mut changed = pane_layer_placement(0, 0);
    changed.host_image_id = Some(PANE_GRAPHICS_IMAGE_ID_BIT | 7);
    changed.placement.data_fingerprint += 1;
    changed.placement.data_len = 3;
    changed.placement.data = vec![0xff, 0x00, 0x41];
    changed.placement.render.source_y = 5;
    changed.placement.x_offset = 3;
    changed.placement.y_offset = 4;
    let replacement = update(&mut cache, &[changed], false);

    assert_eq!(
        String::from_utf8(replacement).expect("utf-8 commands"),
        "\x1b[1;1H\x1b_Ga=T,t=d,f=32,s=30,v=30,i=2147483655,p=773910,c=3,r=3,z=0,C=1,q=2,\
         y=5,w=30,h=25,X=3,Y=4,m=0;/wBB\x1b\\",
        "one transmit-and-display replaces the layer, with no delete to a blank frame"
    );
}

#[test]
fn changed_pixels_release_the_replaced_host_image() {
    let mut cache = HostGraphicsCache::default();
    update(&mut cache, &[test_placement(0, 0)], false);
    assert_eq!(cache.images.len(), 1);
    let superseded = *cache.images.keys().next().expect("uploaded image");

    let mut changed = test_placement(0, 0);
    changed.placement.data_fingerprint = 43;
    let bytes = update(&mut cache, &[changed], false);

    assert!(shows(&bytes, "a=t"), "changed content re-uploads");
    assert!(
        shows(&bytes, &format!("a=d,d=I,i={superseded}")),
        "the superseded host image is deleted"
    );
    assert_eq!(cache.images.len(), 1);
    assert_eq!(cache.placements.len(), 1);
}

#[test]
fn colliding_image_identity_keeps_the_placement_it_replaces() {
    let mut first = test_placement(0, 0);
    first.placement.data_fingerprint = 335;
    let mut changed = test_placement(0, 0);
    changed.placement.data_fingerprint = 336;
    assert_eq!(
        host_image_id(first.pane_id, &first.placement),
        host_image_id(changed.pane_id, &changed.placement),
        "the two payloads share one host image identity"
    );

    let mut cache = HostGraphicsCache::default();
    update(&mut cache, &[first], false);
    let bytes = update(&mut cache, &[changed], false);

    assert!(
        shows(&bytes, "a=t"),
        "the colliding payload is uploaded again"
    );
    assert!(shows(&bytes, "a=p"), "the placement is shown again");
    assert!(
        !shows(&bytes, "a=d,d=i"),
        "the placement it just showed stays"
    );
    assert_eq!(cache.placements.len(), 1);
}

/// Second terminal source of the same pixels on another position.
fn twin_placement() -> HostPlacement {
    let mut twin = test_placement(5, 5);
    twin.placement.image_id = 8;
    twin.placement.placement_id = 4;
    twin.source_key = HostSourceKey::Terminal {
        pane_id: twin.pane_id,
        image_id: twin.placement.image_id,
    };
    twin
}

#[test]
fn shared_uploads_live_until_the_last_source_leaves() {
    let mut cache = HostGraphicsCache::default();
    update(&mut cache, &[test_placement(0, 0), twin_placement()], false);
    assert_eq!(cache.images.len(), 1, "same pixels dedup to one upload");

    let mut changed = test_placement(0, 0);
    changed.placement.data_fingerprint = 43;
    let bytes = update(&mut cache, &[changed, twin_placement()], false);

    assert!(!shows(&bytes, "a=d,d=I"), "the shared upload survives");
    assert_eq!(cache.images.len(), 2);
}

#[test]
fn obsolete_source_entries_do_not_leak_old_images() {
    let mut cache = HostGraphicsCache::default();
    update(&mut cache, &[test_placement(0, 0), twin_placement()], false);
    assert_eq!(cache.images.len(), 1);
    assert_eq!(cache.sources.len(), 2);
    let shared = *cache.images.keys().next().expect("uploaded image");

    let mut changed = test_placement(0, 0);
    changed.placement.data_fingerprint = 43;
    let bytes = update(&mut cache, &[changed], false);

    assert!(
        shows(&bytes, &format!("a=d,d=I,i={shared}")),
        "the old upload goes once its last live source moves on"
    );
    assert_eq!(cache.images.len(), 1);
    assert_eq!(cache.sources.len(), 1);
}

#[test]
fn stale_placements_leave_the_shared_image_alive() {
    let mut cache = HostGraphicsCache::default();
    update(&mut cache, &[test_placement(0, 0)], false);
    assert_eq!(cache.placements.len(), 1);

    let bytes = update(&mut cache, &[], false);
    assert!(shows(&bytes, "a=d,d=i"), "the placement is deleted");
    assert!(!shows(&bytes, "d=I"), "the image is kept");
    assert!(cache.placements.is_empty());
    assert_eq!(cache.images.len(), 1);
}

#[test]
fn hidden_layers_replay_without_resending_pixels() {
    let mut cache = HostGraphicsCache::default();
    update(&mut cache, &[pane_layer_placement(0, 0)], false);

    for (visible, replay) in [(false, false), (true, false), (true, true)] {
        let current = visible.then(|| pane_layer_placement(0, 0));
        let bytes = update(&mut cache, current.as_slice(), replay);
        assert!(!shows(&bytes, "a=t"), "hidden layers resend no pixels");
        assert!(!shows(&bytes, "a=d,d=I"), "hidden layers keep their image");
        assert_eq!(shows(&bytes, "a=p"), visible, "only visible layers display");
    }
    assert_eq!(cache.images.len(), 1);
    assert_eq!(cache.sources.len(), 1);
}

#[test]
fn removing_a_layer_releases_its_unshared_image() {
    let mut cache = HostGraphicsCache::default();
    update(&mut cache, &[pane_layer_placement(0, 0)], false);
    let host_id = *cache.images.keys().next().expect("uploaded pane layer");

    let bytes = drain(&mut cache, &[], &HashSet::new());

    assert!(shows(&bytes, &format!("a=d,d=I,i={host_id}")));
    assert!(cache.images.is_empty());
    assert!(cache.placements.is_empty());
    assert!(cache.sources.is_empty());
}

#[test]
fn hiding_a_layer_removes_only_its_placement() {
    let mut cache = HostGraphicsCache::default();
    update(&mut cache, &[pane_layer_placement(0, 0)], false);
    let host_id = *cache.images.keys().next().expect("uploaded pane layer");

    let bytes = update(&mut cache, &[pane_layer_placement(100, 100)], false);

    assert!(shows(&bytes, "a=d,d=i"), "the placement is deleted");
    assert!(
        !shows(&bytes, &format!("a=d,d=I,i={host_id}")),
        "the image stays"
    );
    assert_eq!(cache.images.len(), 1);
    assert!(cache.placements.is_empty());
    assert_eq!(cache.sources.len(), 1);
}

#[test]
fn clipping_keeps_source_identity_for_later_replacement() {
    let mut cache = HostGraphicsCache::default();
    update(&mut cache, &[test_placement(0, 0)], false);
    let original = *cache.images.keys().next().expect("uploaded image");

    update(&mut cache, &[test_placement(100, 100)], false);
    assert_eq!(cache.images.len(), 1);
    assert_eq!(cache.sources.len(), 1);

    let mut changed = test_placement(0, 0);
    changed.placement.data_fingerprint = 43;
    let bytes = update(&mut cache, &[changed], false);

    assert!(shows(&bytes, &format!("a=d,d=I,i={original}")));
    assert_eq!(cache.images.len(), 1);
    assert_eq!(cache.sources.len(), 1);
}

#[test]
fn removing_a_layer_keeps_a_terminal_shared_upload() {
    let mut cache = HostGraphicsCache::default();
    update(
        &mut cache,
        &[pane_layer_placement(0, 0), test_placement(4, 0)],
        false,
    );
    assert_eq!(cache.images.len(), 1);

    let mut bytes = drain(&mut cache, &[], &HashSet::new());
    bytes.extend(update(&mut cache, &[test_placement(4, 0)], false));

    assert!(!shows(&bytes, "a=d,d=I"), "the shared upload survives");
    assert_eq!(cache.images.len(), 1);
    assert_eq!(cache.placements.len(), 1);
    assert_eq!(cache.sources.len(), 1);
}

#[test]
fn changing_one_source_does_not_starve_its_sibling() {
    let layers = |first| {
        [(1, "a", first), (2, "b", 80)].map(|(id, name, fingerprint)| {
            let mut placement = pane_layer_placement(0, 0);
            placement.host_image_id = Some(PANE_GRAPHICS_IMAGE_ID_BIT | id);
            placement.source_key = HostSourceKey::PaneLayer {
                pane_id: placement.pane_id,
                layer_id: name.into(),
            };
            placement.placement.data_fingerprint = fingerprint;
            placement
        })
    };
    let initial = layers(42);
    let live = initial
        .iter()
        .map(|placement| placement.source_key.clone())
        .collect::<HashSet<_>>();
    let mut cache = HostGraphicsCache::default();
    assert!(cache.update_layers(&initial, &live, None).incomplete);
    assert!(cache.update_layers(&layers(43), &live, None).incomplete);
    assert_eq!(cache.images.len(), 2, "the second source uploads next");

    let terminal = |id| {
        let mut placement = test_placement(0, 0);
        placement.placement.image_id = id;
        placement.placement.data_fingerprint = u64::from(id);
        placement.source_key = HostSourceKey::Terminal {
            pane_id: placement.pane_id,
            image_id: id,
        };
        placement
    };
    let second = terminal(99).source_key;
    let mut cache = HostGraphicsCache::default();
    for id in 1..=3 {
        assert!(
            cache
                .update_layers(&[terminal(id), terminal(99)], &HashSet::new(), None)
                .incomplete
        );
    }
    assert!(cache.sources.contains_key(&second));
}

#[test]
fn large_images_are_local_only() {
    let placements = || {
        let mut large = test_placement(0, 0);
        large.placement.data_len = 24 * 1024 * 1024;
        let mut later = test_placement(4, 0);
        later.placement.image_id = 8;
        later.source_key = HostSourceKey::Terminal {
            pane_id: later.pane_id,
            image_id: 8,
        };
        [large, later]
    };
    for (budget, expected) in [
        (None, (true, 1, 0)),
        (Some(GRAPHICS_TRANSACTION_BUDGET), (false, 1, 1)),
    ] {
        let mut cache = HostGraphicsCache::default();
        let encoded = cache.update_layers(&placements(), &HashSet::new(), budget);
        assert!(shows(&encoded.bytes, "a=t"));
        assert_eq!(
            (
                encoded.incomplete,
                cache.images.len(),
                cache.oversized.len()
            ),
            expected
        );
    }
}

#[test]
fn view_identity_reports_a_change_and_clears_the_continuation() {
    let mut cache = HostGraphicsCache::default();
    drain(&mut cache, &[pane_layer_placement(0, 0)], &HashSet::new());
    assert!(
        cache.continuation.is_some(),
        "a frame leaves a resume cursor"
    );

    assert!(cache.update_view(Some(HostView::new(1))));
    assert!(
        cache.continuation.is_none(),
        "a view change drops the resume cursor"
    );
    assert!(!cache.update_view(Some(HostView::new(1))));
    assert!(cache.update_view(Some(HostView::new(2))));
    assert!(cache.update_view(None));
}

/// Debug line a placement at the pane origin emits before it is clipped.
#[cfg(not(target_arch = "wasm32"))]
const ORIGIN_VIEWPORT_CHECK: &str = "clipped_placement: viewport check viewport_col=0 \
     viewport_row=0 area_w=20 area_h=10 scrollback_offset=0 raw_viewport_row=0 \
     cond1=false cond2=false";

/// Buffer one captured debug stream appends to.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);

#[cfg(not(target_arch = "wasm32"))]
impl std::io::Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Debug lines one clipping call emits, without the event header, and its result.
#[cfg(not(target_arch = "wasm32"))]
fn clipping_events(placement: &HostPlacement) -> (Option<(ClippedPlacement, u32)>, Vec<String>) {
    let buffer = Buffer::default();
    let writer = buffer.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_level(false)
        .with_target(false)
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(move || writer.clone())
        .finish();
    let clipped = maestro_test_tracing::with_default(subscriber, || clipped_placement(placement));
    let text = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
    (clipped, text.lines().map(str::to_owned).collect())
}

/// Assert one clipping call's emitted debug lines and whether it kept a placement.
#[cfg(not(target_arch = "wasm32"))]
fn assert_clipping(placement: &HostPlacement, expected: &[&str], kept: bool) {
    let (clipped, events) = clipping_events(placement);
    assert_eq!(events, expected);
    assert_eq!(clipped.is_some(), kept);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn clipping_reports_each_rejection_reason_and_success() {
    let mut zero_area = test_placement(0, 0);
    zero_area.area = Rect::new(0, 0, 0, 10);
    assert_clipping(
        &zero_area,
        &["clipped_placement: area zero area_w=0 area_h=10"],
        false,
    );

    let mut zero_grid = test_placement(0, 0);
    zero_grid.placement.render.grid_cols = 0;
    assert_clipping(
        &zero_grid,
        &["clipped_placement: grid zero grid_cols=0 grid_rows=3"],
        false,
    );

    assert_clipping(
        &test_placement(20, 0),
        &[
            "clipped_placement: viewport check viewport_col=20 viewport_row=0 area_w=20 area_h=10 \
             scrollback_offset=0 raw_viewport_row=0 cond1=true cond2=false",
        ],
        false,
    );

    assert_clipping(
        &test_placement(-3, 0),
        &[
            ORIGIN_VIEWPORT_CHECK,
            "clipped_placement: visible dims check visible_cols=0 visible_rows=3 \
             left_clip_cells=3 top_clip_cells=0",
        ],
        false,
    );

    let mut past_image = test_placement(0, 0);
    past_image.placement.render.source_x = 30;
    assert_clipping(
        &past_image,
        &[
            ORIGIN_VIEWPORT_CHECK,
            "clipped_placement: visible dims check visible_cols=3 visible_rows=3 \
             left_clip_cells=0 top_clip_cells=0",
            "clipped_placement: source dims zero source_width=0 source_height=30 image_width=30 \
             image_height=30",
        ],
        false,
    );

    assert_clipping(
        &test_placement(0, 0),
        &[
            ORIGIN_VIEWPORT_CHECK,
            "clipped_placement: visible dims check visible_cols=3 visible_rows=3 \
             left_clip_cells=0 top_clip_cells=0",
            "clipped_placement: success",
        ],
        true,
    );
}
