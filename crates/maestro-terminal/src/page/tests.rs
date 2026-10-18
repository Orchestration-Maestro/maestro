//! Managed table ownership and page accounting.

use super::{CellPosition, Page, layout::Capacity};

#[test]
fn append_chunks_track_suffix_ownership() {
    let mut page = Page::init(Capacity {
        cols: 1,
        rows: 1,
        ..Capacity::default()
    });
    let position = CellPosition { x: 0, y: 0 };
    page.get_cells_mut(0)[0] = crate::Cell::init('A');
    for _ in 0..4 {
        page.append_grapheme(position, 'B').unwrap();
    }
    assert_eq!(page.grapheme_alloc.used_bytes(), 16);
    page.append_grapheme(position, 'C').unwrap();
    assert_eq!(page.grapheme_alloc.used_bytes(), 32);
    page.clear_grapheme(position);
    assert_eq!(page.grapheme_alloc.used_bytes(), 0);
}

#[test]
fn page_clone_owns_graphemes() {
    let mut source = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    let position = CellPosition { x: 0, y: 0 };
    source.get_cells_mut(0)[0] = crate::Cell::init('\t');
    source.append_grapheme(position, '\n').unwrap();
    source.append_grapheme(position, '\u{b}').unwrap();
    let cloned = source.clone();
    source.clear_grapheme(position);
    source.update_row_grapheme_flag(0);
    assert_eq!(cloned.lookup_grapheme(position), Some(&['\n', '\u{b}'][..]));
    assert!(cloned.get_cells(0)[0].has_grapheme());
    assert!(cloned.get_row(0).grapheme());
    assert_eq!(cloned.grapheme_count(), 1);
    assert_eq!(source.grapheme_count(), 0);
}

#[test]
fn page_clone_keeps_style_values() {
    let mut source = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    let mut style = crate::Style::default();
    style.flags.set_bold(true);
    let id = source.styles.add(style).unwrap();
    source.get_cells_mut(0)[0] = crate::Cell::init('A');
    source.get_cells_mut(0)[0].set_style_id(id);
    source.get_row_mut(0).set_styled(true);
    let cloned = source.clone();
    source.get_cells_mut(0)[0].set_style_id(0);
    assert!(source.styles.release_multiple(id, 1).is_some());
    assert!(
        cloned
            .styles
            .get(cloned.get_cells(0)[0].style_id())
            .flags
            .bold()
    );
    assert_eq!(cloned.styles.count(), 1);
    assert_eq!(source.styles.count(), 0);
}

#[test]
fn page_copy_graphemes() {
    let mut source = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    for y in 0..10 {
        source.get_cells_mut(y)[1] = crate::Cell::init('A');
        source
            .append_grapheme(CellPosition { x: 1, y }, '\n')
            .unwrap();
    }
    let mut destination = Page::init(source.capacity);
    destination.clone_from(&source, 0..10).unwrap();
    for y in 0..10 {
        source.clear_grapheme(CellPosition { x: 1, y });
    }
    assert_eq!(source.grapheme_count(), 0);
    assert_eq!(destination.grapheme_count(), 10);
    for y in 0..10 {
        assert_eq!(destination.get_cells(y)[1].codepoint(), 'A');
        assert!(destination.get_cells(y)[1].has_grapheme());
        assert!(destination.get_row(y).grapheme());
        assert_eq!(
            destination.lookup_grapheme(CellPosition { x: 1, y }),
            Some(&['\n'][..])
        );
    }
}

#[test]
fn page_copy_releases_old_graphemes() {
    let source = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    let mut destination = Page::init(source.capacity);
    for y in 0..10 {
        destination.get_cells_mut(y)[1] = crate::Cell::init('A');
        destination
            .append_grapheme(CellPosition { x: 1, y }, 'B')
            .unwrap();
    }
    destination.clone_from(&source, 0..10).unwrap();
    assert_eq!(destination.grapheme_count(), 0);
    for y in 0..10 {
        assert_eq!(destination.get_cells(y), &[crate::Cell::default(); 10]);
        assert!(!destination.get_row(y).grapheme());
    }
    assert_eq!(destination.grapheme_alloc.used_bytes(), 0);
}

