//! Unicode placeholder decoding and row-local placement runs.

use super::{
    image::Image,
    render::{RenderPlacement, rounded},
    storage::{ImageStorage, Location, PlacementId, PlacementKey},
};
use crate::{
    PageList,
    page::CellPosition,
    page_list::{Direction, Pin, RowIterator},
    style::Color,
};

/// Scalar marking a cell as an image placeholder.
pub const PLACEHOLDER: char = '\u{10eeee}';

/// Traverse placeholder runs from a pin through an inclusive final row.
#[must_use]
pub fn placement_iterator(pin: Pin, limit: Option<Pin>) -> PlacementIterator {
    PlacementIterator {
        rows: pin.row_iterator(Direction::RightDown, limit),
        row: None,
        initial_x: Some(pin.x),
    }
}

/// Cursor yielding one compatible placeholder run at a time.
pub struct PlacementIterator {
    /// Remaining retained rows.
    rows: RowIterator,
    /// Current row and next column.
    row: Option<Pin>,
    /// First row's requested starting column.
    initial_x: Option<u16>,
}

impl PlacementIterator {
    /// Decode the next run, borrowing current cell data only during this call.
    pub fn next(&mut self, pages: &PageList) -> Option<Placement> {
        let mut run: Option<IncompletePlacement> = None;
        loop {
            if self.row.is_none()
                && let Some(prev) = run
            {
                return Some(prev.complete());
            }
            let pin = if let Some(pin) = self.row {
                pin
            } else {
                let mut pin = self.rows.next(pages)?;
                pin.x = self.initial_x.take().unwrap_or(0);
                pin
            };
            let (row, cell) = pin.row_and_cell(pages)?;
            if !row.kitty_virtual_placeholder() {
                self.row = None;
                continue;
            }
            self.row = pin.right(pages, 1);
            if cell.codepoint() != PLACEHOLDER
                && let Some(prev) = run
            {
                return Some(prev.complete());
            }
            if cell.codepoint() != PLACEHOLDER {
                continue;
            }
            let mut curr = IncompletePlacement::init(pin, pages);
            let Some(prev) = &mut run else {
                curr.row = Some(curr.row.unwrap_or(0));
                curr.col = Some(curr.col.unwrap_or(0));
                run = Some(curr);
                continue;
            };
            if !prev.append(&curr) {
                self.row = Some(pin);
                return Some(prev.complete());
            }
        }
    }
}

/// A fragment of an image, expressed as a horizontal run of screen cells.
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    /// Retained origin of the run.
    pub pin: Pin,
    /// Foreground-encoded image identity, including optional high bits.
    pub image_id: u32,
    /// Underline-encoded prototype identity, or zero to select any prototype.
    pub placement_id: u32,
    /// First image column represented by this run.
    pub col: u32,
    /// Image row represented by this run.
    pub row: u32,
    /// Run length in cells.
    pub width: u32,
    /// Run height in cells.
    pub height: u32,
}

/// Optional cell metadata before continuation rules are applied.
struct IncompletePlacement {
    /// Origin retained from the first cell.
    pin: Pin,
    /// Mandatory low image bits.
    image_id_low: u32,
    /// Optional high image byte.
    image_id_high: Option<u8>,
    /// Nonzero underline identity.
    placement_id: Option<u32>,
    /// Optional image row.
    row: Option<u32>,
    /// Optional image column.
    col: Option<u32>,
    /// Accumulated cell width.
    width: u32,
}

impl IncompletePlacement {
    /// Decode colours and the first three suffix scalars of a placeholder.
    fn init(pin: Pin, pages: &PageList) -> Self {
        let style = pin
            .row_and_cell(pages)
            .and_then(|(_, cell)| pin.style(pages, *cell))
            .unwrap_or_default();
        let cps = pin
            .grapheme(pages, CellPosition { x: pin.x, y: pin.y })
            .unwrap_or_default();
        let placement_id = color_to_id(style.underline_color);
        Self {
            pin,
            image_id_low: color_to_id(style.fg_color),
            image_id_high: cps
                .get(2)
                .and_then(|cp| get_index(*cp))
                .and_then(|index| u8::try_from(index).ok()),
            placement_id: (placement_id != 0).then_some(placement_id),
            row: cps.first().and_then(|cp| get_index(*cp)),
            col: cps.get(1).and_then(|cp| get_index(*cp)),
            width: 1,
        }
    }

