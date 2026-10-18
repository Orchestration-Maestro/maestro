//! Page records and owned storage through the public interface.

use maestro_terminal::Cell;

#[test]
fn cell_codepoint_reads() {
    assert_eq!(Cell::init('A').codepoint(), 'A');
}

#[test]
fn cell_text_presence_reads() {
    assert!(Cell::init('A').has_text());
}

#[test]
fn cell_empty_reads() {
    assert!(!Cell::init('\0').has_text());
}

#[test]
fn cell_width_reads() {
    use maestro_terminal::page::cell::Wide;
    let mut cell = Cell::init('界');
    cell.set_wide(Wide::Wide);
    assert_eq!(cell.wide(), Wide::Wide);
    assert_eq!(cell.grid_width(), 2);
}

#[test]
fn cell_defaults_zero() {
    use maestro_terminal::page::cell::SemanticContent;
    let cell = Cell::default();
    assert!(cell.is_zero());
    assert_eq!(cell.codepoint(), '\0');
    assert_eq!(cell.semantic_content(), SemanticContent::Output);
    assert_eq!(std::mem::size_of::<Cell>(), 8);
}

#[test]
fn cell_typed_fields_match() {
    use maestro_terminal::{
        color::Rgb,
        page::cell::{Content, Wide},
    };
    let backgrounds = [
        Content::BgColorPalette(255),
        Content::BgColorRgb(Rgb { r: 1, g: 2, b: 3 }),
    ];
    for cp in ['\0', 'A', '\u{10EEEE}', '\u{10FFFF}'] {
        let contents = [
            Content::Codepoint(cp),
            Content::CodepointGrapheme(cp),
            backgrounds[0],
            backgrounds[1],
        ];
        for (tag, content) in contents.into_iter().enumerate() {
            for wide in [Wide::Narrow, Wide::Wide, Wide::SpacerTail, Wide::SpacerHead] {
                let mut cell = Cell::default();
                cell.set_content(content);
                cell.set_wide(wide);
                cell.set_style_id(42);
                check_cell_content(cell, content, tag, cp);
                assert_eq!(cell.grid_width(), if wide == Wide::Wide { 2 } else { 1 });
                assert_eq!(
                    cell.is_empty(),
                    tag < 2 && cp == '\0' && wide == Wide::Narrow
                );
            }
        }
    }
    assert!(!Cell::has_text_any(&[]));
    assert!(!Cell::default().has_styling());
}

#[test]
fn cell_semantic_and_protection_fields_roundtrip() {
    use maestro_terminal::page::cell::{Content, SemanticContent, Wide};
    for semantic in [
        SemanticContent::Output,
        SemanticContent::Input,
        SemanticContent::Prompt,
    ] {
        for id in [0, 1, u16::MAX] {
            for wide in [Wide::Narrow, Wide::Wide, Wide::SpacerTail, Wide::SpacerHead] {
                let mut cell = Cell::init('A');
                cell.set_semantic_content(semantic);
                cell.set_style_id(id);
                cell.set_wide(wide);
                check_cell_flags(cell);
                assert_eq!(cell.semantic_content(), semantic);
                assert_eq!(cell.style_id(), id);
                assert_eq!(cell.wide(), wide);
                assert_eq!(cell.content(), Content::Codepoint('A'));
            }
        }
    }
}

#[test]
fn row_wrap_reads() {
    let mut row = maestro_terminal::page::row::Row::default();
    row.set_wrap(true);
    assert!(row.wrap());
}

#[test]
fn row_dirty_reads() {
    let mut row = maestro_terminal::page::row::Row::default();
    row.set_dirty(true);
    assert!(row.dirty());
}

