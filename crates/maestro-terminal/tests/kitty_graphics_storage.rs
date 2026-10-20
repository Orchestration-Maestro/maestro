//! Screen-owned image quota and placement lifetime behavior.

use maestro_terminal::{
    Screen,
    kitty::graphics::{
        image::Image,
        storage::{ImageStorage, Location, Placement},
    },
    screen::Options,
};

/// Own a tiny image with distinguishable bytes.
fn image(id: u32, data: &[u8]) -> Image {
    Image {
        id,
        data: data.to_vec(),
        ..Image::default()
    }
}

#[test]
fn eviction_accepts_exactly_enough_reclaimed_bytes() {
    let mut screen = Screen::init(Options::default());
    let mut storage = ImageStorage::default();
    storage.set_limit(&mut screen.pages, 8);
    storage
        .add_image(&mut screen.pages, image(1, b"abcd"))
        .unwrap();
    storage
        .add_image(&mut screen.pages, image(2, b"efgh"))
        .unwrap();
    storage
        .add_image(&mut screen.pages, image(3, b"ijkl"))
        .unwrap();
    assert_eq!(storage.images.len(), 2);
    assert!(storage.image_by_id(1).is_none());
    assert_eq!(storage.image_by_id(2).unwrap().data, b"efgh");
    assert_eq!(storage.image_by_id(3).unwrap().data, b"ijkl");
}

#[test]
fn eviction_releases_the_removed_placements_pin() {
    let mut screen = Screen::init(Options::default());
    let baseline = screen.pages.tracked_pins().count();
    let mut storage = ImageStorage::default();
    storage.set_limit(&mut screen.pages, 4);
    storage
        .add_image(&mut screen.pages, image(1, b"abcd"))
        .unwrap();
    let origin = *screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
    let pin = screen.pages.track_pin(origin).unwrap();
    storage
        .add_placement(
            &mut screen.pages,
            1,
            1,
            Placement {
                location: Location::Pin(pin),
                ..Placement::default()
            },
        )
        .unwrap();
    storage
        .add_image(&mut screen.pages, image(2, b"efg"))
        .unwrap();
    assert!(storage.placements.is_empty());
    assert_eq!(screen.pages.tracked_pins().count(), baseline);
}

#[test]
fn external_placement_replacement_releases_the_previous_pin() {
    let mut screen = Screen::init(Options::default());
    let baseline = screen.pages.tracked_pins().count();
    let mut storage = ImageStorage::default();
    storage
        .add_image(&mut screen.pages, image(1, b"abcd"))
        .unwrap();
    for _ in 0..2 {
        let origin = *screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
        let pin = screen.pages.track_pin(origin).unwrap();
        storage
            .add_placement(
                &mut screen.pages,
                1,
                1,
                Placement {
                    location: Location::Pin(pin),
                    ..Placement::default()
                },
            )
            .unwrap();
    }
    assert_eq!(storage.placements.len(), 1);
    assert_eq!(screen.pages.tracked_pins().count(), baseline + 1);
}

#[test]
fn image_numbers_resolve_the_latest_transmission_not_the_largest_id() {
    let mut screen = Screen::init(Options::default());
    let mut storage = ImageStorage::default();
    let mut first = image(100, b"first");
    first.number = 9;
    storage.add_image(&mut screen.pages, first).unwrap();
    let mut second = image(1, b"second");
    second.number = 9;
    storage.add_image(&mut screen.pages, second).unwrap();
    assert_eq!(storage.image_by_number(9).unwrap().id, 1);
    assert!(storage.image_by_number(10).is_none());
    let mut retransmit = image(100, b"again");
    retransmit.number = 9;
    storage.add_image(&mut screen.pages, retransmit).unwrap();
    assert_eq!(storage.image_by_number(9).unwrap().data, b"again");
    assert_eq!(storage.total_bytes, 11);
}

