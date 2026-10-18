//! Chunk boundaries and fragmented first-fit allocation.

use super::BitmapAllocator;

#[test]
fn bitmap_required_bytes() {
    check_required::<1>();
    check_required::<2>();
    check_required::<4>();
    check_required::<16>();
    check_required::<32>();
}

/// Compare typed byte rounding with the independent recorded results.
fn check_required<const CHUNK: usize>() {
    for &(chunk, elements, byte, codepoint) in REQUIRED_BYTES {
        if chunk != CHUNK {
            continue;
        }
        assert_eq!(
            BitmapAllocator::<CHUNK>::bytes_required::<u8>(elements),
            byte
        );
        assert_eq!(
            BitmapAllocator::<CHUNK>::bytes_required::<char>(elements),
            codepoint
        );
    }
}

#[test]
fn bitmap_find_one() {
    let mut words = [0x8000_0000_0000_0e00];
    assert_eq!(super::find_free_chunks(&mut words, 2), Some(9));
    assert_eq!(words, [0x8000_0000_0000_0800]);
}

#[test]
fn bitmap_find_none() {
    let mut words = [0x8700_0000_0000_0000];
    assert_eq!(super::find_free_chunks(&mut words, 4), None);
    assert_eq!(words, [0x8700_0000_0000_0000]);
}

#[test]
fn bitmap_find_multiple() {
    let mut words = [0x8700_0000_0000_0070, 0x803e_0000_0000_3e00];
    assert_eq!(super::find_free_chunks(&mut words, 4), Some(73));
    assert_eq!(words, [0x8700_0000_0000_0070, 0x803e_0000_0000_2000]);
}

#[test]
fn bitmap_find_full_word() {
    let mut words = [u64::MAX];
    assert_eq!(super::find_free_chunks(&mut words, 64), Some(0));
    assert_eq!(words, [0]);
}

#[test]
fn bitmap_find_multiple_words() {
    let mut words = [u64::MAX; 2];
    assert_eq!(super::find_free_chunks(&mut words, 65), Some(0));
    assert_eq!(words, [0, u64::MAX - 1]);
}

#[test]
fn bitmap_find_offset_multiple_words() {
    let mut words = [0xff00_0000_0000_0000, u64::MAX, u64::MAX];
    assert_eq!(super::find_free_chunks(&mut words, 65), Some(56));
    assert_eq!(words, [0, 0xfe00_0000_0000_0000, u64::MAX]);
}

#[test]
fn bitmap_find_exact_multiple_words() {
    let mut words = [u64::MAX; 3];
    assert_eq!(super::find_free_chunks(&mut words, 128), Some(0));
    assert_eq!(words, [0, 0, u64::MAX]);
}

#[test]
fn bitmap_layout_matches() {
    check_recorded_layouts::<1>();
    check_recorded_layouts::<2>();
    check_recorded_layouts::<4>();
    check_recorded_layouts::<16>();
    check_recorded_layouts::<32>();

    let layout = BitmapAllocator::<4>::layout(256);
    assert_eq!(layout.bitmap_count, 1);
    assert_eq!(layout.bitmap_start, 0);
    assert_eq!(layout.chunks_start, 8);
    assert_eq!(layout.total_size, 264);
    assert_eq!(BitmapAllocator::<4>::layout(0).total_size, 0);
    assert_eq!(BitmapAllocator::<4>::layout(257).total_size, 528);
}

#[test]
fn bitmap_layout_matches_addressable_chunks() {
    check_addressable::<1>();
    check_addressable::<2>();
    check_addressable::<4>();
    check_addressable::<16>();
    check_addressable::<32>();
}

/// Check the independent addressability invariant at all source capacities.
fn check_addressable<const CHUNK: usize>() {
    for bytes in [
        1,
        CHUNK as u64,
        CHUNK as u64 + 1,
        48,
        64,
        512,
        1024,
        2048,
        8192,
        8193,
    ] {
        let layout = BitmapAllocator::<CHUNK>::layout(bytes);
        assert_eq!(
            layout.total_size - layout.chunks_start,
            layout.bitmap_count * 64 * CHUNK as u64
        );
        assert!(layout.total_size - layout.chunks_start >= bytes);
    }
}