#[test]
fn row_all_typed_fields_roundtrip() {
    use maestro_terminal::page::row::{Row, SemanticPrompt};
    let flags: [RowFlag; 7] = [
        (Row::wrap, Row::set_wrap),
        (Row::wrap_continuation, Row::set_wrap_continuation),
        (Row::grapheme, Row::set_grapheme),
        (Row::styled, Row::set_styled),
        (Row::hyperlink, Row::set_hyperlink),
        (
            Row::kitty_virtual_placeholder,
            Row::set_kitty_virtual_placeholder,
        ),
        (Row::dirty, Row::set_dirty),
    ];
    assert_eq!(std::mem::size_of::<Row>(), 8);
    for prompt in [
        SemanticPrompt::None,
        SemanticPrompt::Prompt,
        SemanticPrompt::PromptContinuation,
    ] {
        for selected in 0..flags.len() {
            for value in [true, false] {
                check_row_flags(&flags, prompt, selected, value);
            }
        }
    }
}

#[test]
fn style_background_cells_override() {
    use maestro_terminal::{
        color::Rgb,
        page::cell::Content,
        style::{Color, Style},
    };
    let palette = std::array::from_fn(|index| Rgb {
        r: u8::try_from(index).unwrap(),
        g: 10,
        b: 20,
    });
    let style = Style {
        bg_color: Color::Palette(7),
        ..Style::default()
    };
    let mut cell = Cell::default();
    cell.set_content(Content::BgColorPalette(255));
    assert_eq!(
        style.bg(&cell, &palette),
        Some(Rgb {
            r: 255,
            g: 10,
            b: 20
        })
    );
    cell.set_content(Content::BgColorRgb(Rgb { r: 1, g: 2, b: 3 }));
    assert_eq!(style.bg(&cell, &palette), Some(Rgb { r: 1, g: 2, b: 3 }));
}

/// Check the content and style reads independently of width.
fn check_cell_content(
    cell: Cell,
    content: maestro_terminal::page::cell::Content,
    tag: usize,
    cp: char,
) {
    use maestro_terminal::page::cell::ContentTag;
    assert_eq!(cell.content(), content);
    assert_eq!(
        cell.content_tag(),
        [
            ContentTag::Codepoint,
            ContentTag::CodepointGrapheme,
            ContentTag::BgColorPalette,
            ContentTag::BgColorRgb
        ][tag]
    );
    assert_eq!(cell.has_text(), tag < 2 && cp != '\0');
    assert_eq!(cell.codepoint(), if tag < 2 { cp } else { '\0' });
    assert_eq!(cell.has_grapheme(), tag == 1);
    assert_eq!(cell.style_id(), 42);
    assert!(cell.has_styling());
    assert_eq!(
        Cell::has_text_any(&[Cell::default(), cell]),
        tag < 2 && cp != '\0'
    );
}

/// Toggle both flags without changing any other packed fields.
fn check_cell_flags(mut cell: Cell) {
    let original = cell;
    for protected in [true, false] {
        cell.set_protected(protected);
        for link in [true, false] {
            cell.set_hyperlink(link);
            assert_eq!(cell.protected(), protected);
            assert_eq!(cell.hyperlink(), link);
            let mut restored = cell;
            restored.set_protected(original.protected());
            restored.set_hyperlink(original.hyperlink());
            assert_eq!(restored, original);
        }
    }
}

/// Read and write one row flag.
type RowFlag = (
    fn(maestro_terminal::page::row::Row) -> bool,
    fn(&mut maestro_terminal::page::row::Row, bool),
);

/// Check one flag against independently held neighbouring fields.
fn check_row_flags(
    flags: &[RowFlag],
    prompt: maestro_terminal::page::row::SemanticPrompt,
    selected: usize,
    value: bool,
) {
    use maestro_terminal::page::row::{Row, SemanticPrompt};
    let mut row = Row::default();
    row.set_semantic_prompt(prompt);
    for &(_, setter) in flags {
        setter(&mut row, !value);
    }
    flags[selected].1(&mut row, value);
    for (index, &(read, _)) in flags.iter().enumerate() {
        assert_eq!(read(row), if index == selected { value } else { !value });
    }
    assert_eq!(row.semantic_prompt(), prompt);
    assert_eq!(
        row.managed_memory(),
        row.styled() || row.hyperlink() || row.grapheme()
    );
    row.set_semantic_prompt(SemanticPrompt::None);
    for (index, &(read, _)) in flags.iter().enumerate() {
        assert_eq!(read(row), if index == selected { value } else { !value });
    }
}

