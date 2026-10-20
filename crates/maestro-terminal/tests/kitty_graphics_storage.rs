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