    /// Extend compatible implicit or explicitly adjacent columns.
    fn append(&mut self, other: &Self) -> bool {
        if self.image_id_low != other.image_id_low
            || self.placement_id != other.placement_id
            || other.row.is_some_and(|row| Some(row) != self.row)
            || other.col.is_some_and(|col| {
                Some(col) != self.col.and_then(|col| col.checked_add(self.width))
            })
            || other
                .image_id_high
                .is_some_and(|high| Some(high) != self.image_id_high)
        {
            return false;
        }
        self.width += 1;
        true
    }

    /// Resolve absent identity and fragment metadata.
    fn complete(&self) -> Placement {
        Placement {
            pin: self.pin,
            image_id: self.image_id_low | (u32::from(self.image_id_high.unwrap_or(0)) << 24),
            placement_id: self.placement_id.unwrap_or(0),
            col: self.col.unwrap_or(0),
            row: self.row.unwrap_or(0),
            width: self.width,
            height: 1,
        }
    }
}

/// Interpret indexed and direct colours as protocol identity bits.
fn color_to_id(color: Color) -> u32 {
    match color {
        Color::None => 0,
        Color::Palette(index) => u32::from(index),
        Color::Rgb(rgb) => (u32::from(rgb.r) << 16) | (u32::from(rgb.g) << 8) | u32::from(rgb.b),
    }
}

/// Decode a supported combining scalar, ignoring unknown values.
fn get_index(cp: char) -> Option<u32> {
    DIACRITICS
        .binary_search(&u32::from(cp))
        .ok()
        .and_then(|index| u32::try_from(index).ok())
}