#[test]
fn style_color_precedence_matches() {
    use maestro_terminal::{
        color::Rgb,
        style::{Color, Fg, Style},
    };
    let mut palette = [Rgb::default(); 256];
    for (index, rgb) in [
        (0, [29, 31, 33]),
        (7, [197, 200, 198]),
        (8, [102, 102, 102]),
        (15, [234, 234, 234]),
        (255, [238, 238, 238]),
    ] {
        let [r, g, b] = rgb;
        palette[index] = Rgb { r, g, b };
    }
    let default = Rgb { r: 1, g: 2, b: 3 };
    let custom = Rgb { r: 9, g: 8, b: 7 };
    let other = Rgb { r: 4, g: 5, b: 6 };
    let choices = [
        (Color::None, default, default, custom),
        (Color::Palette(0), palette[0], palette[8], palette[8]),
        (Color::Palette(7), palette[7], palette[15], palette[15]),
        (Color::Palette(8), palette[8], palette[8], palette[8]),
        (
            Color::Palette(255),
            palette[255],
            palette[255],
            palette[255],
        ),
        (Color::Rgb(default), default, default, custom),
        (Color::Rgb(other), other, other, other),
    ];
    for case in choices {
        check_style_colors(case, &palette);
    }
    let style = Style {
        fg_color: Color::Palette(7),
        bg_color: Color::Palette(8),
        underline_color: Color::Rgb(other),
        ..Style::default()
    };
    assert_eq!(
        style.fg(Fg {
            default,
            palette: &palette,
            bold: None
        }),
        palette[7]
    );
    assert_eq!(style.bg(&Cell::default(), &palette), Some(palette[8]));
    assert_eq!(style.underline_color(&palette), Some(other));
}

/// Check the recorded foreground and background expectations for one colour source.
fn check_style_colors(
    case: (
        maestro_terminal::style::Color,
        maestro_terminal::color::Rgb,
        maestro_terminal::color::Rgb,
        maestro_terminal::color::Rgb,
    ),
    palette: &maestro_terminal::color::Palette,
) {
    use maestro_terminal::{
        color::Rgb,
        page::cell::Content,
        style::{BoldColor, Color, Fg, Style},
    };
    let (choice, base, bright, replacement) = case;
    let default = Rgb { r: 1, g: 2, b: 3 };
    let custom = Rgb { r: 9, g: 8, b: 7 };
    let mut style = Style {
        fg_color: choice,
        bg_color: choice,
        underline_color: choice,
        ..Style::default()
    };
    for bold in [false, true] {
        style.flags.set_bold(bold);
        let expected_bg = if choice == Color::None {
            None
        } else {
            Some(base)
        };
        assert_eq!(style.bg(&Cell::default(), palette), expected_bg);
        assert_eq!(style.underline_color(palette), expected_bg);
        assert_eq!(style.is_default(), choice == Color::None && !bold);
        let expected_cell = match choice {
            Color::None => None,
            Color::Palette(index) => Some(Content::BgColorPalette(index)),
            Color::Rgb(rgb) => Some(Content::BgColorRgb(rgb)),
        };
        assert_eq!(style.bg_cell().map(Cell::content), expected_cell);
        for (policy, expected) in [
            (None, base),
            (Some(BoldColor::Bright), if bold { bright } else { base }),
            (
                Some(BoldColor::Color(custom)),
                if bold { replacement } else { base },
            ),
        ] {
            assert_eq!(
                style.fg(Fg {
                    default,
                    palette,
                    bold: policy
                }),
                expected
            );
        }
    }
}