#[test]
fn content_generations_never_recur_across_storages_or_resets() {
    let mut screen = Screen::init(Options::default());
    let mut first = ImageStorage::default();
    let mut second = ImageStorage::default();
    assert_eq!(first.generation, 0);
    first.add_image(&mut screen.pages, image(1, b"a")).unwrap();
    let transmitted = first.generation;
    second.add_image(&mut screen.pages, image(1, b"b")).unwrap();
    assert!(second.generation > transmitted);
    first.clear(&mut screen.pages);
    assert!(first.generation > second.generation);
    let reset = first.generation;
    first.add_image(&mut screen.pages, image(1, b"a")).unwrap();
    assert!(first.generation > reset);
    assert_eq!(first.image_by_id(1).unwrap().generation, first.generation);
    assert!(first.dirty);
}

#[test]
fn quota_prefers_unused_images_before_older_placed_images() {
    let mut screen = Screen::init(Options::default());
    let mut storage = ImageStorage::default();
    storage.set_limit(&mut screen.pages, 8);
    storage
        .add_image(&mut screen.pages, image(1, b"aaaa"))
        .unwrap();
    storage
        .add_placement(&mut screen.pages, 1, 1, Placement::default())
        .unwrap();
    storage
        .add_image(&mut screen.pages, image(2, b"bbbb"))
        .unwrap();
    storage
        .add_image(&mut screen.pages, image(3, b"cccc"))
        .unwrap();
    assert!(storage.image_by_id(1).is_some());
    assert!(storage.image_by_id(2).is_none());
    assert!(storage.image_by_id(3).is_some());
    let before = storage.generation;
    storage.set_limit(&mut screen.pages, 4);
    assert_eq!(storage.total_bytes, 4);
    assert!(storage.image_by_id(1).is_some());
    assert!(storage.generation > before);
}

#[test]
fn anonymous_placements_coexist_and_disabling_releases_all_pins() {
    let mut screen = Screen::init(Options::default());
    let baseline = screen.pages.tracked_pins().count();
    let mut storage = ImageStorage::default();
    storage.image_limits = maestro_terminal::kitty::graphics::image::Limits::ALL;
    storage
        .add_image(&mut screen.pages, image(1, b"aaaa"))
        .unwrap();
    for _ in 0..2 {
        let origin = *screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
        let pin = screen.pages.track_pin(origin).unwrap();
        storage
            .add_placement(
                &mut screen.pages,
                1,
                0,
                Placement {
                    location: Location::Pin(pin),
                    ..Placement::default()
                },
            )
            .unwrap();
    }
    assert_eq!(storage.placements.len(), 2);
    assert_eq!(screen.pages.tracked_pins().count(), baseline + 2);
    let before = storage.generation;
    storage.set_limit(&mut screen.pages, 0);
    assert!(!storage.enabled());
    assert_eq!(storage.total_bytes, 0);
    assert!(storage.images.is_empty());
    assert!(storage.placements.is_empty());
    assert!(storage.generation > before);
    assert!(storage.image_limits.file);
    assert_eq!(screen.pages.tracked_pins().count(), baseline);
}

#[test]
fn oversized_image_is_rejected_without_mutating_storage() {
    let mut screen = Screen::init(Options::default());
    let mut storage = ImageStorage::default();
    storage.set_limit(&mut screen.pages, 4);
    storage
        .add_image(&mut screen.pages, image(1, b"aaaa"))
        .unwrap();
    let before = storage.generation;
    assert!(
        storage
            .add_image(&mut screen.pages, image(2, b"large"))
            .is_err()
    );
    assert_eq!(storage.generation, before);
    assert_eq!(storage.image_by_id(1).unwrap().data, b"aaaa");
    assert_eq!(storage.total_bytes, 4);
}

