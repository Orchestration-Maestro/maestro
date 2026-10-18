//! Identity and last-owner release of shared storage values.

use super::RefCountedSet;
use crate::style::{Color, Style};

#[test]
fn set_identity_and_last_release() {
    let mut set = RefCountedSet::init(16);
    let value = Style {
        fg_color: Color::Palette(7),
        ..Style::default()
    };
    let first = set.add(value).unwrap();
    let shared = set.add(value).unwrap();
    let distinct = set
        .add(Style {
            fg_color: Color::Palette(8),
            ..value
        })
        .unwrap();
    assert_ne!(first, 0);
    assert_eq!(first, shared);
    assert_ne!(first, distinct);
    assert_eq!(set.ref_count(first), 2);
    set.use_multiple(first, 2);
    assert_eq!(set.ref_count(first), 4);
    assert!(set.release_multiple(first, 3).is_none());
    assert_eq!(*set.get(first), value);
    assert_eq!(set.lookup(&value), Some(first));
    let retained = set.release_multiple(first, 1).unwrap();
    assert_eq!(*retained, value);
    assert_eq!(set.lookup(&value), None);
    assert_eq!(set.count(), 1);
    assert_eq!(set.get(distinct).fg_color, Color::Palette(8));
}

#[test]
fn set_zero_capacity() {
    let mut set = RefCountedSet::init(0);
    assert_eq!(set.count(), 0);
    assert_eq!(
        set.add(Style::default()),
        Err(crate::page::StorageError::OutOfMemory)
    );
    assert_eq!(set.count(), 0);
}

#[test]
fn set_layout_rounding() {
    for &(requested, styles, links) in SET_LAYOUTS {
        assert_eq!(super::layout(requested, 36, 4), styles);
        assert_eq!(super::layout(requested, 48, 8), links);
    }
    for (count, expected) in [
        (0, 0),
        (1, 3),
        (2, 4),
        (3, 5),
        (4, 7),
        (7, 10),
        (8, 12),
        (15, 20),
        (16, 21),
        (17, 23),
        (63, 79),
        (64, 80),
        (65, 82),
        (128, 159),
        (16_384, 20_167),
        (65_535, 80_660),
    ] {
        assert_eq!(RefCountedSet::<Style>::capacity_for_count(count), expected);
    }
}

#[test]
fn set_capacity_controls() {
    for (requested, living) in [
        (0, 0),
        (1, 0),
        (2, 0),
        (3, 2),
        (4, 2),
        (7, 5),
        (8, 5),
        (15, 12),
        (16, 12),
        (17, 25),
        (63, 51),
        (64, 51),
        (65, 103),
        (128, 103),
        (16_384, 13_311),
        (65_535, 53_247),
    ] {
        let mut set = RefCountedSet::init(requested);
        let mut ids = Vec::new();
        for value in 0..living {
            let id = set.add(value).unwrap();
            assert_eq!(*set.get(id), value);
            assert_eq!(set.add(value).unwrap(), id);
            assert_eq!(set.ref_count(id), 2);
            ids.push(id);
        }
        assert_eq!(set.add(living), Err(crate::page::StorageError::OutOfMemory));
        assert_eq!(set.count(), living);
        for id in ids {
            assert!(set.release_multiple(id, 1).is_none());
            assert!(set.release_multiple(id, 1).is_some());
        }
        assert_eq!(set.count(), 0);
        if living != 0 {
            let id = set.add(living).unwrap();
            assert_eq!(*set.get(id), living);
        }
    }
}

mod outcomes;

#[test]
fn set_visible_outcomes_survive_reuse() {
    for &(released, new, expected_cp, expected_palette, expected_live) in outcomes::CONTROLS {
        let mut set = RefCountedSet::init(16);
        let mut cells = [crate::Cell::init('A'); 10];
        for (palette, cell) in (0..10).zip(&mut cells) {
            cell.set_style_id(
                set.add(Style {
                    fg_color: Color::Palette(palette),
                    ..Style::default()
                })
                .unwrap(),
            );
        }
        assert!(
            set.release_multiple(cells[usize::from(released)].style_id(), 1)
                .is_some()
        );
        let mut replacement = crate::Cell::init('B');
        replacement.set_style_id(
            set.add(Style {
                fg_color: Color::Palette(new),
                ..Style::default()
            })
            .unwrap(),
        );
        assert_eq!(replacement.codepoint(), expected_cp);
        assert_eq!(
            set.get(replacement.style_id()).fg_color,
            Color::Palette(expected_palette)
        );
        let live: Vec<_> = cells
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != usize::from(released))
            .map(|(_, cell)| set.get(cell.style_id()).fg_color)
            .collect();
        assert_eq!(live, expected_live.map(Color::Palette));
        assert_eq!(set.count(), 10);
    }
    let mut set = RefCountedSet::init(16);
    let ids: Vec<_> = (0..12).map(|value| set.add(value).unwrap()).collect();
    assert_eq!(set.add(12), Err(crate::page::StorageError::OutOfMemory));
    assert_eq!(*set.release_multiple(ids[4], 1).unwrap(), 4);
    let replacement = set.add(200).unwrap();
    assert_eq!(*set.get(replacement), 200);
    for (value, id) in ids.into_iter().enumerate().filter(|(value, _)| *value != 4) {
        assert_eq!(*set.get(id), value);
    }
}

