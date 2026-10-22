use super::*;
use maestro_protocol::frames::UNDERLINE_STYLE_MASK;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

/// Unpack a frame-record colour; unknown tags and names read as reset.
fn color_from_u32(value: u32) -> Color {
    let [tag, r, g, b] = value.to_be_bytes();
    match tag {
        0x00 => match b {
            0x01 => Color::Black,
            0x02 => Color::Red,
            0x03 => Color::Green,
            0x04 => Color::Yellow,
            0x05 => Color::Blue,
            0x06 => Color::Magenta,
            0x07 => Color::Cyan,
            0x08 => Color::Gray,
            0x09 => Color::DarkGray,
            0x0A => Color::LightRed,
            0x0B => Color::LightGreen,
            0x0C => Color::LightYellow,
            0x0D => Color::LightBlue,
            0x0E => Color::LightMagenta,
            0x0F => Color::LightCyan,
            0x10 => Color::White,
            _ => Color::Reset,
        },
        0x01 => Color::Indexed(b),
        0x02 => Color::Rgb(r, g, b),
        _ => Color::Reset,
    }
}

/// Unpack frame-record modifier bits, dropping the underline style.
fn modifier_from_u16(value: u16) -> Modifier {
    Modifier::from_bits_truncate(value & !UNDERLINE_STYLE_MASK)
}

/// Rebuild a buffer, or nothing when the cell count disagrees with the size.
fn buffer_from_frame(frame: &FrameData) -> Option<Buffer> {
    let expected = usize::from(frame.width) * usize::from(frame.height);
    if frame.cells.len() != expected {
        return None;
    }
    let area = Rect::new(0, 0, frame.width, frame.height);
    let mut buffer = Buffer::filled(area, Cell::new(" "));
    for (cell, data) in buffer.content.iter_mut().zip(&frame.cells) {
        cell.set_symbol(&data.symbol);
        cell.fg = color_from_u32(data.fg);
        cell.bg = color_from_u32(data.bg);
        cell.modifier = modifier_from_u16(data.modifier);
        if data.skip {
            cell.set_diff_option(CellDiffOption::Skip);
        }
    }
    Some(buffer)
}

/// A 5 by 3 buffer whose first three cells carry styled text.
fn styled_buffer() -> Buffer {
    let mut buffer = Buffer::filled(Rect::new(0, 0, 5, 3), Cell::new(" "));
    let cell = &mut buffer[(0, 0)];
    cell.set_symbol("H");
    cell.fg = Color::Red;
    cell.modifier = Modifier::BOLD;
    let cell = &mut buffer[(1, 0)];
    cell.set_symbol("i");
    cell.fg = Color::Green;
    cell.modifier = Modifier::ITALIC;
    let cell = &mut buffer[(2, 0)];
    cell.set_symbol("!");
    cell.fg = Color::Rgb(255, 128, 0);
    cell.bg = Color::Indexed(220);
    buffer
}

#[test]
fn frame_record_round_trips_through_a_frame_buffer() {
    let buffer = styled_buffer();
    let cursor = CursorState {
        x: 1,
        y: 0,
        visible: true,
        shape: 0,
    };

    let frame = frame_from_buffer(&buffer, Some(cursor.clone()), &[]);

    assert_eq!((frame.width, frame.height, frame.cells.len()), (5, 3, 15));
    assert_eq!(frame.cursor, Some(cursor));
    let cells: Vec<_> = frame.cells[..3]
        .iter()
        .map(|cell| (cell.symbol.as_str(), cell.fg, cell.bg, cell.modifier))
        .collect();
    let reset = color_to_u32(Color::Reset);
    assert_eq!(
        cells,
        [
            ("H", color_to_u32(Color::Red), reset, Modifier::BOLD.bits()),
            (
                "i",
                color_to_u32(Color::Green),
                reset,
                Modifier::ITALIC.bits()
            ),
            (
                "!",
                color_to_u32(Color::Rgb(255, 128, 0)),
                color_to_u32(Color::Indexed(220)),
                0
            ),
        ]
    );

    let restored = buffer_from_frame(&frame).unwrap();
    assert_eq!(restored.area, buffer.area);
    let restored: Vec<_> = restored.content[..3]
        .iter()
        .map(|cell| (cell.symbol(), cell.fg, cell.bg, cell.modifier))
        .collect();
    assert_eq!(
        restored,
        [
            ("H", Color::Red, Color::Reset, Modifier::BOLD),
            ("i", Color::Green, Color::Reset, Modifier::ITALIC),
            (
                "!",
                Color::Rgb(255, 128, 0),
                Color::Indexed(220),
                Modifier::empty()
            ),
        ]
    );
}