#[test]
fn page_move_graphemes() {
    let mut page = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    for x in 0..10 {
        page.get_cells_mut(0)[usize::from(x)] =
            crate::Cell::init(char::from_u32(u32::from(x) + 1).unwrap());
        page.append_grapheme(CellPosition { x, y: 0 }, '\n')
            .unwrap();
    }
    page.move_cells(CellPosition { x: 0, y: 0 }, CellPosition { x: 0, y: 1 }, 10);
    assert_eq!(page.grapheme_count(), 10);
    assert_eq!(page.get_cells(0), &[crate::Cell::default(); 10]);
    assert!(!page.get_row(0).grapheme());
    assert!(page.get_row(1).grapheme());
    for x in 0..10 {
        assert_eq!(
            page.get_cells(1)[usize::from(x)].codepoint() as u32,
            u32::from(x) + 1
        );
        assert_eq!(
            page.lookup_grapheme(CellPosition { x, y: 1 }),
            Some(&['\n'][..])
        );
    }
}

#[test]
fn page_copy_links_at_capacity() {
    use crate::hyperlink::{Hyperlink, HyperlinkId};
    let mut source = Page::init(Capacity {
        cols: 120,
        rows: 1,
        ..Capacity::default()
    });
    let count = u16::try_from(source.hyperlink_capacity()).unwrap();
    for x in 0..count {
        source.get_cells_mut(0)[usize::from(x)] = crate::Cell::init('A');
        let id = source
            .insert_hyperlink(Hyperlink {
                id: HyperlinkId::Implicit(1),
                uri: b"https://example.test".to_vec(),
            })
            .unwrap();
        source.set_hyperlink(CellPosition { x, y: 0 }, id).unwrap();
    }
    let mut destination = Page::init(source.capacity);
    destination.clone_from(&source, 0..1).unwrap();
    assert_eq!(source.hyperlink_count(), usize::from(count));
    assert_eq!(destination.hyperlink_count(), usize::from(count));
    for x in 0..count {
        let id = destination
            .lookup_hyperlink(CellPosition { x, y: 0 })
            .unwrap();
        assert_eq!(
            destination.hyperlink_set.get(id).uri,
            b"https://example.test"
        );
        assert!(destination.get_cells(0)[usize::from(x)].hyperlink());
    }
}

#[test]
fn row_copy_same_page_links() {
    use crate::hyperlink::{Hyperlink, HyperlinkId};
    let mut page = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    page.get_cells_mut(0)[7] = crate::Cell::init('B');
    let id = page
        .insert_hyperlink(Hyperlink {
            id: HyperlinkId::Implicit(1),
            uri: b"https://example.test".to_vec(),
        })
        .unwrap();
    page.set_hyperlink(CellPosition { x: 7, y: 0 }, id).unwrap();
    page.clone_partial_row_within(1, 0, 2..8).unwrap();
    assert_eq!(page.hyperlink_count(), 2);
    assert_eq!(page.get_cells(1)[7].codepoint(), 'B');
    assert!(page.get_cells(1)[7].hyperlink());
    assert!(page.get_row(1).hyperlink());
    assert_eq!(page.lookup_hyperlink(CellPosition { x: 7, y: 1 }), Some(id));
}

#[test]
fn row_copy_omits_unselected_same_page_links() {
    use crate::hyperlink::{Hyperlink, HyperlinkId};
    let mut page = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    page.get_cells_mut(0)[7] = crate::Cell::init('B');
    let id = page
        .insert_hyperlink(Hyperlink {
            id: HyperlinkId::Implicit(1),
            uri: b"https://example.test".to_vec(),
        })
        .unwrap();
    page.set_hyperlink(CellPosition { x: 7, y: 0 }, id).unwrap();
    page.clone_partial_row_within(1, 0, 2..6).unwrap();
    assert_eq!(page.hyperlink_count(), 1);
    assert!(!page.get_row(1).hyperlink());
    assert!(page.get_cells(1).iter().all(|cell| !cell.hyperlink()));
}