#[test]
fn placement_sizes_preserve_aspect_ratio_offsets_and_explicit_grid() {
    let image = Image {
        width: 16,
        height: 9,
        ..Image::default()
    };
    let mut p = Placement {
        columns: 10,
        ..Placement::default()
    };
    assert_eq!(p.pixel_size(&image, (10, 20)), (100, 56));
    assert_eq!(p.grid_size(&image, (10, 20)), (10, 3));
    p.columns = 0;
    p.rows = 5;
    assert_eq!(p.pixel_size(&image, (10, 20)), (178, 100));
    p.rows = 0;
    p.x_offset = 5;
    p.y_offset = 12;
    assert_eq!(p.pixel_size(&image, (10, 20)), (16, 9));
    assert_eq!(p.grid_size(&image, (10, 20)), (3, 2));
    assert_eq!(p.grid_size(&image, (0, 0)), (0, 0));
    p.columns = 2;
    p.rows = 3;
    assert_eq!(p.pixel_size(&image, (10, 20)), (20, 60));
    assert_eq!(p.grid_size(&image, (10, 20)), (2, 3));
    p.columns = 0;
    p.rows = 0;
    p.source_width = 7;
    p.source_height = 5;
    assert_eq!(p.pixel_size(&image, (10, 20)), (7, 5));
}

#[test]
fn deletes_preserve_quota_virtual_placements_and_noop_generations() {
    use maestro_terminal::{kitty::graphics::command::Delete, point::Coordinate};
    let (mut screen, mut storage) = rectangle_fixture(false);
    storage.set_limit(&mut screen.pages, 42);
    storage
        .add_image(&mut screen.pages, image(2, b"de"))
        .unwrap();
    storage
        .add_placement(&mut screen.pages, 2, 1, Placement::default())
        .unwrap();
    let generation = storage.generation;
    storage.dirty = false;
    storage.delete(
        &mut screen.pages,
        (10, 20),
        Coordinate::default(),
        Delete::AnimationFrames(true),
    );
    assert_eq!(storage.generation, generation);
    assert!(!storage.dirty);
    storage.delete(
        &mut screen.pages,
        (10, 20),
        Coordinate::default(),
        Delete::All(true),
    );
    assert_eq!(storage.placements.len(), 1);
    assert!(storage.image_by_id(1).is_none());
    assert!(storage.image_by_id(2).is_some());
    assert_eq!(storage.total_bytes, 2);
    assert_eq!(storage.total_limit, 42);
    assert!(storage.generation > generation);
    let generation = storage.generation;
    storage.delete(
        &mut screen.pages,
        (10, 20),
        Coordinate::default(),
        Delete::All(true),
    );
    assert_eq!(storage.generation, generation);
    storage.delete(
        &mut screen.pages,
        (10, 20),
        Coordinate::default(),
        Delete::Id {
            delete: true,
            image_id: 2,
            placement_id: 1,
        },
    );
    assert!(storage.images.is_empty());
    assert!(storage.placements.is_empty());
    assert_eq!(storage.total_bytes, 0);
}

/// A two-column image spanning all three rows, starting in the second column.
fn rectangle_fixture(split: bool) -> (Screen, ImageStorage) {
    use maestro_terminal::{Point, point::Coordinate};
    let mut screen = Screen::init(Options {
        cols: std::num::NonZeroU16::new(5).unwrap(),
        rows: std::num::NonZeroU16::new(3).unwrap(),
        ..Options::default()
    });
    let mut storage = ImageStorage::default();
    storage
        .add_image(
            &mut screen.pages,
            Image {
                id: 1,
                width: 20,
                height: 60,
                data: vec![1; 4],
                ..Image::default()
            },
        )
        .unwrap();
    let origin = screen
        .pages
        .pin(Point::Active(Coordinate { x: 1, y: 0 }))
        .unwrap();
    let pin = screen.pages.track_pin(origin).unwrap();
    storage
        .add_placement(
            &mut screen.pages,
            1,
            1,
            Placement {
                location: Location::Pin(pin),
                columns: 2,
                rows: 3,
                z: 7,
                ..Placement::default()
            },
        )
        .unwrap();
    if split {
        let at = screen
            .pages
            .pin(Point::Active(Coordinate { x: 0, y: 1 }))
            .unwrap();
        screen.pages.split(at).unwrap();
    }
    (screen, storage)
}

#[test]
fn row_deletion_spans_split_pages_including_both_boundary_rows() {
    use maestro_terminal::{kitty::graphics::command::Delete, point::Coordinate};
    for split in [false, true] {
        for y in 1..=3 {
            let (mut screen, mut storage) = rectangle_fixture(split);
            storage.delete(
                &mut screen.pages,
                (10, 20),
                Coordinate::default(),
                Delete::Row { delete: false, y },
            );
            assert!(storage.placements.is_empty(), "split={split} row={y}");
            assert_eq!(storage.images.len(), 1);
        }
    }
}