#[test]
fn bitmap_small_capacity_stays_in_bounds() {
    let mut allocator = BitmapAllocator::<16>::init(48);
    let mut storage = vec![0; allocator.capacity_bytes()];
    assert_eq!(storage.len(), 1024);
    for index in 0..64 {
        let chunks = allocator.alloc(16).unwrap();
        assert_eq!(chunks, index..index + 1);
        storage[chunks.start * 16..chunks.end * 16].fill(0xaa);
    }
    assert_eq!(
        allocator.alloc(16),
        Err(crate::page::StorageError::OutOfMemory)
    );
    assert_eq!(storage, vec![0xaa; 1024]);
    assert_eq!(allocator.used_bytes(), 1024);
}

#[test]
fn bitmap_allocate_sequentially() {
    let mut allocator = BitmapAllocator::<4>::init(64);
    for index in 0..64 {
        assert_eq!(allocator.alloc(1).unwrap(), index..index + 1);
    }
    assert_eq!(allocator.used_bytes(), 256);
    assert_eq!(
        allocator.alloc(1),
        Err(crate::page::StorageError::OutOfMemory)
    );
}

#[test]
fn bitmap_allocate_codepoints() {
    let mut allocator = BitmapAllocator::<4>::init(128);
    let mut values = vec!['\0'; allocator.capacity_bytes() / 4];
    let first = allocator
        .alloc(BitmapAllocator::<4>::bytes_required::<char>(1))
        .unwrap();
    let second = allocator
        .alloc(BitmapAllocator::<4>::bytes_required::<char>(1))
        .unwrap();
    assert_eq!(first, 0..1);
    assert_eq!(second, 1..2);
    values[first.start] = 'A';
    values[second.start] = 'B';
    assert_eq!(&values[..2], &['A', 'B']);
}

#[test]
fn bitmap_allocate_multichunk_codepoints() {
    let mut allocator = BitmapAllocator::<16>::init(128);
    let mut values = vec!['\0'; allocator.capacity_bytes() / 4];
    let first = allocator
        .alloc(BitmapAllocator::<16>::bytes_required::<char>(6))
        .unwrap();
    let second = allocator
        .alloc(BitmapAllocator::<16>::bytes_required::<char>(1))
        .unwrap();
    assert_eq!(first, 0..2);
    assert_eq!(second, 2..3);
    values[first.start * 4..first.start * 4 + 6].copy_from_slice(&['A', 'B', 'C', 'D', 'E', 'F']);
    values[second.start * 4] = 'Z';
    assert_eq!(&values[..6], &['A', 'B', 'C', 'D', 'E', 'F']);
    assert_eq!(values[8], 'Z');
}

#[test]
fn bitmap_allocate_large_span() {
    let mut allocator = BitmapAllocator::<2>::init(256);
    let span = allocator.alloc(129).unwrap();
    assert_eq!(span, 0..65);
    assert_eq!(allocator.used_bytes(), 130);
    assert_eq!(allocator.bitmap, [0, u64::MAX - 1]);
    let mut values = vec![0; allocator.capacity_bytes()];
    values[span.start * 2..span.start * 2 + 129].fill(0x55);
    assert_eq!(&values[..129], &[0x55; 129]);
    assert_eq!(values[129], 0);
}

#[test]
fn bitmap_free_one_word() {
    let mut allocator = BitmapAllocator::<4>::init(512);
    let first = allocator.alloc(256).unwrap();
    let second = allocator.alloc(256).unwrap();
    assert_eq!(allocator.used_bytes(), 512);
    allocator.free(first);
    assert_eq!(allocator.bitmap, [u64::MAX, 0]);
    assert_eq!(allocator.used_bytes(), 256);
    assert_eq!(allocator.alloc(256).unwrap(), 0..64);
    assert_eq!(second, 64..128);
}