#[test]
fn exact_capacity_empty_rows() {
    let page = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    assert_eq!(
        page.exact_row_capacity(0..5),
        Capacity {
            cols: 10,
            rows: 5,
            styles: 0,
            hyperlink_bytes: 0,
            grapheme_bytes: 0,
            string_bytes: 0
        }
    );
}

/// Add one cell-owned style reference and maintain its row marker.
fn write_style(page: &mut Page, position: CellPosition, style: crate::Style) {
    let id = page.styles.add(style).unwrap();
    page.get_cells_mut(position.y)[usize::from(position.x)].set_style_id(id);
    page.get_row_mut(position.y).set_styled(true);
}

#[test]
fn exact_capacity_unique_styles() {
    let mut page = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    let style = |palette| crate::Style {
        fg_color: crate::style::Color::Palette(palette),
        ..crate::Style::default()
    };
    assert_eq!(page.exact_row_capacity(0..5).styles, 0);
    write_style(&mut page, CellPosition { x: 0, y: 0 }, style(1));
    assert_eq!(page.exact_row_capacity(0..5).styles, 3);
    write_style(&mut page, CellPosition { x: 1, y: 0 }, style(1));
    assert_eq!(page.exact_row_capacity(0..5).styles, 3);
    write_style(&mut page, CellPosition { x: 2, y: 0 }, style(2));
    assert_eq!(page.exact_row_capacity(0..5).styles, 4);
    write_style(&mut page, CellPosition { x: 0, y: 7 }, style(3));
    let exact = page.exact_row_capacity(0..5);
    assert_eq!(exact.styles, 4);
    assert_eq!(page.exact_row_capacity(0..10).styles, 5);
    let mut copied = Page::init(exact);
    copied.clone_from(&page, 0..5).unwrap();
    assert_eq!(copied.exact_row_capacity(0..5), exact);
    assert_eq!(
        copied.styles.get(copied.get_cells(0)[2].style_id()),
        &style(2)
    );
}

#[test]
fn exact_capacity_repeated_style_clones() {
    let mut page = Page::init(Capacity {
        cols: 10,
        rows: 1,
        ..Capacity::default()
    });
    let mut style = crate::Style::default();
    style.flags.set_bold(true);
    for x in 0..10 {
        write_style(&mut page, CellPosition { x, y: 0 }, style);
    }
    let capacity = page.exact_row_capacity(0..1);
    assert_eq!(capacity.styles, 3);
    let mut copied = Page::init(capacity);
    copied.clone_from(&page, 0..1).unwrap();
    assert_eq!(copied.styles.count(), 1);
    assert_eq!(
        copied.styles.ref_count(copied.get_cells(0)[0].style_id()),
        10
    );
    assert_eq!(copied.exact_row_capacity(0..1), capacity);
}

#[test]
fn exact_capacity_many_styles_one_row() {
    let mut page = Page::init(Capacity {
        cols: u16::MAX,
        rows: 1,
        styles: u16::MAX,
        ..Capacity::default()
    });
    for x in 0..1000_u16 {
        let [r, g] = x.to_le_bytes();
        write_style(
            &mut page,
            CellPosition { x, y: 0 },
            crate::Style {
                fg_color: crate::style::Color::Rgb(crate::color::Rgb { r, g, b: 0 }),
                ..crate::Style::default()
            },
        );
    }
    assert_eq!(page.styles.count(), 1000);
    assert_eq!(page.exact_row_capacity(0..1).styles, 1232);
}

#[test]
fn exact_capacity_grapheme_bytes() {
    let mut page = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    assert_eq!(page.exact_row_capacity(0..5).grapheme_bytes, 0);
    for (index, (x, y)) in [(0, 0), (1, 0), (0, 1), (0, 7)].into_iter().enumerate() {
        page.get_cells_mut(y)[usize::from(x)] = crate::Cell::init('A');
        page.append_grapheme(CellPosition { x, y }, 'B').unwrap();
        assert_eq!(
            page.exact_row_capacity(0..5).grapheme_bytes,
            [16, 32, 48, 48][index]
        );
    }
    assert_eq!(page.exact_row_capacity(0..10).grapheme_bytes, 64);
    let exact = page.exact_row_capacity(0..5);
    let mut copied = Page::init(exact);
    copied.clone_from(&page, 0..5).unwrap();
    assert_eq!(copied.exact_row_capacity(0..5), exact);
}

