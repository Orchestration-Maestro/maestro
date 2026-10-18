//! Complete logical region comparisons on both page-rounding sizes.

use super::{Capacity, Layout};

#[test]
fn layout_regions_match_both_page_sizes() {
    for &(page_size, capacity, expected) in LAYOUTS {
        assert_eq!(super::layout_for_page_size(capacity, page_size), expected);
    }
}

#[test]
fn layout_accepts_maximum_capacity() {
    let capacity = Capacity {
        cols: u16::MAX,
        rows: u16::MAX,
        styles: u16::MAX,
        hyperlink_bytes: u16::MAX,
        grapheme_bytes: u32::MAX,
        string_bytes: u32::MAX,
    };
    let layout = super::layout_for_page_size(capacity, 4096);
    assert_eq!(layout.total_size, 48_638_988_288);
}

#[test]
fn capacity_decreases_columns() {
    let capacity = super::STD_CAPACITY
        .adjust(super::Adjustment { cols: Some(107) })
        .unwrap();
    assert_eq!(capacity.rows, native_rows(107));
    assert_eq!(
        super::layout_for_page_size(capacity, super::PAGE_SIZE).total_size,
        native_total()
    );
    assert!(
        super::layout_for_page_size(
            Capacity {
                rows: capacity.rows + 1,
                ..capacity
            },
            4096
        )
        .total_size
            > native_total()
    );
}

#[test]
fn records_and_capacity_boundaries_match() {
    fn copy_record<T: Copy>() {}
    copy_record::<crate::Cell>();
    copy_record::<crate::page::row::Row>();
    assert_eq!(std::mem::size_of::<crate::Cell>(), 8);
    assert_eq!(std::mem::size_of::<crate::page::row::Row>(), 8);
    assert_eq!(
        Capacity::default(),
        Capacity {
            cols: 0,
            rows: 0,
            styles: 16,
            hyperlink_bytes: 192,
            grapheme_bytes: 1024,
            string_bytes: 2048
        }
    );
    assert_eq!(
        super::STD_CAPACITY,
        Capacity {
            cols: 215,
            rows: 215,
            styles: 128,
            hyperlink_bytes: 192,
            grapheme_bytes: 8192,
            string_bytes: 2048
        }
    );
    assert_eq!(crate::size::MAX_PAGE_SIZE, u32::MAX);
    let zero = Capacity {
        cols: 0,
        rows: 0,
        styles: 0,
        hyperlink_bytes: 0,
        grapheme_bytes: 0,
        string_bytes: 0,
    };
    assert_eq!(super::layout_for_page_size(zero, 4096).total_size, 4096);
    assert_eq!(super::layout_for_page_size(zero, 16_384).total_size, 16_384);
    let maximum = Capacity {
        cols: u16::MAX,
        rows: u16::MAX,
        styles: u16::MAX,
        hyperlink_bytes: u16::MAX,
        grapheme_bytes: u32::MAX,
        string_bytes: u32::MAX,
    };
    assert_eq!(
        super::layout_for_page_size(maximum, 16_384).total_size,
        48_639_000_576
    );
}

#[test]
fn capacity_decreases_to_one_column() {
    let capacity = super::STD_CAPACITY
        .adjust(super::Adjustment { cols: Some(1) })
        .unwrap();
    assert_eq!(capacity.rows, native_rows(1));
    assert_eq!(
        super::layout_for_page_size(capacity, super::PAGE_SIZE).total_size,
        native_total()
    );
    assert!(
        super::layout_for_page_size(
            Capacity {
                rows: capacity.rows + 1,
                ..capacity
            },
            4096
        )
        .total_size
            > native_total()
    );
}

#[test]
fn capacity_increases_columns() {
    let capacity = super::STD_CAPACITY
        .adjust(super::Adjustment { cols: Some(430) })
        .unwrap();
    assert_eq!(capacity.rows, native_rows(430));
    assert_eq!(
        super::layout_for_page_size(capacity, super::PAGE_SIZE).total_size,
        native_total()
    );
    assert!(
        super::layout_for_page_size(
            Capacity {
                rows: capacity.rows + 1,
                ..capacity
            },
            4096
        )
        .total_size
            > native_total()
    );
}