#[test]
fn cell_cursor_and_z_deletion_require_rectangle_column_admission() {
    use maestro_terminal::{kitty::graphics::command::Delete, point::Coordinate};
    for split in [false, true] {
        for (x, y) in (1..=3).flat_map(|y| (1..=5).map(move |x| (x, y))) {
            for route in 0..4 {
                let (mut screen, mut storage) = rectangle_fixture(split);
                let command = match route {
                    0 => Delete::IntersectCell { delete: true, x, y },
                    1 => Delete::IntersectCursor(true),
                    2 => Delete::IntersectCellZ {
                        delete: true,
                        x,
                        y,
                        z: 7,
                    },
                    _ => Delete::IntersectCellZ {
                        delete: true,
                        x,
                        y,
                        z: 8,
                    },
                };
                let cursor = Coordinate {
                    x: u16::try_from(x - 1).unwrap(),
                    y: y - 1,
                };
                storage.delete(&mut screen.pages, (10, 20), cursor, command);
                let removed = (2..=3).contains(&x) && route != 3;
                assert_eq!(
                    storage.placements.len(),
                    usize::from(!removed),
                    "split={split} x={x} y={y} route={route}"
                );
                assert_eq!(storage.images.len(), usize::from(!removed));
            }
        }
    }
}

#[test]
fn range_deletion_keeps_both_outside_controls() {
    use maestro_terminal::{kitty::graphics::command::Delete, point::Coordinate};
    for delete in [false, true] {
        for (first, last, expected) in [(1, 2, vec![3]), (2, 2, vec![1, 3]), (1, 1, vec![2, 3])] {
            let (mut screen, mut storage) = rectangle_fixture(false);
            for id in [2, 3] {
                storage
                    .add_image(&mut screen.pages, image(id, b"ab"))
                    .unwrap();
                storage
                    .add_placement(&mut screen.pages, id, 1, Placement::default())
                    .unwrap();
            }
            storage.delete(
                &mut screen.pages,
                (10, 20),
                Coordinate::default(),
                Delete::Range {
                    delete,
                    first,
                    last,
                },
            );
            let mut ids: Vec<_> = storage.placements.keys().map(|key| key.image_id).collect();
            ids.sort_unstable();
            assert_eq!(ids, expected);
            assert_eq!(
                storage.images.len(),
                if delete { expected.len() } else { 3 }
            );
        }
    }
}

#[test]
fn id_and_newest_deletion_keep_other_placements_until_the_last_user() {
    use maestro_terminal::{
        kitty::graphics::{command::Delete, storage::PlacementId},
        point::Coordinate,
    };
    for newest in [false, true] {
        let (mut screen, mut storage) = rectangle_fixture(false);
        storage.images.get_mut(&1).unwrap().number = 9;
        let mut other = image(2, b"second");
        other.number = 9;
        storage.add_image(&mut screen.pages, other).unwrap();
        for pid in [1, 2] {
            storage
                .add_placement(&mut screen.pages, 2, pid, Placement::default())
                .unwrap();
        }
        let cmd = if newest {
            Delete::Newest {
                delete: true,
                image_number: 9,
                placement_id: 1,
            }
        } else {
            Delete::Id {
                delete: true,
                image_id: 2,
                placement_id: 1,
            }
        };
        storage.delete(&mut screen.pages, (10, 20), Coordinate::default(), cmd);
        assert_eq!(storage.placements.len(), 2);
        assert!(
            storage
                .placements
                .keys()
                .any(|key| key.image_id == 2 && key.placement_id == PlacementId::External(2))
        );
        assert!(storage.image_by_id(2).is_some());
        storage.delete(
            &mut screen.pages,
            (10, 20),
            Coordinate::default(),
            Delete::Id {
                delete: true,
                image_id: 2,
                placement_id: 0,
            },
        );
        assert_eq!(storage.placements.len(), 1);
        assert!(storage.image_by_id(2).is_none());
    }
}