#[test]
fn exact_capacity_multi_chunk_graphemes() {
    let mut page = Page::init(Capacity {
        cols: 10,
        rows: 1,
        ..Capacity::default()
    });
    page.get_cells_mut(0)[0] = crate::Cell::init('A');
    page.set_graphemes(CellPosition { x: 0, y: 0 }, &['B', 'C', 'D', 'E', 'F', 'G'])
        .unwrap();
    let exact = page.exact_row_capacity(0..1);
    assert_eq!(exact.grapheme_bytes, 32);
    let mut copied = Page::init(exact);
    copied.clone_from(&page, 0..1).unwrap();
    assert_eq!(copied.exact_row_capacity(0..1), exact);
    assert_eq!(
        copied.lookup_grapheme(CellPosition { x: 0, y: 0 }),
        Some(&['B', 'C', 'D', 'E', 'F', 'G'][..])
    );
}

/// Acquire a link reference and transfer it to one cell.
fn write_link(page: &mut Page, position: CellPosition, link: crate::hyperlink::Hyperlink) {
    let id = page.insert_hyperlink(link).unwrap();
    page.set_hyperlink(position, id).unwrap();
}

#[test]
fn exact_capacity_link_strings() {
    use crate::hyperlink::{Hyperlink, HyperlinkId};
    let mut page = Page::init(Capacity {
        cols: 10,
        rows: 10,
        hyperlink_bytes: 384,
        ..Capacity::default()
    });
    let first = Hyperlink {
        id: HyperlinkId::Implicit(1),
        uri: b"a".to_vec(),
    };
    write_link(&mut page, CellPosition { x: 0, y: 0 }, first.clone());
    assert_eq!(page.exact_row_capacity(0..5).string_bytes, 32);
    write_link(&mut page, CellPosition { x: 1, y: 0 }, first);
    assert_eq!(page.exact_row_capacity(0..5).string_bytes, 32);
    write_link(
        &mut page,
        CellPosition { x: 2, y: 0 },
        Hyperlink {
            id: HyperlinkId::Explicit(b"id".to_vec()),
            uri: b"b".to_vec(),
        },
    );
    assert_eq!(page.exact_row_capacity(0..5).string_bytes, 96);
    write_link(
        &mut page,
        CellPosition { x: 0, y: 7 },
        Hyperlink {
            id: HyperlinkId::Implicit(2),
            uri: b"c".to_vec(),
        },
    );
    let exact = page.exact_row_capacity(0..5);
    assert_eq!(exact.string_bytes, 96);
    assert_eq!(page.exact_row_capacity(0..10).string_bytes, 128);
    let mut copied = Page::init(exact);
    copied.clone_from(&page, 0..5).unwrap();
    assert_eq!(copied.exact_row_capacity(0..5), exact);
}

#[test]
fn exact_capacity_repeated_link_clones() {
    use crate::hyperlink::{Hyperlink, HyperlinkId};
    let mut page = Page::init(Capacity {
        cols: 10,
        rows: 1,
        ..Capacity::default()
    });
    for x in 0..2 {
        write_link(
            &mut page,
            CellPosition { x, y: 0 },
            Hyperlink {
                id: HyperlinkId::Implicit(1),
                uri: b"a".to_vec(),
            },
        );
    }
    let exact = page.exact_row_capacity(0..1);
    assert_eq!(exact.hyperlink_bytes, 144);
    let mut copied = Page::init(exact);
    copied.clone_from(&page, 0..1).unwrap();
    assert_eq!(copied.hyperlink_count(), 2);
    assert_eq!(copied.hyperlink_set.count(), 1);
    assert_eq!(copied.exact_row_capacity(0..1), exact);
}