#[test]
fn page_initializes_storage() {
    let page = maestro_terminal::Page::init(maestro_terminal::page::layout::Capacity {
        cols: 120,
        rows: 80,
        styles: 32,
        ..Default::default()
    });
    assert_eq!(page.size.cols, 120);
    assert_eq!(page.size.rows, 80);
    for y in 0..80 {
        assert_eq!(page.get_cells(y), &[Cell::default(); 120]);
        assert!(!page.get_row(y).dirty());
    }
    assert!(!page.is_dirty());
}

#[test]
fn page_read_write_cells() {
    use maestro_terminal::{
        Page,
        page::{CellPosition, layout::Capacity},
    };
    let mut page = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    for y in 0..10 {
        let (row, cell) = page.get_row_and_cell(CellPosition { x: 1, y });
        row.set_wrap(y % 2 == 1);
        *cell = Cell::init(char::from_u32(u32::from(y)).unwrap());
    }
    for y in 0..10 {
        assert_eq!(page.get_cells(y)[1].codepoint() as u32, u32::from(y));
        assert_eq!(page.get_row(y).wrap(), y % 2 == 1);
        page.get_cells_mut(y)[0] = Cell::init('A');
        page.get_row_mut(y).set_dirty(true);
        assert_eq!(page.get_cells(y)[0].codepoint(), 'A');
    }
    assert!(page.is_dirty());
}

#[test]
fn page_append_small_cluster() {
    use maestro_terminal::{
        Page,
        page::{CellPosition, layout::Capacity},
    };
    let mut page = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    let position = CellPosition { x: 0, y: 0 };
    page.get_cells_mut(0)[0] = Cell::init('\t');
    page.append_grapheme(position, '\n').unwrap();
    assert_eq!(page.lookup_grapheme(position), Some(&['\n'][..]));
    assert!(page.get_cells(0)[0].has_grapheme());
    assert!(page.get_row(0).grapheme());
    page.append_grapheme(position, '\u{b}').unwrap();
    assert_eq!(page.lookup_grapheme(position), Some(&['\n', '\u{b}'][..]));
    page.clear_grapheme(position);
    page.update_row_grapheme_flag(0);
    assert_eq!(page.lookup_grapheme(position), None);
    assert!(!page.get_cells(0)[0].has_grapheme());
    assert!(!page.get_row(0).grapheme());
}

#[test]
fn page_append_crosses_chunk() {
    use maestro_terminal::{
        Page,
        page::{CellPosition, layout::Capacity},
    };
    let mut page = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    let position = CellPosition { x: 0, y: 0 };
    page.get_cells_mut(0)[0] = Cell::init('\t');
    let suffix: Vec<_> = (10..50).map(|cp| char::from_u32(cp).unwrap()).collect();
    for &cp in &suffix {
        page.append_grapheme(position, cp).unwrap();
    }
    assert_eq!(page.lookup_grapheme(position), Some(suffix.as_slice()));
}

#[test]
fn page_clear_partial_graphemes() {
    use maestro_terminal::{
        Page,
        page::{CellPosition, layout::Capacity},
    };
    let mut page = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    for x in 0..2 {
        page.get_cells_mut(0)[usize::from(x)] = Cell::init('A');
        page.append_grapheme(CellPosition { x, y: 0 }, 'B').unwrap();
    }
    page.clear_grapheme(CellPosition { x: 0, y: 0 });
    page.update_row_grapheme_flag(0);
    assert_eq!(page.lookup_grapheme(CellPosition { x: 0, y: 0 }), None);
    assert_eq!(
        page.lookup_grapheme(CellPosition { x: 1, y: 0 }),
        Some(&['B'][..])
    );
    assert!(page.get_row(0).grapheme());
    assert_eq!(page.grapheme_count(), 1);
}

#[test]
fn page_clone_preserves_records() {
    use maestro_terminal::{Page, page::layout::Capacity};
    let capacity = Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    };
    let mut source = Page::init(capacity);
    for y in 0..10 {
        source.get_cells_mut(y)[1] = Cell::init(char::from_u32(u32::from(y)).unwrap());
    }
    let cloned = source.clone();
    for y in 0..10 {
        source.get_cells_mut(y).fill(Cell::default());
    }
    assert_eq!(cloned.capacity, capacity);
    for y in 0..10 {
        assert_eq!(cloned.get_cells(y)[1].codepoint() as u32, u32::from(y));
        assert_eq!(source.get_cells(y), &[Cell::default(); 10]);
    }
}