/// Ordered combining scalars whose indices encode image fragment coordinates.
const DIACRITICS: [u32; 297] = [
    0x0305, 0x030D, 0x030E, 0x0310, 0x0312, 0x033D, 0x033E, 0x033F, 0x0346, 0x034A, 0x034B, 0x034C,
    0x0350, 0x0351, 0x0352, 0x0357, 0x035B, 0x0363, 0x0364, 0x0365, 0x0366, 0x0367, 0x0368, 0x0369,
    0x036A, 0x036B, 0x036C, 0x036D, 0x036E, 0x036F, 0x0483, 0x0484, 0x0485, 0x0486, 0x0487, 0x0592,
    0x0593, 0x0594, 0x0595, 0x0597, 0x0598, 0x0599, 0x059C, 0x059D, 0x059E, 0x059F, 0x05A0, 0x05A1,
    0x05A8, 0x05A9, 0x05AB, 0x05AC, 0x05AF, 0x05C4, 0x0610, 0x0611, 0x0612, 0x0613, 0x0614, 0x0615,
    0x0616, 0x0617, 0x0657, 0x0658, 0x0659, 0x065A, 0x065B, 0x065D, 0x065E, 0x06D6, 0x06D7, 0x06D8,
    0x06D9, 0x06DA, 0x06DB, 0x06DC, 0x06DF, 0x06E0, 0x06E1, 0x06E2, 0x06E4, 0x06E7, 0x06E8, 0x06EB,
    0x06EC, 0x0730, 0x0732, 0x0733, 0x0735, 0x0736, 0x073A, 0x073D, 0x073F, 0x0740, 0x0741, 0x0743,
    0x0745, 0x0747, 0x0749, 0x074A, 0x07EB, 0x07EC, 0x07ED, 0x07EE, 0x07EF, 0x07F0, 0x07F1, 0x07F3,
    0x0816, 0x0817, 0x0818, 0x0819, 0x081B, 0x081C, 0x081D, 0x081E, 0x081F, 0x0820, 0x0821, 0x0822,
    0x0823, 0x0825, 0x0826, 0x0827, 0x0829, 0x082A, 0x082B, 0x082C, 0x082D, 0x0951, 0x0953, 0x0954,
    0x0F82, 0x0F83, 0x0F86, 0x0F87, 0x135D, 0x135E, 0x135F, 0x17DD, 0x193A, 0x1A17, 0x1A75, 0x1A76,
    0x1A77, 0x1A78, 0x1A79, 0x1A7A, 0x1A7B, 0x1A7C, 0x1B6B, 0x1B6D, 0x1B6E, 0x1B6F, 0x1B70, 0x1B71,
    0x1B72, 0x1B73, 0x1CD0, 0x1CD1, 0x1CD2, 0x1CDA, 0x1CDB, 0x1CE0, 0x1DC0, 0x1DC1, 0x1DC3, 0x1DC4,
    0x1DC5, 0x1DC6, 0x1DC7, 0x1DC8, 0x1DC9, 0x1DCB, 0x1DCC, 0x1DD1, 0x1DD2, 0x1DD3, 0x1DD4, 0x1DD5,
    0x1DD6, 0x1DD7, 0x1DD8, 0x1DD9, 0x1DDA, 0x1DDB, 0x1DDC, 0x1DDD, 0x1DDE, 0x1DDF, 0x1DE0, 0x1DE1,
    0x1DE2, 0x1DE3, 0x1DE4, 0x1DE5, 0x1DE6, 0x1DFE, 0x20D0, 0x20D1, 0x20D4, 0x20D5, 0x20D6, 0x20D7,
    0x20DB, 0x20DC, 0x20E1, 0x20E7, 0x20E9, 0x20F0, 0x2CEF, 0x2CF0, 0x2CF1, 0x2DE0, 0x2DE1, 0x2DE2,
    0x2DE3, 0x2DE4, 0x2DE5, 0x2DE6, 0x2DE7, 0x2DE8, 0x2DE9, 0x2DEA, 0x2DEB, 0x2DEC, 0x2DED, 0x2DEE,
    0x2DEF, 0x2DF0, 0x2DF1, 0x2DF2, 0x2DF3, 0x2DF4, 0x2DF5, 0x2DF6, 0x2DF7, 0x2DF8, 0x2DF9, 0x2DFA,
    0x2DFB, 0x2DFC, 0x2DFD, 0x2DFE, 0x2DFF, 0xA66F, 0xA67C, 0xA67D, 0xA6F0, 0xA6F1, 0xA8E0, 0xA8E1,
    0xA8E2, 0xA8E3, 0xA8E4, 0xA8E5, 0xA8E6, 0xA8E7, 0xA8E8, 0xA8E9, 0xA8EA, 0xA8EB, 0xA8EC, 0xA8ED,
    0xA8EE, 0xA8EF, 0xA8F0, 0xA8F1, 0xAAB0, 0xAAB2, 0xAAB3, 0xAAB7, 0xAAB8, 0xAABE, 0xAABF, 0xAAC1,
    0xFE20, 0xFE21, 0xFE22, 0xFE23, 0xFE24, 0xFE25, 0xFE26, 0x10A0F, 0x10A38, 0x1D185, 0x1D186,
    0x1D187, 0x1D188, 0x1D189, 0x1D1AA, 0x1D1AB, 0x1D1AC, 0x1D1AD, 0x1D242, 0x1D243, 0x1D244,
];

/// Failure to resolve a placeholder's prototype grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// No matching prototype exists for the image.
    PlacementMissingPlacement,
    /// The grid cannot be represented in terminal cell counts.
    PlacementGridOutOfBounds,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