#[test]
fn exact_capacity_many_link_cells() {
    use crate::hyperlink::{Hyperlink, HyperlinkId};
    let mut page = Page::init(Capacity {
        cols: 50,
        rows: 1,
        ..Capacity::default()
    });
    for x in 0..50 {
        write_link(
            &mut page,
            CellPosition { x, y: 0 },
            Hyperlink {
                id: HyperlinkId::Implicit(1),
                uri: b"a".to_vec(),
            },
        );
    }
    let exact = page.exact_row_capacity(0..1);
    assert_eq!(exact.hyperlink_bytes, 192);
    assert_eq!(exact.string_bytes, 32);
    let mut copied = Page::init(exact);
    copied.clone_from(&page, 0..1).unwrap();
    assert_eq!(copied.hyperlink_count(), 50);
    assert!(copied.get_cells(0).iter().all(|cell| cell.hyperlink()));
    assert_eq!(copied.exact_row_capacity(0..1), exact);
}

#[test]
fn hyperlink_identity_lifetime_and_clone() {
    hyperlink_identity_pairs();
    hyperlink_last_owner();
    hyperlink_rollback();
}

/// Compare every retained URI and explicit/implicit identity combination.
fn hyperlink_identity_pairs() {
    use crate::hyperlink::{Hyperlink, HyperlinkId};
    let links = [
        Hyperlink {
            id: HyperlinkId::Explicit(b"id".to_vec()),
            uri: b"https://example.com".to_vec(),
        },
        Hyperlink {
            id: HyperlinkId::Explicit(b"other".to_vec()),
            uri: b"https://example.com".to_vec(),
        },
        Hyperlink {
            id: HyperlinkId::Implicit(1),
            uri: b"https://example.com".to_vec(),
        },
        Hyperlink {
            id: HyperlinkId::Implicit(2),
            uri: b"https://example.com".to_vec(),
        },
        Hyperlink {
            id: HyperlinkId::Explicit(b"id".to_vec()),
            uri: b"https://other.example".to_vec(),
        },
    ];
    let equal = [
        [true, false, false, false, false],
        [false, true, false, false, false],
        [false, false, true, false, false],
        [false, false, false, true, false],
        [false, false, false, false, true],
    ];
    for (left, first) in links.iter().enumerate() {
        for (right, second) in links.iter().enumerate() {
            let mut page = Page::init(Capacity {
                cols: 10,
                rows: 1,
                ..Capacity::default()
            });
            let a = page.insert_hyperlink(first.clone()).unwrap();
            let b = page.insert_hyperlink(second.clone()).unwrap();
            assert_eq!(a == b, equal[left][right]);
            assert_eq!(
                page.hyperlink_set.ref_count(b),
                1 + u16::from(equal[left][right])
            );
        }
    }
}

/// Keep byte identities through cross-page copy and last-owner release.
fn hyperlink_last_owner() {
    use crate::hyperlink::{Hyperlink, HyperlinkId};
    let mut source = Page::init(Capacity {
        cols: 10,
        rows: 1,
        ..Capacity::default()
    });
    let link = Hyperlink {
        id: HyperlinkId::Explicit(vec![0xff, b'a']),
        uri: vec![0xfe, b'u'],
    };
    let id = source.insert_hyperlink(link.clone()).unwrap();
    source
        .set_hyperlink(CellPosition { x: 0, y: 0 }, id)
        .unwrap();
    let retained = source.insert_hyperlink(link.clone()).unwrap();
    let mut copied = Page::init(source.capacity);
    copied.clone_from(&source, 0..1).unwrap();
    source.clear_hyperlink(CellPosition { x: 0, y: 0 });
    assert_eq!(source.string_alloc.used_bytes(), 64);
    source.release_hyperlink(retained);
    assert_eq!(source.string_alloc.used_bytes(), 0);
    let copied_id = copied
        .lookup_hyperlink(CellPosition { x: 0, y: 0 })
        .unwrap();
    drop(source);
    assert_eq!(copied.hyperlink_set.get(copied_id), &link);
    copied.clear_hyperlink(CellPosition { x: 0, y: 0 });
    assert_eq!(copied.string_alloc.used_bytes(), 0);
}