#[test]
fn bitmap_free_half_word() {
    let mut allocator = BitmapAllocator::<1>::init(192);
    let first = allocator.alloc(32).unwrap();
    let second = allocator.alloc(32).unwrap();
    let mut values = [0; 192];
    values[first.clone()].fill(0x11);
    values[second.clone()].fill(0x22);
    allocator.free(first);
    assert_eq!(allocator.bitmap, [0xffff_ffff, u64::MAX, u64::MAX]);
    assert_eq!(&values[second], &[0x22; 32]);
    assert_eq!(allocator.used_bytes(), 32);
    assert_eq!(allocator.alloc(32).unwrap(), 0..32);
}

#[test]
fn bitmap_free_two_halves() {
    let mut allocator = BitmapAllocator::<1>::init(192);
    let first = allocator.alloc(32).unwrap();
    let second = allocator.alloc(32).unwrap();
    assert_eq!(first, 0..32);
    assert_eq!(second, 32..64);
    allocator.free(second);
    assert_eq!(
        allocator.bitmap,
        [0xffff_ffff_0000_0000, u64::MAX, u64::MAX]
    );
    allocator.free(first);
    assert_eq!(allocator.bitmap, [u64::MAX; 3]);
    assert_eq!(allocator.used_bytes(), 0);
    assert_eq!(allocator.alloc(64).unwrap(), 0..64);
}

#[test]
fn bitmap_free_word_and_half() {
    let mut allocator = BitmapAllocator::<1>::init(192);
    let span = allocator.alloc(96).unwrap();
    assert_eq!(span, 0..96);
    assert_eq!(allocator.bitmap, [0, 0xffff_ffff_0000_0000, u64::MAX]);
    allocator.free(span);
    assert_eq!(allocator.bitmap, [u64::MAX; 3]);
    assert_eq!(allocator.used_bytes(), 0);
    assert_eq!(allocator.alloc(96).unwrap(), 0..96);
}

#[test]
fn bitmap_free_two_word_and_half() {
    let mut allocator = BitmapAllocator::<1>::init(192);
    let first = allocator.alloc(96).unwrap();
    let second = allocator.alloc(96).unwrap();
    assert_eq!(first, 0..96);
    assert_eq!(second, 96..192);
    assert_eq!(allocator.bitmap, [0; 3]);
    allocator.free(second);
    assert_eq!(allocator.bitmap, [0, 0xffff_ffff_0000_0000, u64::MAX]);
    assert_eq!(allocator.used_bytes(), 96);
    allocator.free(first);
    assert_eq!(allocator.bitmap, [u64::MAX; 3]);
    assert_eq!(allocator.alloc(192).unwrap(), 0..192);
}

#[test]
fn bitmap_free_offset_word_and_half() {
    let mut allocator = BitmapAllocator::<1>::init(192);
    let first = allocator.alloc(48).unwrap();
    let second = allocator.alloc(96).unwrap();
    assert_eq!(first, 0..48);
    assert_eq!(second, 48..144);
    assert_eq!(allocator.bitmap, [0, 0, 0xffff_ffff_ffff_0000]);
    allocator.free(second);
    assert_eq!(
        allocator.bitmap,
        [0xffff_0000_0000_0000, u64::MAX, u64::MAX]
    );
    assert_eq!(allocator.used_bytes(), 48);
    allocator.free(first);
    assert_eq!(allocator.bitmap, [u64::MAX; 3]);
    assert_eq!(allocator.alloc(192).unwrap(), 0..192);
}

#[test]
fn bitmap_free_three_partial_words() {
    let mut allocator = BitmapAllocator::<1>::init(192);
    let first = allocator.alloc(48).unwrap();
    let second = allocator.alloc(48).unwrap();
    let third = allocator.alloc(48).unwrap();
    assert_eq!(first, 0..48);
    assert_eq!(second, 64..112);
    assert_eq!(third, 128..176);
    allocator.free(second);
    assert_eq!(
        allocator.bitmap,
        [0xffff_0000_0000_0000, u64::MAX, 0xffff_0000_0000_0000]
    );
    allocator.free(first);
    assert_eq!(
        allocator.bitmap,
        [u64::MAX, u64::MAX, 0xffff_0000_0000_0000]
    );
    assert_eq!(allocator.used_bytes(), 48);
    allocator.free(third);
    assert_eq!(allocator.bitmap, [u64::MAX; 3]);
    assert_eq!(allocator.alloc(192).unwrap(), 0..192);
}