#[test]
fn page_copy_range() {
    use maestro_terminal::{Page, page::layout::Capacity};
    let mut source = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    for y in 0..10 {
        source.get_cells_mut(y)[1] = Cell::init(char::from_u32(u32::from(y)).unwrap());
    }
    let mut destination = Page::init(source.capacity);
    destination.clone_from(&source, 0..10).unwrap();
    for y in 0..10 {
        source.get_cells_mut(y).fill(Cell::default());
    }
    for y in 0..10 {
        assert_eq!(destination.get_cells(y)[1].codepoint() as u32, u32::from(y));
        assert_eq!(source.get_cells(y), &[Cell::default(); 10]);
    }
}

#[test]
fn page_copy_truncates_columns() {
    use maestro_terminal::{Page, page::layout::Capacity};
    let mut source = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    for y in 0..10 {
        source.get_cells_mut(y)[1] = Cell::init(char::from_u32(u32::from(y)).unwrap());
    }
    let mut destination = Page::init(Capacity {
        cols: 5,
        ..source.capacity
    });
    destination.clone_from(&source, 0..10).unwrap();
    assert_eq!(destination.size.cols, 5);
    for y in 0..10 {
        assert_eq!(destination.get_cells(y)[1].codepoint() as u32, u32::from(y));
    }
}

#[test]
fn page_copy_selected_rows() {
    use maestro_terminal::{Page, page::layout::Capacity};
    let mut source = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    for y in 0..10 {
        source.get_cells_mut(y)[1] = Cell::init(char::from_u32(u32::from(y)).unwrap());
    }
    let mut destination = Page::init(source.capacity);
    destination.clone_from(&source, 0..5).unwrap();
    for y in 0..5 {
        assert_eq!(destination.get_cells(y)[1].codepoint() as u32, u32::from(y));
    }
    for y in 5..10 {
        assert_eq!(destination.get_cells(y), &[Cell::default(); 10]);
    }
}

#[test]
fn row_copy_partial() {
    use maestro_terminal::{Page, page::layout::Capacity};
    let mut source = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    for x in 0..10 {
        source.get_cells_mut(0)[x] =
            Cell::init(char::from_u32(u32::try_from(x + 1).unwrap()).unwrap());
    }
    let mut destination = Page::init(source.capacity);
    destination
        .clone_partial_row_from(&source, 0, 0, 2..8)
        .unwrap();
    for x in 0..10 {
        assert_eq!(
            destination.get_cells(0)[x].codepoint() as u32,
            if (2..8).contains(&x) {
                u32::try_from(x + 1).unwrap()
            } else {
                0
            }
        );
    }
}

#[test]
fn row_copy_retains_unselected_source_graphemes() {
    use maestro_terminal::{
        Page,
        page::{CellPosition, layout::Capacity},
    };
    let mut source = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    for x in [0, 9] {
        source.get_cells_mut(0)[usize::from(x)] = Cell::init('A');
        source
            .append_grapheme(CellPosition { x, y: 0 }, 'B')
            .unwrap();
    }
    source.get_cells_mut(0)[3] = Cell::init('C');
    let mut destination = Page::init(source.capacity);
    destination
        .clone_partial_row_from(&source, 0, 0, 2..8)
        .unwrap();
    assert_eq!(destination.get_cells(0)[3].codepoint(), 'C');
    assert_eq!(destination.grapheme_count(), 0);
    assert!(!destination.get_row(0).grapheme());
    assert!(
        destination
            .get_cells(0)
            .iter()
            .all(|cell| !cell.has_grapheme())
    );
    assert_eq!(source.grapheme_count(), 2);
}