#[test]
fn capacity_adjust_columns_sweep() {
    let mut capacity = super::STD_CAPACITY;
    assert_eq!(
        capacity.adjust(super::Adjustment::default()).unwrap(),
        capacity
    );
    assert_eq!(
        capacity
            .adjust(super::Adjustment { cols: Some(0) })
            .unwrap()
            .rows,
        native_rows(0)
    );
    for cols in 1..430 {
        capacity = capacity
            .adjust(super::Adjustment { cols: Some(cols) })
            .unwrap();
        assert_eq!(
            super::layout_for_page_size(capacity, super::PAGE_SIZE).total_size,
            native_total()
        );
        assert!(
            super::layout_for_page_size(
                Capacity {
                    rows: capacity.rows + 1,
                    ..capacity
                },
                4096
            )
            .total_size
                > native_total()
        );
    }
}

#[test]
fn capacity_rejects_unfittable_columns() {
    assert_eq!(
        super::STD_CAPACITY.adjust(super::Adjustment { cols: Some(65_535) }),
        Err(crate::page::CapacityError)
    );
}

#[test]
fn capacity_maximum_columns() {
    let capacity = super::STD_CAPACITY;
    let maximum = capacity.max_cols().unwrap();
    assert_eq!(maximum, native_maximum());
    assert!(maximum >= capacity.cols);
    assert!(
        capacity
            .adjust(super::Adjustment {
                cols: Some(maximum)
            })
            .unwrap()
            .rows
            >= 1
    );
    assert_eq!(
        capacity.adjust(super::Adjustment {
            cols: Some(maximum + 1)
        }),
        Err(crate::page::CapacityError)
    );
}

#[test]
fn capacity_column_limit_keeps_byte_size() {
    let capacity = super::STD_CAPACITY;
    let adjusted = capacity
        .adjust(super::Adjustment {
            cols: capacity.max_cols(),
        })
        .unwrap();
    assert_eq!(
        super::layout_for_page_size(adjusted, super::PAGE_SIZE).total_size,
        native_total()
    );
}

#[test]
fn capacity_one_row_column_limit() {
    let capacity = super::STD_CAPACITY;
    assert_eq!(
        capacity
            .adjust(super::Adjustment {
                cols: capacity.max_cols()
            })
            .unwrap()
            .rows,
        1
    );
}

#[test]
fn layout_rounds_link_map_once() {
    let layout = super::layout_for_page_size(
        Capacity {
            hyperlink_bytes: 144,
            ..Capacity::default()
        },
        4096,
    );
    assert_eq!(layout.hyperlink_map.capacity, 64);
    assert_eq!(layout.hyperlink_map.total_size, 464);
    assert_eq!(layout.hyperlink_set.table_cap, 4);
}

