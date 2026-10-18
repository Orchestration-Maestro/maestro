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
            keys_start: 0,
            vals_start: 0,
            capacity: 0,
        },
        super::Layout {
            total_size: 16,
            keys_start: 0,
            vals_start: 0,
            capacity: 0,
        },
    ),
    (
        1,
        super::Layout {
            total_size: 40,
            keys_start: 4,
            vals_start: 8,
            capacity: 1,
        },
        super::Layout {
            total_size: 32,
            keys_start: 4,
            vals_start: 12,
            capacity: 2,
        },
    ),
    (
        2,
        super::Layout {
            total_size: 64,
            keys_start: 4,
            vals_start: 16,
            capacity: 2,
        },
        super::Layout {
            total_size: 44,
            keys_start: 4,
            vals_start: 20,
            capacity: 4,
        },
    ),
    (
        3,
        super::Layout {
            total_size: 104,
            keys_start: 4,
            vals_start: 24,
            capacity: 4,
        },
        super::Layout {
            total_size: 44,
            keys_start: 4,
            vals_start: 20,
            capacity: 4,
        },
    ),
    (
        4,
        super::Layout {
            total_size: 104,
            keys_start: 4,
            vals_start: 24,
            capacity: 4,
        },
        super::Layout {
            total_size: 72,
            keys_start: 8,
            vals_start: 40,
            capacity: 8,
        },
    ),
    (
        7,
        super::Layout {
            total_size: 184,
            keys_start: 8,
            vals_start: 40,
            capacity: 8,
        },
        super::Layout {
            total_size: 128,
            keys_start: 16,
            vals_start: 80,
            capacity: 16,
        },
    ),
    (
        8,
        super::Layout {
            total_size: 184,
            keys_start: 8,
            vals_start: 40,
            capacity: 8,
        },
        super::Layout {
            total_size: 128,
            keys_start: 16,
            vals_start: 80,
            capacity: 16,
        },
    ),
    (
        15,
        super::Layout {
            total_size: 352,
            keys_start: 16,
            vals_start: 80,
            capacity: 16,
        },
        super::Layout {
            total_size: 240,
            keys_start: 32,
            vals_start: 160,
            capacity: 32,
        },
    ),
    (
        16,
        super::Layout {
            total_size: 352,
            keys_start: 16,
            vals_start: 80,
            capacity: 16,
        },
        super::Layout {
            total_size: 240,
            keys_start: 32,
            vals_start: 160,
            capacity: 32,
        },
    ),
    (
        17,
        super::Layout {
            total_size: 688,
            keys_start: 32,
            vals_start: 160,
            capacity: 32,
        },
        super::Layout {
            total_size: 240,
            keys_start: 32,
            vals_start: 160,
            capacity: 32,
        },
    ),
    (
        63,
        super::Layout {
            total_size: 1360,
            keys_start: 64,
            vals_start: 320,
            capacity: 64,
        },
        super::Layout {
            total_size: 912,
            keys_start: 128,
            vals_start: 640,
            capacity: 128,
        },
    ),
    (
        64,
        super::Layout {
            total_size: 1360,
            keys_start: 64,
            vals_start: 320,
            capacity: 64,
        },
        super::Layout {
            total_size: 912,
            keys_start: 128,
            vals_start: 640,
            capacity: 128,
        },
    ),
    (
        65,
        super::Layout {
            total_size: 2704,
            keys_start: 128,
            vals_start: 640,
            capacity: 128,
        },
        super::Layout {
            total_size: 912,
            keys_start: 128,
            vals_start: 640,
            capacity: 128,
        },
    ),
    (
        128,
        super::Layout {
            total_size: 2704,
            keys_start: 128,
            vals_start: 640,
            capacity: 128,
        },
        super::Layout {
            total_size: 1808,
            keys_start: 256,
            vals_start: 1280,
            capacity: 256,
        },
    ),
    (
        16_384,
        super::Layout {
            total_size: 344_080,
            keys_start: 16_384,
            vals_start: 81_920,
            capacity: 16_384,
        },
        super::Layout {
            total_size: 229_392,
            keys_start: 32_768,
            vals_start: 163_840,
            capacity: 32_768,
        },
    ),
    (
        65_535,
        super::Layout {
            total_size: 1_376_272,
            keys_start: 65_536,
            vals_start: 327_680,
            capacity: 65_536,
        },
        super::Layout {
            total_size: 917_520,
            keys_start: 131_072,
            vals_start: 655_360,
            capacity: 131_072,
        },
    ),
];