/// Roll back partial string insertion and preserve a retained ID on map refusal.
fn hyperlink_rollback() {
    use crate::hyperlink::{Hyperlink, HyperlinkId};
    let mut failing = Page::init(Capacity {
        cols: 120,
        rows: 1,
        string_bytes: 1,
        ..Capacity::default()
    });
    assert_eq!(
        failing.insert_hyperlink(Hyperlink {
            id: HyperlinkId::Explicit(vec![b'i']),
            uri: vec![b'u'; 2048]
        }),
        Err(super::StorageError::StringsOutOfMemory)
    );
    assert_eq!(failing.string_alloc.used_bytes(), 0);
    assert_eq!(failing.hyperlink_set.count(), 0);
    for x in 0..u16::try_from(failing.hyperlink_capacity()).unwrap() {
        write_link(
            &mut failing,
            CellPosition { x, y: 0 },
            Hyperlink {
                id: HyperlinkId::Implicit(1),
                uri: vec![b'u'],
            },
        );
    }
    let incoming = failing
        .insert_hyperlink(Hyperlink {
            id: HyperlinkId::Implicit(2),
            uri: vec![b'v'],
        })
        .unwrap();
    let x = u16::try_from(failing.hyperlink_capacity()).unwrap();
    assert_eq!(
        failing.set_hyperlink(CellPosition { x, y: 0 }, incoming),
        Err(super::StorageError::HyperlinkMapOutOfMemory)
    );
    assert!(!failing.get_cells(0)[usize::from(x)].hyperlink());
    assert_eq!(failing.hyperlink_set.ref_count(incoming), 1);
    failing.release_hyperlink(incoming);
    assert_eq!(failing.string_alloc.used_bytes(), 32);
}

#[test]
fn clearing_empty_row_removes_placeholder_flag() {
    let mut page = Page::init(Capacity {
        cols: 2,
        rows: 1,
        ..Capacity::default()
    });
    page.get_row_mut(0).set_kitty_virtual_placeholder(true);
    page.clear_cells(0, 0..2);
    assert!(!page.get_row(0).kitty_virtual_placeholder());
}

#[test]
fn storage_errors_keep_categories() {
    use super::StorageError as E;
    let position = CellPosition { x: 0, y: 0 };
    let mut no_clusters = Page::init(Capacity {
        cols: 2,
        rows: 1,
        grapheme_bytes: 0,
        ..Capacity::default()
    });
    no_clusters.get_cells_mut(0)[0] = crate::Cell::init('A');
    assert_eq!(
        no_clusters.set_graphemes(position, &['B']),
        Err(E::GraphemeAllocOutOfMemory)
    );
    assert_eq!(
        no_clusters.append_grapheme(position, 'B'),
        Err(E::OutOfMemory)
    );
    assert!(!no_clusters.get_cells(0)[0].has_grapheme());
    let mut small = Page::init(Capacity {
        grapheme_bytes: 1,
        ..no_clusters.capacity
    });
    small.get_cells_mut(0).fill(crate::Cell::init('A'));
    small.set_graphemes(position, &['B']).unwrap();
    assert_eq!(
        small.set_graphemes(CellPosition { x: 1, y: 0 }, &['C']),
        Err(E::GraphemeMapOutOfMemory)
    );
    assert_eq!(small.grapheme_alloc.used_bytes(), 16);
    assert!(!small.get_cells(0)[1].has_grapheme());
    link_error_controls();
    style_copy_error_control();
    for (error, name) in [
        (E::OutOfMemory, "OutOfMemory"),
        (E::StyleSetOutOfMemory, "StyleSetOutOfMemory"),
        (E::HyperlinkSetOutOfMemory, "HyperlinkSetOutOfMemory"),
        (E::HyperlinkMapOutOfMemory, "HyperlinkMapOutOfMemory"),
        (E::StringAllocOutOfMemory, "StringAllocOutOfMemory"),
        (E::StringsOutOfMemory, "StringsOutOfMemory"),
        (E::SetOutOfMemory, "SetOutOfMemory"),
        (E::GraphemeMapOutOfMemory, "GraphemeMapOutOfMemory"),
        (E::GraphemeAllocOutOfMemory, "GraphemeAllocOutOfMemory"),
    ] {
        assert_eq!(error.to_string(), name);
    }
    assert_eq!(super::CapacityError.to_string(), "OutOfMemory");
}

