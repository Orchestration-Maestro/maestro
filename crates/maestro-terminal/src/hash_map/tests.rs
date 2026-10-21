//! Map accounting at all retained load-factor boundaries.

#[test]
fn map_layout_rounding() {
    for &(count, graphemes, hyperlinks) in LAYOUTS {
        assert_eq!(super::layout(count, 4, 16, 8, 100), graphemes);
        assert_eq!(super::layout(count, 4, 2, 2, 80), hyperlinks);
    }
}

/// Independent complete region records from the storage producer.
const LAYOUTS: &[(u64, super::Layout, super::Layout)] = &[
    (
        0,
        super::Layout {
            total_size: 16,
            capacity: 0,
        },
        super::Layout {
            total_size: 16,
            capacity: 0,
        },
    ),
    (
        1,
        super::Layout {
            total_size: 40,
            capacity: 1,
        },
        super::Layout {
            total_size: 32,
            capacity: 2,
        },
    ),
    (
        2,
        super::Layout {
            total_size: 64,
            capacity: 2,
        },
        super::Layout {
            total_size: 44,
            capacity: 4,
        },
    ),
    (
        3,
        super::Layout {
            total_size: 104,
            capacity: 4,
        },
        super::Layout {
            total_size: 44,
            capacity: 4,
        },
    ),
    (
        4,
        super::Layout {
            total_size: 104,
            capacity: 4,
        },
        super::Layout {
            total_size: 72,
            capacity: 8,
        },
    ),
    (
        7,
        super::Layout {
            total_size: 184,
            capacity: 8,
        },
        super::Layout {
            total_size: 128,
            capacity: 16,
        },
    ),
    (
        8,
        super::Layout {
            total_size: 184,
            capacity: 8,
        },
        super::Layout {
            total_size: 128,
            capacity: 16,
        },
    ),
    (
        15,
        super::Layout {
            total_size: 352,
            capacity: 16,
        },
        super::Layout {
            total_size: 240,
            capacity: 32,
        },
    ),
    (
        16,
        super::Layout {
            total_size: 352,
            capacity: 16,
        },
        super::Layout {
            total_size: 240,
            capacity: 32,
        },
    ),
    (
        17,
        super::Layout {
            total_size: 688,
            capacity: 32,
        },
        super::Layout {
            total_size: 240,
            capacity: 32,
        },
    ),
    (
        63,
        super::Layout {
            total_size: 1360,
            capacity: 64,
        },
        super::Layout {
            total_size: 912,
            capacity: 128,
        },
    ),
    (
        64,
        super::Layout {
            total_size: 1360,
            capacity: 64,
        },
        super::Layout {
            total_size: 912,
            capacity: 128,
        },
    ),
    (
        65,
        super::Layout {
            total_size: 2704,
            capacity: 128,
        },
        super::Layout {
            total_size: 912,
            capacity: 128,
        },
    ),
    (
        128,
        super::Layout {
            total_size: 2704,
            capacity: 128,
        },
        super::Layout {
            total_size: 1808,
            capacity: 256,
        },
    ),
    (
        16_384,
        super::Layout {
            total_size: 344_080,
            capacity: 16_384,
        },
        super::Layout {
            total_size: 229_392,
            capacity: 32_768,
        },
    ),
    (
        65_535,
        super::Layout {
            total_size: 1_376_272,
            capacity: 65_536,
        },
        super::Layout {
            total_size: 917_520,
            capacity: 131_072,
        },
    ),
];