impl Placement {
    /// Fit the image into its prototype grid and select this run's fragment.
    ///
    /// # Errors
    /// Returns an error for missing prototypes or unrepresentable grid dimensions.
    pub fn render_placement(
        &self,
        storage: &ImageStorage,
        image: &Image,
        cell: (u32, u32),
    ) -> Result<RenderPlacement, Error> {
        let (cols, rows) = self.grid(storage, image, cell)?;
        let grid_width = f64::from(cols) * f64::from(cell.0);
        let grid_height = f64::from(rows) * f64::from(cell.1);
        let width = f64::from(image.width);
        let height = f64::from(image.height);
        let scale = if width * grid_height > height * grid_width {
            grid_width / width.max(1.0)
        } else {
            grid_height / height.max(1.0)
        };
        let x = fragment_axis(
            width,
            grid_width,
            scale,
            (self.col, self.width, cols),
            cell.0,
        );
        let y = fragment_axis(
            height,
            grid_height,
            scale,
            (self.row, self.height, rows),
            cell.1,
        );
        let mut result = RenderPlacement {
            top_left: self.pin,
            offset_x: 0,
            offset_y: 0,
            source_x: 0,
            source_y: 0,
            source_width: 0,
            source_height: 0,
            dest_width: 0,
            dest_height: 0,
        };
        if x.1 <= 0.0 || y.1 <= 0.0 {
            return Ok(result);
        }
        result.offset_x = rounded(x.2);
        result.offset_y = rounded(y.2);
        result.source_x = rounded(x.0);
        result.source_y = rounded(y.0);
        result.source_width = rounded(x.1);
        result.source_height = rounded(y.1);
        result.dest_width = rounded(x.3);
        result.dest_height = rounded(y.3);
        Ok(result)
    }

    /// Resolve the prototype shared by fragment rendering and image projection.
    pub(super) fn prototype<'a>(
        &self,
        storage: &'a ImageStorage,
    ) -> Option<&'a super::storage::Placement> {
        if self.placement_id > 0 {
            storage.placements.get(&PlacementKey {
                image_id: self.image_id,
                placement_id: PlacementId::External(self.placement_id),
            })
        } else {
            storage
                .placements
                .iter()
                .find(|(key, placement)| {
                    key.image_id == self.image_id && placement.location == Location::Virtual
                })
                .map(|(_, placement)| placement)
        }
    }

    /// Resolve the prototype's explicit or image-sized grid.
    fn grid(
        &self,
        storage: &ImageStorage,
        image: &Image,
        cell: (u32, u32),
    ) -> Result<(u32, u32), Error> {
        let placement = self
            .prototype(storage)
            .ok_or(Error::PlacementMissingPlacement)?;
        let cols = if placement.columns > 0 {
            placement.columns
        } else {
            image
                .width
                .checked_div(cell.0)
                .map_or(0, |_| image.width.div_ceil(cell.0))
        };
        let rows = if placement.rows > 0 {
            placement.rows
        } else {
            image
                .height
                .checked_div(cell.1)
                .map_or(0, |_| image.height.div_ceil(cell.1))
        };
        if cols == 0 || rows == 0 || u16::try_from(cols).is_err() || u16::try_from(rows).is_err() {
            return Err(Error::PlacementGridOutOfBounds);
        }
        Ok((cols, rows))
    }
}

/// Project one source axis through centered padding into a destination fragment.
fn fragment_axis(
    image: f64,
    grid: f64,
    scale: f64,
    fragment: (u32, u32, u32),
    cell: u32,
) -> (f64, f64, f64, f64) {
    let padding = (grid - image * scale) / 2.0 / scale;
    let padded = image + padding * 2.0;
    let mut source = padded * (f64::from(fragment.0) / f64::from(fragment.2));
    let mut length = padded * (f64::from(fragment.1) / f64::from(fragment.2));
    let mut offset = 0.0;
    let mut dest = f64::from(fragment.1) * f64::from(cell);
    if source < padding {
        offset = padding - source;
        length -= offset;
        dest -= offset * scale;
        source = 0.0;
        if length > image {
            length = image;
            dest = image * scale;
        }
    } else if source + length > padded - padding {
        source -= padding;
        length = padded - padding - source;
        length -= padding;
        dest = length * scale;
    } else {
        source -= padding;
    }
    (source, length, offset * scale, dest)
}

#[cfg(test)]
mod tests {
    use super::DIACRITICS;

    #[test]
    fn diacritic_lookup_table_remains_strictly_ordered() {
        assert!(DIACRITICS.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
