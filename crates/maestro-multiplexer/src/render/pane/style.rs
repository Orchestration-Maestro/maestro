//! Frame styles and symbols for emulator cells.

use crate::render::is_halfwidth_katakana_voiced_grapheme;
use maestro_app::daemon::{PaneRenderView, RgbColor as HostColor};
use maestro_app::terminal::{
    Color as CellColor, Content, Flags, PLACEHOLDER, Palette, RenderCell, Rgb, Wide,
};
use maestro_protocol::frames::with_underline_style;
use ratatui::{
    buffer::Cell,
    style::{Color, Modifier, Style},
};
use unicode_width::UnicodeWidthStr;

/// Palette entries a program redefined, as with OSC 4.
///
/// Forwarding an index to the host resolves it against the host's own
/// palette and loses the redefinition, so only redefined entries become RGB;
/// the rest stay indexed and keep following the host theme.
pub(super) struct PaletteOverrides([Option<Rgb>; 256]);

impl PaletteOverrides {
    /// Entries that differ from the defaults, or none when nothing was
    /// redefined, which is the common case.
    pub(super) fn new(active: &Palette, default: &Palette) -> Option<Self> {
        let overrides: [Option<Rgb>; 256] = std::array::from_fn(|index| {
            Some(active[index]).filter(|color| *color != default[index])
        });
        overrides
            .iter()
            .any(Option::is_some)
            .then_some(Self(overrides))
    }
}

/// A frame colour for a cell colour source; the default source has none.
pub(super) fn cell_color(color: CellColor, overrides: Option<&PaletteOverrides>) -> Option<Color> {
    match color {
        CellColor::None => None,
        CellColor::Palette(index) => Some(
            overrides
                .and_then(|overrides| overrides.0[usize::from(index)])
                .map_or(Color::Indexed(index), rgb),
        ),
        CellColor::Rgb(color) => Some(rgb(color)),
    }
}

/// A frame colour for an engine colour.
fn rgb(color: Rgb) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}

/// The engine default colour, or none when the host terminal already shows
/// it; without a host report the pane's initial default stands in.
fn default_color(color: Rgb, host: Option<HostColor>, initial: Rgb) -> Option<Color> {
    let shown = host.map_or(initial == color, |host| {
        (host.r, host.g, host.b) == (color.r, color.g, color.b)
    });
    (!shown).then(|| rgb(color))
}

/// Per-frame colour policy for converting emulator cells.
pub(super) struct CellStyler {
    /// Explicit default foreground, absent when the host shows it.
    default_fg: Option<Color>,
    /// Explicit default background, absent when the host shows it.
    default_bg: Option<Color>,
    /// Effective default foreground, used when inverse text needs one.
    resolved_fg: Color,
    /// Effective default background, used when inverse text needs one.
    resolved_bg: Color,
    /// Redefined palette entries.
    overrides: Option<PaletteOverrides>,
    /// Whether image placeholders are blanked because the host draws images.
    hide_placeholders: bool,
}

impl CellStyler {
    /// Read the colour policy of one refreshed pane snapshot.
    pub(super) fn new(view: &PaneRenderView<'_>) -> Self {
        let colors = &view.render_state.colors;
        let host = view.host_terminal_theme;
        Self {
            default_fg: default_color(
                colors.foreground,
                host.foreground,
                view.initial_default_foreground,
            ),
            default_bg: default_color(
                colors.background,
                host.background,
                view.initial_default_background,
            ),
            resolved_fg: rgb(colors.foreground),
            resolved_bg: rgb(colors.background),
            overrides: PaletteOverrides::new(&colors.palette, &view.default_palette),
            hide_placeholders: view.kitty_graphics_enabled,
        }
    }

    /// Style of an empty cell.
    pub(super) fn blank_style(&self) -> Style {
        frame_style(self.default_fg, self.default_bg)
    }

    /// Frame style of a cell: hidden text takes its background and inverse
    /// text swaps colours, resolving transparent ones first so it stays
    /// readable.
    pub(super) fn style(&self, cell: &RenderCell<'_>) -> Style {
        let source = cell.style();
        let overrides = self.overrides.as_ref();
        let mut fg = cell_color(source.fg_color, overrides).or(self.default_fg);
        let background = match cell.raw().content() {
            Content::BgColorPalette(index) => CellColor::Palette(index),
            Content::BgColorRgb(color) => CellColor::Rgb(color),
            _ => source.bg_color,
        };
        let mut bg = cell_color(background, overrides).or(self.default_bg);
        if source.flags.invisible() {
            fg = bg;
        }
        if source.flags.inverse() {
            (fg, bg) = (
                Some(bg.unwrap_or(self.resolved_bg)),
                Some(fg.unwrap_or(self.resolved_fg)),
            );
        }
        let mut style = frame_style(fg, bg).add_modifier(modifiers(source.flags));
        style.underline_color = cell_color(source.underline_color, overrides);
        style
    }

    /// Write the frame symbol of a cell.
    pub(super) fn symbol(&self, cell: &RenderCell<'_>, out: &mut String) {
        cell_symbol(cell, self.hide_placeholders, out);
    }
}

/// A style setting only the colours that are present.
fn frame_style(fg: Option<Color>, bg: Option<Color>) -> Style {
    Style {
        fg,
        bg,
        ..Style::default()
    }
}

/// Frame modifiers for cell attributes, with the underline style in the
/// frame-record bits.
fn modifiers(flags: Flags) -> Modifier {
    let underline = flags.underline() as u8;
    let modifier = [
        (flags.bold(), Modifier::BOLD),
        (flags.italic(), Modifier::ITALIC),
        (flags.faint(), Modifier::DIM),
        (flags.blink(), Modifier::SLOW_BLINK),
        (underline != 0, Modifier::UNDERLINED),
        (flags.strikethrough(), Modifier::CROSSED_OUT),
    ]
    .into_iter()
    .filter(|(set, _)| *set)
    .fold(Modifier::empty(), |modifier, (_, flag)| modifier | flag);
    Modifier::from_bits_retain(with_underline_style(modifier.bits(), underline))
}

/// Write a cell's symbol: its grapheme, or blanks of the cell's width when
/// the grapheme is empty, a hidden image placeholder or measures differently
/// than the cell, except where hosts draw the grapheme at the cell's width.
pub(super) fn cell_symbol(cell: &RenderCell<'_>, hide_placeholders: bool, out: &mut String) {
    let wide = cell.raw().wide();
    out.clear();
    match wide {
        Wide::SpacerTail => {}
        Wide::SpacerHead => out.push(' '),
        Wide::Narrow | Wide::Wide => {
            out.extend(cell.graphemes());
            if out.is_empty() || (hide_placeholders && out.starts_with(PLACEHOLDER)) {
                out.clear();
                out.push(' ');
            }
        }
    }
    let expected = match wide {
        Wide::Wide => 2,
        Wide::Narrow | Wide::SpacerHead => 1,
        Wide::SpacerTail => 0,
    };
    let width = out.width();
    let kept = width == expected
        || (wide == Wide::Narrow && width == 2)
        || (wide == Wide::Wide && is_halfwidth_katakana_voiced_grapheme(out));
    if !kept {
        out.clear();
        out.push_str(match wide {
            Wide::Wide => "  ",
            Wide::SpacerTail => "",
            Wide::Narrow | Wide::SpacerHead => " ",
        });
    }
}

/// Replace a frame cell's symbol and style.
pub(super) fn paint(target: &mut Cell, symbol: &str, style: Style) {
    target.reset();
    target.set_symbol(symbol);
    target.set_style(style);
}