/// Preserve managed-row metadata when style admission fails mid-copy.
fn style_copy_error_control() {
    use super::StorageError as E;
    let position = CellPosition { x: 0, y: 0 };
    let mut source = Page::init(Capacity {
        cols: 2,
        rows: 1,
        ..Capacity::default()
    });
    source.get_cells_mut(0)[0] = crate::Cell::init('A');
    write_style(
        &mut source,
        position,
        crate::Style {
            fg_color: crate::style::Color::Palette(1),
            ..Default::default()
        },
    );
    let mut destination = Page::init(Capacity {
        styles: 0,
        ..source.capacity
    });
    assert_eq!(
        destination.clone_partial_row_from(&source, 0, 0, 0..1),
        Err(E::StyleSetOutOfMemory)
    );
    assert_eq!(destination.styles.count(), 0);
    assert_eq!(destination.get_cells(0)[0].style_id(), 0);
    assert!(destination.get_row(0).styled());
}

/// Reach insertion and row-copy link errors at their actual table owners.
fn link_error_controls() {
    use super::StorageError as E;
    use crate::hyperlink::{Hyperlink, HyperlinkId};
    let position = CellPosition { x: 0, y: 0 };
    let link = || Hyperlink {
        id: HyperlinkId::Implicit(3),
        uri: vec![b'w'],
    };
    let capacity = Capacity {
        cols: 2,
        rows: 1,
        ..Capacity::default()
    };
    let mut no_strings = Page::init(Capacity {
        string_bytes: 0,
        ..capacity
    });
    assert_eq!(
        no_strings.insert_hyperlink(link()),
        Err(E::StringsOutOfMemory)
    );
    assert_eq!(no_strings.string_alloc.used_bytes(), 0);
    let mut no_set = Page::init(Capacity {
        hyperlink_bytes: 0,
        ..capacity
    });
    assert_eq!(no_set.insert_hyperlink(link()), Err(E::SetOutOfMemory));
    assert_eq!(no_set.string_alloc.used_bytes(), 0);
    let mut source = Page::init(capacity);
    write_link(&mut source, position, link());
    assert_eq!(
        no_strings.clone_from(&source, 0..1),
        Err(E::StringAllocOutOfMemory)
    );
    assert_eq!(no_strings.hyperlink_count(), 0);
    let mut set_without_slots = Page::init(Capacity {
        hyperlink_bytes: 96,
        ..capacity
    });
    assert_eq!(
        set_without_slots.clone_from(&source, 0..1),
        Err(E::HyperlinkSetOutOfMemory)
    );
    assert_eq!(set_without_slots.string_alloc.used_bytes(), 0);
    assert_eq!(set_without_slots.hyperlink_count(), 0);
    assert_eq!(
        no_set.clone_from(&source, 0..1),
        Err(E::HyperlinkMapOutOfMemory)
    );
    assert_eq!(no_set.hyperlink_count(), 0);
    assert!(!no_set.get_cells(0)[0].hyperlink());
}