#[test]
fn columns_rows_and_z_remove_only_their_selected_physical_placements() {
    use maestro_terminal::{kitty::graphics::command::Delete, point::Coordinate};
    for one_cell in [false, true] {
        for x in 1..=5 {
            let (mut screen, mut storage) = rectangle_fixture(true);
            if one_cell {
                let placement = storage.placements.values_mut().next().unwrap();
                placement.columns = 1;
                placement.rows = 1;
            }
            let baseline = screen.pages.tracked_pins().count();
            storage.delete(
                &mut screen.pages,
                (10, 20),
                Coordinate::default(),
                Delete::Column { delete: false, x },
            );
            let removed = x == 2 || (!one_cell && x == 3);
            assert_eq!(storage.placements.len(), usize::from(!removed));
            assert_eq!(
                screen.pages.tracked_pins().count(),
                baseline - usize::from(removed)
            );
        }
        for y in 1..=3 {
            let (mut screen, mut storage) = rectangle_fixture(true);
            if one_cell {
                let placement = storage.placements.values_mut().next().unwrap();
                placement.columns = 1;
                placement.rows = 1;
            }
            storage.delete(
                &mut screen.pages,
                (10, 20),
                Coordinate::default(),
                Delete::Row { delete: true, y },
            );
            assert_eq!(storage.placements.len(), usize::from(one_cell && y != 1));
        }
    }
    let (mut screen, mut storage) = rectangle_fixture(false);
    storage
        .add_placement(
            &mut screen.pages,
            1,
            2,
            Placement {
                z: 7,
                ..Placement::default()
            },
        )
        .unwrap();
    storage.delete(
        &mut screen.pages,
        (10, 20),
        Coordinate::default(),
        Delete::Z { delete: true, z: 7 },
    );
    assert_eq!(storage.placements.len(), 1);
    assert!(storage.image_by_id(1).is_some());
}

#[test]
fn invalid_and_unmatched_deletes_leave_content_and_generation_unchanged() {
    use maestro_terminal::{kitty::graphics::command::Delete, point::Coordinate};
    let (mut screen, mut storage) = rectangle_fixture(false);
    let generation = storage.generation;
    storage.dirty = false;
    let commands = [
        Delete::IntersectCell {
            delete: true,
            x: 0,
            y: 1,
        },
        Delete::IntersectCell {
            delete: true,
            x: 2,
            y: 0,
        },
        Delete::IntersectCell {
            delete: true,
            x: u32::MAX,
            y: 2,
        },
        Delete::IntersectCellZ {
            delete: true,
            x: 2,
            y: 4,
            z: 7,
        },
        Delete::Column { delete: true, x: 0 },
        Delete::Row { delete: true, y: 0 },
        Delete::Row { delete: true, y: 4 },
        Delete::Z { delete: true, z: 8 },
        Delete::Range {
            delete: true,
            first: 3,
            last: 2,
        },
        Delete::Range {
            delete: true,
            first: 0,
            last: 2,
        },
        Delete::Newest {
            delete: true,
            image_number: 99,
            placement_id: 0,
        },
    ];
    for command in commands {
        storage.delete(&mut screen.pages, (10, 20), Coordinate::default(), command);
        assert_eq!(storage.generation, generation);
        assert!(!storage.dirty);
        assert_eq!(storage.placements.len(), 1);
    }
}

#[test]
fn delete_all_keeps_unplaced_images_unless_uppercase_was_requested() {
    use maestro_terminal::{kitty::graphics::command::Delete, point::Coordinate};
    let (mut screen, mut storage) = rectangle_fixture(false);
    let baseline = screen.pages.tracked_pins().count();
    storage
        .add_image(&mut screen.pages, image(2, b"unused"))
        .unwrap();
    storage.delete(
        &mut screen.pages,
        (10, 20),
        Coordinate::default(),
        Delete::All(false),
    );
    assert!(storage.placements.is_empty());
    assert_eq!(storage.images.len(), 2);
    assert_eq!(screen.pages.tracked_pins().count(), baseline - 1);
    storage.delete(
        &mut screen.pages,
        (10, 20),
        Coordinate::default(),
        Delete::All(true),
    );
    assert!(storage.images.is_empty());
    let generation = storage.generation;
    storage.dirty = false;
    storage.delete(
        &mut screen.pages,
        (10, 20),
        Coordinate::default(),
        Delete::All(true),
    );
    assert_eq!(storage.generation, generation);
    assert!(!storage.dirty);
}

