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