#[test]
fn bitmap_free_two_offset_spans() {
    let mut allocator = BitmapAllocator::<1>::init(256);
    let first = allocator.alloc(48).unwrap();
    let second = allocator.alloc(96).unwrap();
    let third = allocator.alloc(96).unwrap();
    assert_eq!(first, 0..48);
    assert_eq!(second, 48..144);
    assert_eq!(third, 144..240);
    allocator.free(second);
    assert_eq!(
        allocator.bitmap,
        [
            0xffff_0000_0000_0000,
            u64::MAX,
            0xffff,
            0xffff_0000_0000_0000
        ]
    );
    allocator.free(first);
    assert_eq!(
        allocator.bitmap,
        [u64::MAX, u64::MAX, 0xffff, 0xffff_0000_0000_0000]
    );
    assert_eq!(allocator.used_bytes(), 96);
    allocator.free(third);
    assert_eq!(allocator.bitmap, [u64::MAX; 4]);
    assert_eq!(allocator.alloc(256).unwrap(), 0..256);
}

/// Chunk size, element count and byte/codepoint rounded requirements.
const REQUIRED_BYTES: &[(usize, usize, usize, usize)] = &[
    (1, 0, 0, 0),
    (1, 1, 1, 4),
    (1, 2, 2, 8),
    (1, 63, 63, 252),
    (1, 64, 64, 256),
    (1, 65, 65, 260),
    (1, 128, 128, 512),
    (2, 0, 0, 0),
    (2, 1, 2, 4),
    (2, 2, 2, 8),
    (2, 3, 4, 12),
    (2, 126, 126, 504),
    (2, 128, 128, 512),
    (2, 129, 130, 516),
    (2, 256, 256, 1_024),
    (4, 0, 0, 0),
    (4, 1, 4, 4),
    (4, 3, 4, 12),
    (4, 4, 4, 16),
    (4, 5, 8, 20),
    (4, 252, 252, 1_008),
    (4, 256, 256, 1_024),
    (4, 257, 260, 1_028),
    (4, 512, 512, 2_048),
    (16, 0, 0, 0),
    (16, 1, 16, 16),
    (16, 15, 16, 64),
    (16, 16, 16, 64),
    (16, 17, 32, 80),
    (16, 1_008, 1_008, 4_032),
    (16, 1_024, 1_024, 4_096),
    (16, 1_025, 1_040, 4_112),
    (16, 2_048, 2_048, 8_192),
    (32, 0, 0, 0),
    (32, 1, 32, 32),
    (32, 31, 32, 128),
    (32, 32, 32, 128),
    (32, 33, 64, 160),
    (32, 2_016, 2_016, 8_064),
    (32, 2_048, 2_048, 8_192),
    (32, 2_049, 2_080, 8_224),
    (32, 4_096, 4_096, 16_384),
];

/// Compare every logical bitmap region with the retained source records.
fn check_recorded_layouts<const CHUNK: usize>() {
    for &(chunk, bytes, expected) in BITMAP_LAYOUTS {
        if chunk == CHUNK {
            assert_eq!(BitmapAllocator::<CHUNK>::layout(bytes), expected);
        }
    }
}