#[test]
fn row_copy_retains_unselected_destination_graphemes() {
    use maestro_terminal::{
        Page,
        page::{CellPosition, layout::Capacity},
    };
    let mut source = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    for x in 0..10 {
        source.get_cells_mut(0)[x] =
            Cell::init(char::from_u32(u32::try_from(x + 1).unwrap()).unwrap());
    }
    let mut destination = Page::init(source.capacity);
    for x in [0, 9] {
        destination.get_cells_mut(0)[usize::from(x)] = Cell::init('»');
        destination
            .append_grapheme(CellPosition { x, y: 0 }, 'B')
            .unwrap();
    }
    destination
        .clone_partial_row_from(&source, 0, 0, 2..8)
        .unwrap();
    for x in 2..8 {
        assert_eq!(
            destination.get_cells(0)[x].codepoint() as u32,
            u32::try_from(x + 1).unwrap()
        );
    }
    for x in [0, 9] {
        assert_eq!(destination.get_cells(0)[usize::from(x)].codepoint(), '»');
        assert_eq!(
            destination.lookup_grapheme(CellPosition { x, y: 0 }),
            Some(&['B'][..])
        );
    }
    assert!(destination.get_row(0).grapheme());
    assert_eq!(destination.grapheme_count(), 2);
}

#[test]
fn page_move_plain_cells() {
    use maestro_terminal::{
        Page,
        page::{CellPosition, layout::Capacity},
    };
    let mut page = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    for x in 0..10 {
        page.get_cells_mut(0)[x] =
            Cell::init(char::from_u32(u32::try_from(x + 1).unwrap()).unwrap());
    }
    page.move_cells(CellPosition { x: 0, y: 0 }, CellPosition { x: 0, y: 1 }, 10);
    assert_eq!(page.get_cells(0), &[Cell::default(); 10]);
    for x in 0..10 {
        assert_eq!(
            page.get_cells(1)[x].codepoint() as u32,
            u32::try_from(x + 1).unwrap()
        );
    }
}

#[test]
fn page_reinit_clears_owned_tables() {
    use maestro_terminal::{
        Page,
        hyperlink::{Hyperlink, HyperlinkId},
        page::{CellPosition, layout::Capacity},
    };
    let mut page = Page::init(Capacity {
        cols: 10,
        rows: 10,
        ..Capacity::default()
    });
    let position = CellPosition { x: 0, y: 0 };
    page.get_cells_mut(0)[0] = Cell::init('A');
    page.append_grapheme(position, 'B').unwrap();
    let id = page
        .insert_hyperlink(Hyperlink {
            id: HyperlinkId::Implicit(1),
            uri: b"a".to_vec(),
        })
        .unwrap();
    page.set_hyperlink(position, id).unwrap();
    page.dirty = true;
    page.get_row_mut(0).set_dirty(true);
    page.reinit();
    assert_eq!(page.grapheme_count(), 0);
    assert_eq!(page.hyperlink_count(), 0);
    assert!(!page.is_dirty());
    for y in 0..10 {
        assert_eq!(page.get_cells(y), &[Cell::default(); 10]);
    }
}