#[test]
fn frame_record_links_follow_their_drawn_symbol_and_share_targets() {
    let buffer = styled_buffer();
    let link = |x, symbol: &str| ((x, 0), symbol.to_owned(), "https://example.com".to_owned());
    let frame = frame_from_buffer(&buffer, None, &[link(0, "H"), link(1, "i"), link(2, "?")]);
    let links: Vec<_> = frame.cells[..3].iter().map(|cell| cell.hyperlink).collect();
    assert_eq!(links, [Some(0), Some(0), None]);
    assert_eq!(frame.hyperlinks, ["https://example.com"]);
}

#[test]
fn frame_record_rejects_mismatched_cell_count() {
    let cell = CellData {
        symbol: "X".into(),
        fg: 0,
        bg: 0,
        modifier: 0,
        skip: false,
        hyperlink: None,
    };
    let frame = FrameData {
        cells: vec![cell; 5],
        width: 3,
        height: 2,
        cursor: None,
        hyperlinks: Vec::new(),
        graphics: Vec::new(),
    };
    assert!(buffer_from_frame(&frame).is_none());
}

#[test]
fn frame_record_named_colors_round_trip() {
    let named = [
        Color::Reset,
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::Gray,
        Color::DarkGray,
        Color::LightRed,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightBlue,
        Color::LightMagenta,
        Color::LightCyan,
        Color::White,
    ];
    for (code, color) in (0..).zip(named) {
        assert_eq!(color_to_u32(color), code, "{color:?}");
        assert_eq!(color_from_u32(color_to_u32(color)), color, "{color:?}");
    }
}

#[test]
fn frame_record_indexed_color_round_trips() {
    for index in 0..=255u8 {
        let color = Color::Indexed(index);
        assert_eq!(color_to_u32(color), 0x01_00_00_00 | u32::from(index));
        assert_eq!(color_from_u32(color_to_u32(color)), color, "{index}");
    }
}

#[test]
fn frame_record_rgb_color_round_trips() {
    assert_eq!(color_to_u32(Color::Rgb(0xAB, 0xCD, 0xEF)), 0x02_AB_CD_EF);
    for color in [
        Color::Rgb(0xAB, 0xCD, 0xEF),
        Color::Rgb(0, 0, 0),
        Color::Rgb(255, 255, 255),
    ] {
        assert_eq!(color_from_u32(color_to_u32(color)), color);
    }
}

#[test]
fn frame_record_modifiers_round_trip() {
    for modifier in [
        Modifier::BOLD,
        Modifier::ITALIC,
        Modifier::REVERSED,
        Modifier::UNDERLINED,
        Modifier::DIM,
        Modifier::SLOW_BLINK,
        Modifier::CROSSED_OUT,
        Modifier::BOLD | Modifier::ITALIC,
        Modifier::BOLD | Modifier::UNDERLINED | Modifier::REVERSED,
        Modifier::empty(),
    ] {
        let mut cell = Cell::new("x");
        cell.modifier = modifier;
        assert_eq!(
            modifier_from_u16(cell_from_buffer(&cell).modifier),
            modifier,
            "{modifier:?}"
        );
    }
}

#[test]
fn frame_record_underline_style_rides_above_the_modifier_bits() {
    use maestro_protocol::frames::{underline_style_from_modifier, with_underline_style};
    let underlined = Modifier::UNDERLINED.bits();
    let curly = with_underline_style(underlined, 3);
    assert_eq!(underline_style_from_modifier(curly), 3);
    assert_eq!(curly & !UNDERLINE_STYLE_MASK, underlined);
    assert_eq!(modifier_from_u16(curly), Modifier::UNDERLINED);
    assert_eq!(
        underline_style_from_modifier(with_underline_style(0, 0x1F)),
        0x0F
    );
    assert_eq!(underline_style_from_modifier(underlined), 0);
}