#[test]
fn retained_rectangle_clips_at_screen_edges_and_excludes_virtual_or_empty_layouts() {
    let (screen, mut storage) = rectangle_fixture(true);
    let image = storage.images.get(&1).unwrap();
    let placement = storage.placements.values_mut().next().unwrap();
    let rect = placement.rect(image, &screen.pages, (10, 20)).unwrap();
    assert_eq!((rect.top_left.x, rect.top_left.y), (1, 0));
    assert_eq!((rect.bottom_right.x, rect.bottom_right.y), (2, 1));
    assert_ne!(rect.top_left.node, rect.bottom_right.node);
    placement.columns = 100;
    placement.rows = 100;
    let rect = placement.rect(image, &screen.pages, (10, 20)).unwrap();
    assert_eq!((rect.bottom_right.x, rect.bottom_right.y), (4, 1));
    placement.columns = 0;
    placement.rows = 0;
    assert!(placement.rect(image, &screen.pages, (0, 0)).is_none());
    placement.location = Location::Virtual;
    assert!(placement.rect(image, &screen.pages, (10, 20)).is_none());
}

#[test]
fn cursor_deletion_removes_one_or_all_overlapping_placements_and_only_unused_images() {
    use maestro_terminal::{Point, kitty::graphics::command::Delete, point::Coordinate};
    for (cursor, remaining) in [
        (Coordinate { x: 1, y: 0 }, 1),
        (Coordinate { x: 2, y: 1 }, 0),
    ] {
        for delete in [false, true] {
            let (mut screen, mut storage) = rectangle_fixture(false);
            storage
                .add_image(&mut screen.pages, image(2, b"unplaced"))
                .unwrap();
            let origin = screen
                .pages
                .pin(Point::Active(Coordinate { x: 2, y: 1 }))
                .unwrap();
            let pin = screen.pages.track_pin(origin).unwrap();
            storage
                .add_placement(
                    &mut screen.pages,
                    1,
                    2,
                    Placement {
                        location: Location::Pin(pin),
                        columns: 2,
                        rows: 2,
                        ..Placement::default()
                    },
                )
                .unwrap();
            let baseline = screen.pages.tracked_pins().count();
            storage.delete(
                &mut screen.pages,
                (10, 20),
                cursor,
                Delete::IntersectCursor(delete),
            );
            assert_eq!(storage.placements.len(), remaining);
            assert_eq!(
                screen.pages.tracked_pins().count(),
                baseline - (2 - remaining)
            );
            assert_eq!(
                storage.images.len(),
                if delete && remaining == 0 { 1 } else { 2 }
            );
            assert!(storage.image_by_id(2).is_some());
        }
    }
}

#[test]
fn placement_mutations_leave_the_image_payload_generation_unchanged() {
    use maestro_terminal::{kitty::graphics::command::Delete, point::Coordinate};
    let (mut screen, mut storage) = rectangle_fixture(false);
    let payload_generation = storage.image_by_id(1).unwrap().generation;
    let generation = storage.generation;
    storage.dirty = false;
    storage
        .add_placement(&mut screen.pages, 1, 2, Placement::default())
        .unwrap();
    assert!(storage.generation > generation);
    assert!(storage.dirty);
    let generation = storage.generation;
    storage.delete(
        &mut screen.pages,
        (10, 20),
        Coordinate::default(),
        Delete::Id {
            delete: false,
            image_id: 1,
            placement_id: 2,
        },
    );
    assert!(storage.generation > generation);
    assert_eq!(
        storage.image_by_id(1).unwrap().generation,
        payload_generation
    );
}