#[test]
fn page_swap_preserves_owned_content() {
    use maestro_terminal::{
        Page,
        hyperlink::{Hyperlink, HyperlinkId},
        page::{CellPosition, layout::Capacity},
    };
    let first = CellPosition { x: 0, y: 0 };
    let second = CellPosition { x: 1, y: 0 };
    for (graphemes, links) in (0..4).flat_map(|g| (0..4).map(move |l| (g, l))) {
        let mut page = Page::init(Capacity {
            cols: 10,
            rows: 1,
            ..Capacity::default()
        });
        for (index, position) in [first, second].into_iter().enumerate() {
            page.get_cells_mut(0)[index] = Cell::init(['A', 'B'][index]);
            if graphemes & (1 << index) != 0 {
                page.append_grapheme(position, ['C', 'D'][index]).unwrap();
            }
            if links & (1 << index) != 0 {
                let id = page
                    .insert_hyperlink(Hyperlink {
                        id: HyperlinkId::Implicit(u32::try_from(index).unwrap()),
                        uri: vec![b'a' + u8::try_from(index).unwrap()],
                    })
                    .unwrap();
                page.set_hyperlink(position, id).unwrap();
            }
        }
        let records = [page.get_cells(0)[0], page.get_cells(0)[1]];
        page.swap_cells(first, second);
        assert_eq!(&page.get_cells(0)[..2], &[records[1], records[0]]);
        for (old, position) in [(1, first), (0, second)] {
            let expected = [['C', 'D'][old]];
            assert_eq!(
                page.lookup_grapheme(position),
                if graphemes & (1 << old) != 0 {
                    Some(&expected[..])
                } else {
                    None
                }
            );
            assert_eq!(
                page.lookup_hyperlink(position).is_some(),
                links & (1 << old) != 0
            );
        }
        page.clear_cells(0, 0..2);
        assert_eq!(page.grapheme_count(), 0);
        assert_eq!(page.hyperlink_count(), 0);
    }
}

#[test]
fn row_flags_and_dirty_sources_match() {
    use maestro_terminal::{
        Page,
        page::{CellPosition, layout::Capacity},
    };
    let mut source = Page::init(Capacity {
        cols: 2,
        rows: 1,
        ..Capacity::default()
    });
    source.dirty = true;
    assert!(source.is_dirty());
    source.get_row_mut(0).set_dirty(true);
    source.dirty = false;
    assert!(source.is_dirty());
    source.get_row_mut(0).set_dirty(false);
    assert!(!source.is_dirty());
    source.get_cells_mut(0)[0] = Cell::init('\u{10eeee}');
    source.get_row_mut(0).set_kitty_virtual_placeholder(true);
    source.get_cells_mut(0)[1] = Cell::init('A');
    source
        .append_grapheme(CellPosition { x: 1, y: 0 }, 'B')
        .unwrap();
    assert!(!source.is_dirty());
    let mut destination = Page::init(source.capacity);
    destination
        .clone_partial_row_from(&source, 0, 0, 0..1)
        .unwrap();
    assert!(destination.get_row(0).kitty_virtual_placeholder());
    assert!(!destination.get_row(0).grapheme());
    assert!(!destination.is_dirty());
    destination.clear_cells(0, 0..2);
    assert!(destination.get_row(0).kitty_virtual_placeholder());
    destination.clear_cells(0, 0..2);
    assert!(!destination.get_row(0).kitty_virtual_placeholder());
    assert!(!destination.is_dirty());
}

#[test]
fn abandoned_hyperlink_releases_string_budget() {
    use maestro_terminal::{
        Page,
        hyperlink::{Hyperlink, HyperlinkId},
        page::{StorageError, layout::Capacity},
    };
    let mut page = Page::init(Capacity {
        cols: 120,
        rows: 1,
        string_bytes: 1,
        ..Capacity::default()
    });
    let link = Hyperlink {
        id: HyperlinkId::Implicit(1),
        uri: vec![b'u'; 2048],
    };
    let retained = page.insert_hyperlink(link.clone()).unwrap();
    assert_eq!(
        page.insert_hyperlink(link.clone()),
        Err(StorageError::StringsOutOfMemory)
    );
    page.release_hyperlink(retained);
    assert!(page.insert_hyperlink(link).is_ok());
}