/// Independent complete region records from the storage producer.
const LAYOUTS: &[(u64, Capacity, Layout)] = &[
    (
        4096,
        Capacity {
            cols: 215,
            rows: 215,
            styles: 128,
            hyperlink_bytes: 192,
            grapheme_bytes: 8192,
            string_bytes: 2048,
        },
        Layout {
            total_size: 401_408,
            rows_start: 0,
            rows_size: 1720,
            cells_start: 1720,
            cells_size: 369_800,
            styles_start: 371_520,
            styles: crate::ref_counted_set::Layout {
                cap: 104,
                table_cap: 128,
                table_mask: 127,
                table_start: 0,
                items_start: 256,
                total_size: 4000,
            },
            grapheme_alloc_start: 375_520,
            grapheme_alloc: crate::bitmap_allocator::Layout {
                total_size: 8256,
                bitmap_count: 8,
                bitmap_start: 0,
                chunks_start: 64,
            },
            grapheme_map_start: 383_776,
            grapheme_map: crate::hash_map::Layout {
                total_size: 10_768,
                keys_start: 512,
                vals_start: 2560,
                capacity: 512,
            },
            string_alloc_start: 394_544,
            string_alloc: crate::bitmap_allocator::Layout {
                total_size: 2056,
                bitmap_count: 1,
                bitmap_start: 0,
                chunks_start: 8,
            },
            hyperlink_map_start: 396_752,
            hyperlink_map: crate::hash_map::Layout {
                total_size: 912,
                keys_start: 128,
                vals_start: 640,
                capacity: 128,
            },
            hyperlink_set_start: 396_600,
            hyperlink_set: crate::ref_counted_set::Layout {
                cap: 3,
                table_cap: 4,
                table_mask: 3,
                table_start: 0,
                items_start: 8,
                total_size: 152,
            },
            capacity: Capacity {
                cols: 215,
                rows: 215,
                styles: 128,
                hyperlink_bytes: 192,
                grapheme_bytes: 8192,
                string_bytes: 2048,
            },
        },
    ),
    (
        4096,
        Capacity {
            cols: 1,
            rows: 1,
            styles: 16,
            hyperlink_bytes: 192,
            grapheme_bytes: 1024,
            string_bytes: 2048,
        },
        Layout {
            total_size: 8192,
            rows_start: 0,
            rows_size: 8,
            cells_start: 8,
            cells_size: 8,
            styles_start: 16,
            styles: crate::ref_counted_set::Layout {
                cap: 13,
                table_cap: 16,
                table_mask: 15,
                table_start: 0,
                items_start: 32,
                total_size: 500,
            },
            grapheme_alloc_start: 520,
            grapheme_alloc: crate::bitmap_allocator::Layout {
                total_size: 1032,
                bitmap_count: 1,
                bitmap_start: 0,
                chunks_start: 8,
            },
            grapheme_map_start: 1552,
            grapheme_map: crate::hash_map::Layout {
                total_size: 1360,
                keys_start: 64,
                vals_start: 320,
                capacity: 64,
            },
            string_alloc_start: 2912,
            string_alloc: crate::bitmap_allocator::Layout {
                total_size: 2056,
                bitmap_count: 1,
                bitmap_start: 0,
                chunks_start: 8,
            },
            hyperlink_map_start: 5120,
            hyperlink_map: crate::hash_map::Layout {
                total_size: 912,
                keys_start: 128,
                vals_start: 640,
                capacity: 128,
            },
            hyperlink_set_start: 4968,
            hyperlink_set: crate::ref_counted_set::Layout {
                cap: 3,
                table_cap: 4,
                table_mask: 3,
                table_start: 0,
                items_start: 8,
                total_size: 152,
            },
            capacity: Capacity {
                cols: 1,
                rows: 1,
                styles: 16,
                hyperlink_bytes: 192,
                grapheme_bytes: 1024,
                string_bytes: 2048,
            },
        },
    ),
    (
        4096,
        Capacity {
            cols: 0,
            rows: 0,
            styles: 0,
            hyperlink_bytes: 0,
            grapheme_bytes: 0,
            string_bytes: 0,
        },
        Layout {
            total_size: 4096,
            rows_start: 0,
            rows_size: 0,
            cells_start: 0,
            cells_size: 0,
            styles_start: 0,
            styles: crate::ref_counted_set::Layout {
                cap: 0,
                table_cap: 0,
                table_mask: 0,
                table_start: 0,
                items_start: 0,
                total_size: 0,
            },
            grapheme_alloc_start: 0,
            grapheme_alloc: crate::bitmap_allocator::Layout {
                total_size: 0,
                bitmap_count: 0,
                bitmap_start: 0,
                chunks_start: 0,
            },
            grapheme_map_start: 0,
            grapheme_map: crate::hash_map::Layout {
                total_size: 16,
                keys_start: 0,
                vals_start: 0,
                capacity: 0,
            },
            string_alloc_start: 16,
            string_alloc: crate::bitmap_allocator::Layout {
                total_size: 0,
                bitmap_count: 0,
                bitmap_start: 0,
                chunks_start: 0,
            },
            hyperlink_map_start: 16,
            hyperlink_map: crate::hash_map::Layout {
                total_size: 16,
                keys_start: 0,
                vals_start: 0,
                capacity: 0,
            },
            hyperlink_set_start: 16,
            hyperlink_set: crate::ref_counted_set::Layout {
                cap: 0,
                table_cap: 0,
                table_mask: 0,
                table_start: 0,
                items_start: 0,
                total_size: 0,
            },
            capacity: Capacity {
                cols: 0,
                rows: 0,
                styles: 0,
                hyperlink_bytes: 0,
                grapheme_bytes: 0,
                string_bytes: 0,
            },
        },
    ),
    (
        4096,
        Capacity {
            cols: 50,
            rows: 1,
            styles: 0,
            hyperlink_bytes: 144,
            grapheme_bytes: 0,
            string_bytes: 64,
        },
        Layout {
            total_size: 4096,
            rows_start: 0,
            rows_size: 8,
            cells_start: 8,
            cells_size: 400,
            styles_start: 408,
            styles: crate::ref_counted_set::Layout {
                cap: 0,
                table_cap: 0,
                table_mask: 0,
                table_start: 0,
                items_start: 0,
                total_size: 0,
            },
            grapheme_alloc_start: 408,
            grapheme_alloc: crate::bitmap_allocator::Layout {
                total_size: 0,
                bitmap_count: 0,
                bitmap_start: 0,
                chunks_start: 0,
            },
            grapheme_map_start: 408,
            grapheme_map: crate::hash_map::Layout {
                total_size: 16,
                keys_start: 0,
                vals_start: 0,
                capacity: 0,
            },
            string_alloc_start: 424,
            string_alloc: crate::bitmap_allocator::Layout {
                total_size: 2056,
                bitmap_count: 1,
                bitmap_start: 0,
                chunks_start: 8,
            },
            hyperlink_map_start: 2632,
            hyperlink_map: crate::hash_map::Layout {
                total_size: 464,
                keys_start: 64,
                vals_start: 320,
                capacity: 64,
            },
            hyperlink_set_start: 2480,
            hyperlink_set: crate::ref_counted_set::Layout {
                cap: 3,
                table_cap: 4,
                table_mask: 3,
                table_start: 0,
                items_start: 8,
                total_size: 152,
            },
            capacity: Capacity {
                cols: 50,
                rows: 1,
                styles: 0,
                hyperlink_bytes: 144,
                grapheme_bytes: 0,
                string_bytes: 64,
            },
        },
    ),
    (
        4096,
        Capacity {
            cols: 65_535,
            rows: 65_535,
            styles: 65_535,
            hyperlink_bytes: 65_535,
            grapheme_bytes: 4_294_967_295,
            string_bytes: 4_294_967_295,
        },
        Layout {
            total_size: 48_638_988_288,
            rows_start: 0,
            rows_size: 524_280,
            cells_start: 524_280,
            cells_size: 34_358_689_800,
            styles_start: 34_359_214_080,
            styles: crate::ref_counted_set::Layout {
                cap: 53_248,
                table_cap: 65_536,
                table_mask: 65_535,
                table_start: 0,
                items_start: 131_072,
                total_size: 2_048_000,
            },
            grapheme_alloc_start: 34_361_262_080,
            grapheme_alloc: crate::bitmap_allocator::Layout {
                total_size: 4_328_521_728,
                bitmap_count: 4_194_304,
                bitmap_start: 0,
                chunks_start: 33_554_432,
            },
            grapheme_map_start: 38_689_783_808,
            grapheme_map: crate::hash_map::Layout {
                total_size: 5_637_144_592,
                keys_start: 268_435_456,
                vals_start: 1_342_177_280,
                capacity: 268_435_456,
            },
            string_alloc_start: 44_326_928_400,
            string_alloc: crate::bitmap_allocator::Layout {
                total_size: 4_311_744_512,
                bitmap_count: 2_097_152,
                bitmap_start: 0,
                chunks_start: 16_777_216,
            },
            hyperlink_map_start: 48_638_756_880,
            hyperlink_map: crate::hash_map::Layout {
                total_size: 229_392,
                keys_start: 32_768,
                vals_start: 163_840,
                capacity: 32_768,
            },
            hyperlink_set_start: 48_638_672_912,
            hyperlink_set: crate::ref_counted_set::Layout {
                cap: 1664,
                table_cap: 2048,
                table_mask: 2047,
                table_start: 0,
                items_start: 4096,
                total_size: 83_968,
            },
            capacity: Capacity {
                cols: 65_535,
                rows: 65_535,
                styles: 65_535,
                hyperlink_bytes: 65_535,
                grapheme_bytes: 4_294_967_295,
                string_bytes: 4_294_967_295,
            },
        },
    ),
    (
        16_384,
        Capacity {
            cols: 215,
            rows: 215,
            styles: 128,
            hyperlink_bytes: 192,
            grapheme_bytes: 8192,
            string_bytes: 2048,
        },
        Layout {
            total_size: 409_600,
            rows_start: 0,
            rows_size: 1720,
            cells_start: 1720,
            cells_size: 369_800,
            styles_start: 371_520,
            styles: crate::ref_counted_set::Layout {
                cap: 104,
                table_cap: 128,
                table_mask: 127,
                table_start: 0,
                items_start: 256,
                total_size: 4000,
            },
            grapheme_alloc_start: 375_520,
            grapheme_alloc: crate::bitmap_allocator::Layout {
                total_size: 8256,
                bitmap_count: 8,
                bitmap_start: 0,
                chunks_start: 64,
            },
            grapheme_map_start: 383_776,
            grapheme_map: crate::hash_map::Layout {
                total_size: 10_768,
                keys_start: 512,
                vals_start: 2560,
                capacity: 512,
            },
            string_alloc_start: 394_544,
            string_alloc: crate::bitmap_allocator::Layout {
                total_size: 2056,
                bitmap_count: 1,
                bitmap_start: 0,
                chunks_start: 8,
            },
            hyperlink_map_start: 396_752,
            hyperlink_map: crate::hash_map::Layout {
                total_size: 912,
                keys_start: 128,
                vals_start: 640,
                capacity: 128,
            },
            hyperlink_set_start: 396_600,
            hyperlink_set: crate::ref_counted_set::Layout {
                cap: 3,
                table_cap: 4,
                table_mask: 3,
                table_start: 0,
                items_start: 8,
                total_size: 152,
            },
            capacity: Capacity {
                cols: 215,
                rows: 215,
                styles: 128,
                hyperlink_bytes: 192,
                grapheme_bytes: 8192,
                string_bytes: 2048,
            },
        },
    ),
    (
        16_384,
        Capacity {
            cols: 1,
            rows: 1,
            styles: 16,
            hyperlink_bytes: 192,
            grapheme_bytes: 1024,
            string_bytes: 2048,
        },
        Layout {
            total_size: 16_384,
            rows_start: 0,
            rows_size: 8,
            cells_start: 8,
            cells_size: 8,
            styles_start: 16,
            styles: crate::ref_counted_set::Layout {
                cap: 13,
                table_cap: 16,
                table_mask: 15,
                table_start: 0,
                items_start: 32,
                total_size: 500,
            },
            grapheme_alloc_start: 520,
            grapheme_alloc: crate::bitmap_allocator::Layout {
                total_size: 1032,
                bitmap_count: 1,
                bitmap_start: 0,
                chunks_start: 8,
            },
            grapheme_map_start: 1552,
            grapheme_map: crate::hash_map::Layout {
                total_size: 1360,
                keys_start: 64,
                vals_start: 320,
                capacity: 64,
            },
            string_alloc_start: 2912,
            string_alloc: crate::bitmap_allocator::Layout {
                total_size: 2056,
                bitmap_count: 1,
                bitmap_start: 0,
                chunks_start: 8,
            },
            hyperlink_map_start: 5120,
            hyperlink_map: crate::hash_map::Layout {
                total_size: 912,
                keys_start: 128,
                vals_start: 640,
                capacity: 128,
            },
            hyperlink_set_start: 4968,
            hyperlink_set: crate::ref_counted_set::Layout {
                cap: 3,
                table_cap: 4,
                table_mask: 3,
                table_start: 0,
                items_start: 8,
                total_size: 152,
            },
            capacity: Capacity {
                cols: 1,
                rows: 1,
                styles: 16,
                hyperlink_bytes: 192,
                grapheme_bytes: 1024,
                string_bytes: 2048,
            },
        },
    ),
    (
        16_384,
        Capacity {
            cols: 0,
            rows: 0,
            styles: 0,
            hyperlink_bytes: 0,
            grapheme_bytes: 0,
            string_bytes: 0,
        },
        Layout {
            total_size: 16_384,
            rows_start: 0,
            rows_size: 0,
            cells_start: 0,
            cells_size: 0,
            styles_start: 0,
            styles: crate::ref_counted_set::Layout {
                cap: 0,
                table_cap: 0,
                table_mask: 0,
                table_start: 0,
                items_start: 0,
                total_size: 0,
            },
            grapheme_alloc_start: 0,
            grapheme_alloc: crate::bitmap_allocator::Layout {
                total_size: 0,
                bitmap_count: 0,
                bitmap_start: 0,
                chunks_start: 0,
            },
            grapheme_map_start: 0,
            grapheme_map: crate::hash_map::Layout {
                total_size: 16,
                keys_start: 0,
                vals_start: 0,
                capacity: 0,
            },
            string_alloc_start: 16,
            string_alloc: crate::bitmap_allocator::Layout {
                total_size: 0,
                bitmap_count: 0,
                bitmap_start: 0,
                chunks_start: 0,
            },
            hyperlink_map_start: 16,
            hyperlink_map: crate::hash_map::Layout {
                total_size: 16,
                keys_start: 0,
                vals_start: 0,
                capacity: 0,
            },
            hyperlink_set_start: 16,
            hyperlink_set: crate::ref_counted_set::Layout {
                cap: 0,
                table_cap: 0,
                table_mask: 0,
                table_start: 0,
                items_start: 0,
                total_size: 0,
            },
            capacity: Capacity {
                cols: 0,
                rows: 0,
                styles: 0,
                hyperlink_bytes: 0,
                grapheme_bytes: 0,
                string_bytes: 0,
            },
        },
    ),
    (
        16_384,
        Capacity {
            cols: 50,
            rows: 1,
            styles: 0,
            hyperlink_bytes: 144,
            grapheme_bytes: 0,
            string_bytes: 64,
        },
        Layout {
            total_size: 16_384,
            rows_start: 0,
            rows_size: 8,
            cells_start: 8,
            cells_size: 400,
            styles_start: 408,
            styles: crate::ref_counted_set::Layout {
                cap: 0,
                table_cap: 0,
                table_mask: 0,
                table_start: 0,
                items_start: 0,
                total_size: 0,
            },
            grapheme_alloc_start: 408,
            grapheme_alloc: crate::bitmap_allocator::Layout {
                total_size: 0,
                bitmap_count: 0,
                bitmap_start: 0,
                chunks_start: 0,
            },
            grapheme_map_start: 408,
            grapheme_map: crate::hash_map::Layout {
                total_size: 16,
                keys_start: 0,
                vals_start: 0,
                capacity: 0,
            },
            string_alloc_start: 424,
            string_alloc: crate::bitmap_allocator::Layout {
                total_size: 2056,
                bitmap_count: 1,
                bitmap_start: 0,
                chunks_start: 8,
            },
            hyperlink_map_start: 2632,
            hyperlink_map: crate::hash_map::Layout {
                total_size: 464,
                keys_start: 64,
                vals_start: 320,
                capacity: 64,
            },
            hyperlink_set_start: 2480,
            hyperlink_set: crate::ref_counted_set::Layout {
                cap: 3,
                table_cap: 4,
                table_mask: 3,
                table_start: 0,
                items_start: 8,
                total_size: 152,
            },
            capacity: Capacity {
                cols: 50,
                rows: 1,
                styles: 0,
                hyperlink_bytes: 144,
                grapheme_bytes: 0,
                string_bytes: 64,
            },
        },
    ),
    (
        16_384,
        Capacity {
            cols: 65_535,
            rows: 65_535,
            styles: 65_535,
            hyperlink_bytes: 65_535,
            grapheme_bytes: 4_294_967_295,
            string_bytes: 4_294_967_295,
        },
        Layout {
            total_size: 48_639_000_576,
            rows_start: 0,
            rows_size: 524_280,
            cells_start: 524_280,
            cells_size: 34_358_689_800,
            styles_start: 34_359_214_080,
            styles: crate::ref_counted_set::Layout {
                cap: 53_248,
                table_cap: 65_536,
                table_mask: 65_535,
                table_start: 0,
                items_start: 131_072,
                total_size: 2_048_000,
            },
            grapheme_alloc_start: 34_361_262_080,
            grapheme_alloc: crate::bitmap_allocator::Layout {
                total_size: 4_328_521_728,
                bitmap_count: 4_194_304,
                bitmap_start: 0,
                chunks_start: 33_554_432,
            },
            grapheme_map_start: 38_689_783_808,
            grapheme_map: crate::hash_map::Layout {
                total_size: 5_637_144_592,
                keys_start: 268_435_456,
                vals_start: 1_342_177_280,
                capacity: 268_435_456,
            },
            string_alloc_start: 44_326_928_400,
            string_alloc: crate::bitmap_allocator::Layout {
                total_size: 4_311_744_512,
                bitmap_count: 2_097_152,
                bitmap_start: 0,
                chunks_start: 16_777_216,
            },
            hyperlink_map_start: 48_638_756_880,
            hyperlink_map: crate::hash_map::Layout {
                total_size: 229_392,
                keys_start: 32_768,
                vals_start: 163_840,
                capacity: 32_768,
            },
            hyperlink_set_start: 48_638_672_912,
            hyperlink_set: crate::ref_counted_set::Layout {
                cap: 1664,
                table_cap: 2048,
                table_mask: 2047,
                table_start: 0,
                items_start: 4096,
                total_size: 83_968,
            },
            capacity: Capacity {
                cols: 65_535,
                rows: 65_535,
                styles: 65_535,
                hyperlink_bytes: 65_535,
                grapheme_bytes: 4_294_967_295,
                string_bytes: 4_294_967_295,
            },
        },
    ),
];

/// Recorded logical byte total for the native page-size policy.
fn native_total() -> u64 {
    if super::PAGE_SIZE == 16_384 {
        409_600
    } else {
        401_408
    }
}

/// Recorded maximum columns for the native page-size policy.
fn native_maximum() -> u16 {
    if super::PAGE_SIZE == 16_384 {
        47_931
    } else {
        46_907
    }
}

/// Recorded adjustment controls, including zero columns from the grid-byte bound.
fn native_rows(cols: u16) -> u16 {
    let controls = if super::PAGE_SIZE == 16_384 {
        [(0, 47_932), (1, 23_966), (107, 443), (430, 111)]
    } else {
        [(0, 46_908), (1, 23_454), (107, 434), (430, 108)]
    };
    controls
        .into_iter()
        .find(|&(input, _)| input == cols)
        .unwrap()
        .1
}