/// Chunk size, requested bytes and complete independent logical layout.
const BITMAP_LAYOUTS: &[(usize, u64, super::Layout)] = &[
    (
        1,
        0,
        super::Layout {
            total_size: 0,
            bitmap_count: 0,
            bitmap_start: 0,
            chunks_start: 0,
        },
    ),
    (
        1,
        1,
        super::Layout {
            total_size: 72,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        1,
        2,
        super::Layout {
            total_size: 72,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        1,
        63,
        super::Layout {
            total_size: 72,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        1,
        64,
        super::Layout {
            total_size: 72,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        1,
        65,
        super::Layout {
            total_size: 144,
            bitmap_count: 2,
            bitmap_start: 0,
            chunks_start: 16,
        },
    ),
    (
        1,
        128,
        super::Layout {
            total_size: 144,
            bitmap_count: 2,
            bitmap_start: 0,
            chunks_start: 16,
        },
    ),
    (
        2,
        0,
        super::Layout {
            total_size: 0,
            bitmap_count: 0,
            bitmap_start: 0,
            chunks_start: 0,
        },
    ),
    (
        2,
        1,
        super::Layout {
            total_size: 136,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        2,
        2,
        super::Layout {
            total_size: 136,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        2,
        3,
        super::Layout {
            total_size: 136,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        2,
        126,
        super::Layout {
            total_size: 136,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        2,
        128,
        super::Layout {
            total_size: 136,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        2,
        129,
        super::Layout {
            total_size: 272,
            bitmap_count: 2,
            bitmap_start: 0,
            chunks_start: 16,
        },
    ),
    (
        2,
        256,
        super::Layout {
            total_size: 272,
            bitmap_count: 2,
            bitmap_start: 0,
            chunks_start: 16,
        },
    ),
    (
        4,
        0,
        super::Layout {
            total_size: 0,
            bitmap_count: 0,
            bitmap_start: 0,
            chunks_start: 0,
        },
    ),
    (
        4,
        1,
        super::Layout {
            total_size: 264,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        4,
        3,
        super::Layout {
            total_size: 264,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        4,
        4,
        super::Layout {
            total_size: 264,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        4,
        5,
        super::Layout {
            total_size: 264,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        4,
        252,
        super::Layout {
            total_size: 264,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        4,
        256,
        super::Layout {
            total_size: 264,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        4,
        257,
        super::Layout {
            total_size: 528,
            bitmap_count: 2,
            bitmap_start: 0,
            chunks_start: 16,
        },
    ),
    (
        4,
        512,
        super::Layout {
            total_size: 528,
            bitmap_count: 2,
            bitmap_start: 0,
            chunks_start: 16,
        },
    ),
    (
        16,
        0,
        super::Layout {
            total_size: 0,
            bitmap_count: 0,
            bitmap_start: 0,
            chunks_start: 0,
        },
    ),
    (
        16,
        1,
        super::Layout {
            total_size: 1_032,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        16,
        15,
        super::Layout {
            total_size: 1_032,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        16,
        16,
        super::Layout {
            total_size: 1_032,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        16,
        17,
        super::Layout {
            total_size: 1_032,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        16,
        1_008,
        super::Layout {
            total_size: 1_032,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        16,
        1_024,
        super::Layout {
            total_size: 1_032,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        16,
        1_025,
        super::Layout {
            total_size: 2_064,
            bitmap_count: 2,
            bitmap_start: 0,
            chunks_start: 16,
        },
    ),
    (
        16,
        2_048,
        super::Layout {
            total_size: 2_064,
            bitmap_count: 2,
            bitmap_start: 0,
            chunks_start: 16,
        },
    ),
    (
        32,
        0,
        super::Layout {
            total_size: 0,
            bitmap_count: 0,
            bitmap_start: 0,
            chunks_start: 0,
        },
    ),
    (
        32,
        1,
        super::Layout {
            total_size: 2_056,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        32,
        31,
        super::Layout {
            total_size: 2_056,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        32,
        32,
        super::Layout {
            total_size: 2_056,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        32,
        33,
        super::Layout {
            total_size: 2_056,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        32,
        2_016,
        super::Layout {
            total_size: 2_056,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        32,
        2_048,
        super::Layout {
            total_size: 2_056,
            bitmap_count: 1,
            bitmap_start: 0,
            chunks_start: 8,
        },
    ),
    (
        32,
        2_049,
        super::Layout {
            total_size: 4_112,
            bitmap_count: 2,
            bitmap_start: 0,
            chunks_start: 16,
        },
    ),
    (
        32,
        4_096,
        super::Layout {
            total_size: 4_112,
            bitmap_count: 2,
            bitmap_start: 0,
            chunks_start: 16,
        },
    ),
];