#[test]
fn refused_hyperlink_releases_string_budget() {
    use maestro_terminal::{
        Page,
        hyperlink::{Hyperlink, HyperlinkId},
        page::{CellPosition, StorageError, layout::Capacity},
    };
    let mut page = Page::init(Capacity {
        cols: 120,
        rows: 1,
        string_bytes: 1,
        ..Capacity::default()
    });
    let count = u16::try_from(page.hyperlink_capacity()).unwrap();
    for x in 0..count {
        let id = page
            .insert_hyperlink(Hyperlink {
                id: HyperlinkId::Implicit(1),
                uri: vec![b'u'],
            })
            .unwrap();
        page.set_hyperlink(CellPosition { x, y: 0 }, id).unwrap();
    }
    let incoming = Hyperlink {
        id: HyperlinkId::Implicit(2),
        uri: vec![b'v'; 2016],
    };
    let id = page.insert_hyperlink(incoming.clone()).unwrap();
    assert_eq!(
        page.set_hyperlink(CellPosition { x: count, y: 0 }, id),
        Err(StorageError::HyperlinkMapOutOfMemory)
    );
    assert!(!page.get_cells(0)[usize::from(count)].hyperlink());
    assert_eq!(page.hyperlink_count(), usize::from(count));
    assert_eq!(
        page.insert_hyperlink(incoming.clone()),
        Err(StorageError::StringsOutOfMemory)
    );
    page.release_hyperlink(id);
    assert!(page.insert_hyperlink(incoming).is_ok());
}

#[test]
fn partial_cross_page_copy_retains_placeholder_summary() {
    use maestro_terminal::{Page, page::layout::Capacity};
    for (incoming, surviving) in [(false, true), (true, false), (true, true), (false, false)] {
        let mut source = Page::init(Capacity {
            cols: 2,
            rows: 2,
            ..Capacity::default()
        });
        let copied = if incoming { '\u{10eeee}' } else { 'A' };
        let retained = if surviving { '\u{10eeee}' } else { '\0' };
        source.get_cells_mut(0)[1] = Cell::init(copied);
        source
            .get_row_mut(0)
            .set_kitty_virtual_placeholder(incoming);
        assert!(!source.get_row(0).managed_memory());
        let mut destination = Page::init(source.capacity);
        destination.get_cells_mut(0)[0] = Cell::init(retained);
        destination
            .get_row_mut(0)
            .set_kitty_virtual_placeholder(surviving);
        destination
            .clone_partial_row_from(&source, 0, 0, 1..2)
            .unwrap();
        assert_eq!(destination.get_cells(0)[0].codepoint(), retained);
        assert_eq!(destination.get_cells(0)[1].codepoint(), copied);
        assert_eq!(
            destination.get_row(0).kitty_virtual_placeholder(),
            incoming || surviving
        );
        destination.clone_row_from(&source, 0, 0).unwrap();
        assert_eq!(destination.get_cells(0)[0].codepoint(), '\0');
        assert_eq!(destination.get_cells(0)[1].codepoint(), copied);
        assert_eq!(destination.get_row(0).kitty_virtual_placeholder(), incoming);
    }
}

#[test]
fn partial_same_page_copy_retains_placeholder_summary() {
    use maestro_terminal::{Page, page::layout::Capacity};
    for (incoming, surviving) in [(false, true), (true, false), (true, true), (false, false)] {
        let mut page = Page::init(Capacity {
            cols: 2,
            rows: 2,
            ..Capacity::default()
        });
        let copied = if incoming { '\u{10eeee}' } else { 'A' };
        let retained = if surviving { '\u{10eeee}' } else { '\0' };
        page.get_cells_mut(0)[1] = Cell::init(copied);
        page.get_row_mut(0).set_kitty_virtual_placeholder(incoming);
        assert!(!page.get_row(0).managed_memory());
        page.get_cells_mut(1)[0] = Cell::init(retained);
        page.get_row_mut(1).set_kitty_virtual_placeholder(surviving);
        page.clone_partial_row_within(1, 0, 1..2).unwrap();
        assert_eq!(page.get_cells(1)[0].codepoint(), retained);
        assert_eq!(page.get_cells(1)[1].codepoint(), copied);
        assert_eq!(
            page.get_row(1).kitty_virtual_placeholder(),
            incoming || surviving
        );
        page.clone_partial_row_within(1, 0, 0..2).unwrap();
        assert_eq!(page.get_cells(1)[0].codepoint(), '\0');
        assert_eq!(page.get_cells(1)[1].codepoint(), copied);
        assert_eq!(page.get_row(1).kitty_virtual_placeholder(), incoming);
    }
}