#[test]
fn owned_style_refs_survive_swaps_and_reset() {
    let mut page = Page::init(Capacity {
        cols: 2,
        rows: 1,
        ..Capacity::default()
    });
    let mut bold = crate::Style::default();
    bold.flags.set_bold(true);
    write_style(&mut page, CellPosition { x: 0, y: 0 }, bold);
    let id = page.get_cells(0)[0].style_id();
    page.swap_cells(CellPosition { x: 0, y: 0 }, CellPosition { x: 1, y: 0 });
    assert_eq!(page.get_cells(0)[1].style_id(), id);
    assert_eq!(page.styles.count(), 1);
    assert_eq!(page.styles.ref_count(id), 1);
    let mut italic = crate::Style::default();
    italic.flags.set_italic(true);
    write_style(&mut page, CellPosition { x: 0, y: 0 }, italic);
    page.clear_cells(0, 0..1);
    assert_eq!(page.styles.lookup(&italic), None);
    assert_eq!(page.styles.count(), 1);
    assert_eq!(page.styles.ref_count(id), 1);
    assert!(page.get_row(0).styled());
    page.clear_cells(0, 0..2);
    assert_eq!(page.styles.count(), 0);
    assert!(!page.get_row(0).styled());
    write_style(&mut page, CellPosition { x: 0, y: 0 }, bold);
    page.reinit();
    assert_eq!(page.styles.count(), 0);
    assert_eq!(page.get_cells(0)[1].style_id(), 0);
}

#[test]
fn row_copy_respects_cleared_cell_link_marker() {
    copy_cleared_link_marker_rows(1);
    copy_cleared_link_marker_rows(2);
}

/// Copy a cleared cell marker through foreign and local row routes.
fn copy_cleared_link_marker_rows(rows: u16) {
    use crate::hyperlink::{Hyperlink, HyperlinkId};
    let mut source = Page::init(Capacity {
        cols: 1,
        rows,
        ..Capacity::default()
    });
    write_link(
        &mut source,
        CellPosition { x: 0, y: 0 },
        Hyperlink {
            id: HyperlinkId::Implicit(4),
            uri: vec![b'x'],
        },
    );
    source.get_cells_mut(0)[0].set_hyperlink(false);
    assert!(
        source
            .lookup_hyperlink(CellPosition { x: 0, y: 0 })
            .is_some()
    );
    assert!(!source.get_cells(0)[0].hyperlink());
    let mut destination = Page::init(source.capacity);
    destination.clone_from(&source, 0..1).unwrap();
    assert_eq!(destination.hyperlink_count(), 0);
    assert!(!destination.get_cells(0)[0].hyperlink());
    if rows > 1 {
        source.clone_partial_row_within(1, 0, 0..1).unwrap();
        assert_eq!(source.hyperlink_count(), 1);
        assert!(!source.get_cells(1)[0].hyperlink());
    }
}

#[test]
fn same_page_copy_refusal_preserves_link_owners() {
    use crate::hyperlink::{Hyperlink, HyperlinkId};
    for full in [true, false] {
        let mut page = Page::init(Capacity {
            cols: 120,
            rows: 2,
            ..Capacity::default()
        });
        let amount = page.hyperlink_capacity() - usize::from(!full);
        for x in 0..u16::try_from(amount).unwrap() {
            page.get_cells_mut(0)[usize::from(x)] = crate::Cell::init('A');
            write_link(
                &mut page,
                CellPosition { x, y: 0 },
                Hyperlink {
                    id: HyperlinkId::Implicit(1),
                    uri: vec![b'r'],
                },
            );
        }
        let id = page.lookup_hyperlink(CellPosition { x: 0, y: 0 }).unwrap();
        let before = page.hyperlink_set.ref_count(id);
        let copied = page.clone_partial_row_within(1, 0, 0..1);
        if full {
            assert_eq!(copied, Err(super::StorageError::HyperlinkMapOutOfMemory));
            assert_eq!(page.hyperlink_set.ref_count(id), before);
            assert!(!page.get_cells(1)[0].hyperlink());
        } else {
            copied.unwrap();
            assert_eq!(page.hyperlink_set.ref_count(id), before + 1);
            assert_eq!(page.get_cells(1)[0].codepoint(), 'A');
            assert!(page.get_cells(1)[0].hyperlink());
        }
        page.clear_cells(0, 0..120);
        assert_eq!(page.hyperlink_count(), usize::from(!full));
        assert_eq!(page.hyperlink_set.count(), usize::from(!full));
        assert_eq!(page.string_alloc.used_bytes(), if full { 0 } else { 32 });
    }
}