/// Complete logical style and hyperlink set layouts at retained boundaries.
const SET_LAYOUTS: &[(u64, super::Layout, super::Layout)] = &[
    (
        0,
        super::Layout {
            cap: 0,
            table_cap: 0,
            table_mask: 0,
            table_start: 0,
            items_start: 0,
            total_size: 0,
        },
        super::Layout {
            cap: 0,
            table_cap: 0,
            table_mask: 0,
            table_start: 0,
            items_start: 0,
            total_size: 0,
        },
    ),
    (
        1,
        super::Layout {
            cap: 0,
            table_cap: 1,
            table_mask: 0,
            table_start: 0,
            items_start: 4,
            total_size: 4,
        },
        super::Layout {
            cap: 0,
            table_cap: 1,
            table_mask: 0,
            table_start: 0,
            items_start: 8,
            total_size: 8,
        },
    ),
    (
        2,
        super::Layout {
            cap: 1,
            table_cap: 2,
            table_mask: 1,
            table_start: 0,
            items_start: 4,
            total_size: 40,
        },
        super::Layout {
            cap: 1,
            table_cap: 2,
            table_mask: 1,
            table_start: 0,
            items_start: 8,
            total_size: 56,
        },
    ),
    (
        3,
        super::Layout {
            cap: 3,
            table_cap: 4,
            table_mask: 3,
            table_start: 0,
            items_start: 8,
            total_size: 116,
        },
        super::Layout {
            cap: 3,
            table_cap: 4,
            table_mask: 3,
            table_start: 0,
            items_start: 8,
            total_size: 152,
        },
    ),
    (
        4,
        super::Layout {
            cap: 3,
            table_cap: 4,
            table_mask: 3,
            table_start: 0,
            items_start: 8,
            total_size: 116,
        },
        super::Layout {
            cap: 3,
            table_cap: 4,
            table_mask: 3,
            table_start: 0,
            items_start: 8,
            total_size: 152,
        },
    ),
    (
        7,
        super::Layout {
            cap: 6,
            table_cap: 8,
            table_mask: 7,
            table_start: 0,
            items_start: 16,
            total_size: 232,
        },
        super::Layout {
            cap: 6,
            table_cap: 8,
            table_mask: 7,
            table_start: 0,
            items_start: 16,
            total_size: 304,
        },
    ),
    (
        8,
        super::Layout {
            cap: 6,
            table_cap: 8,
            table_mask: 7,
            table_start: 0,
            items_start: 16,
            total_size: 232,
        },
        super::Layout {
            cap: 6,
            table_cap: 8,
            table_mask: 7,
            table_start: 0,
            items_start: 16,
            total_size: 304,
        },
    ),
    (
        15,
        super::Layout {
            cap: 13,
            table_cap: 16,
            table_mask: 15,
            table_start: 0,
            items_start: 32,
            total_size: 500,
        },
        super::Layout {
            cap: 13,
            table_cap: 16,
            table_mask: 15,
            table_start: 0,
            items_start: 32,
            total_size: 656,
        },
    ),
    (
        16,
        super::Layout {
            cap: 13,
            table_cap: 16,
            table_mask: 15,
            table_start: 0,
            items_start: 32,
            total_size: 500,
        },
        super::Layout {
            cap: 13,
            table_cap: 16,
            table_mask: 15,
            table_start: 0,
            items_start: 32,
            total_size: 656,
        },
    ),
    (
        17,
        super::Layout {
            cap: 26,
            table_cap: 32,
            table_mask: 31,
            table_start: 0,
            items_start: 64,
            total_size: 1_000,
        },
        super::Layout {
            cap: 26,
            table_cap: 32,
            table_mask: 31,
            table_start: 0,
            items_start: 64,
            total_size: 1_312,
        },
    ),
    (
        63,
        super::Layout {
            cap: 52,
            table_cap: 64,
            table_mask: 63,
            table_start: 0,
            items_start: 128,
            total_size: 2_000,
        },
        super::Layout {
            cap: 52,
            table_cap: 64,
            table_mask: 63,
            table_start: 0,
            items_start: 128,
            total_size: 2_624,
        },
    ),
    (
        64,
        super::Layout {
            cap: 52,
            table_cap: 64,
            table_mask: 63,
            table_start: 0,
            items_start: 128,
            total_size: 2_000,
        },
        super::Layout {
            cap: 52,
            table_cap: 64,
            table_mask: 63,
            table_start: 0,
            items_start: 128,
            total_size: 2_624,
        },
    ),
    (
        65,
        super::Layout {
            cap: 104,
            table_cap: 128,
            table_mask: 127,
            table_start: 0,
            items_start: 256,
            total_size: 4_000,
        },
        super::Layout {
            cap: 104,
            table_cap: 128,
            table_mask: 127,
            table_start: 0,
            items_start: 256,
            total_size: 5_248,
        },
    ),
    (
        128,
        super::Layout {
            cap: 104,
            table_cap: 128,
            table_mask: 127,
            table_start: 0,
            items_start: 256,
            total_size: 4_000,
        },
        super::Layout {
            cap: 104,
            table_cap: 128,
            table_mask: 127,
            table_start: 0,
            items_start: 256,
            total_size: 5_248,
        },
    ),
    (
        16_384,
        super::Layout {
            cap: 13_312,
            table_cap: 16_384,
            table_mask: 16_383,
            table_start: 0,
            items_start: 32_768,
            total_size: 512_000,
        },
        super::Layout {
            cap: 13_312,
            table_cap: 16_384,
            table_mask: 16_383,
            table_start: 0,
            items_start: 32_768,
            total_size: 671_744,
        },
    ),
    (
        65_535,
        super::Layout {
            cap: 53_248,
            table_cap: 65_536,
            table_mask: 65_535,
            table_start: 0,
            items_start: 131_072,
            total_size: 2_048_000,
        },
        super::Layout {
            cap: 53_248,
            table_cap: 65_536,
            table_mask: 65_535,
            table_start: 0,
            items_start: 131_072,
            total_size: 2_686_976,
        },
    ),
];
