//! Deletion selectors.

use super::ParseError;

/// Image deletion selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delete {
    /// All.
    All(bool),
    /// Id.
    Id {
        /// Delete.
        delete: bool,
        /// Image id.
        image_id: u32,
        /// Placement id.
        placement_id: u32,
    },
    /// Newest.
    Newest {
        /// Delete.
        delete: bool,
        /// Image number.
        image_number: u32,
        /// Placement id.
        placement_id: u32,
    },
    /// Intersect Cursor.
    IntersectCursor(bool),
    /// Animation Frames.
    AnimationFrames(bool),
    /// Intersect Cell.
    IntersectCell {
        /// Delete.
        delete: bool,
        /// X.
        x: u32,
        /// Y.
        y: u32,
    },
    /// Intersect Cell Z.
    IntersectCellZ {
        /// Delete.
        delete: bool,
        /// X.
        x: u32,
        /// Y.
        y: u32,
        /// Z.
        z: i32,
    },
    /// Range.
    Range {
        /// Delete.
        delete: bool,
        /// First.
        first: u32,
        /// Last.
        last: u32,
    },
    /// Column.
    Column {
        /// Delete.
        delete: bool,
        /// X.
        x: u32,
    },
    /// Row.
    Row {
        /// Delete.
        delete: bool,
        /// Y.
        y: u32,
    },
    /// Z.
    Z {
        /// Delete.
        delete: bool,
        /// Z.
        z: i32,
    },
}

impl Delete {
    /// Validate a deletion selector and retain its selected fields.
    pub(super) fn parse(keys: &super::parser::Keys) -> Result<Self, ParseError> {
        let selector =
            u8::try_from(keys.get(b'd', u32::from(b'a'))).map_err(|_| ParseError::InvalidFormat)?;
        let delete = selector.is_ascii_uppercase();
        let x = keys.get(b'x', 0);
        let y = keys.get(b'y', 0);
        let z = keys.signed(b'z');
        let placement_id = keys.get(b'p', 0);
        Ok(match selector.to_ascii_lowercase() {
            b'a' => Self::All(delete),
            b'i' => Self::Id {
                delete,
                image_id: keys.get(b'i', 0),
                placement_id,
            },
            b'n' => Self::Newest {
                delete,
                image_number: keys.get(b'I', 0),
                placement_id,
            },
            b'c' => Self::IntersectCursor(delete),
            b'f' => Self::AnimationFrames(delete),
            b'p' => Self::IntersectCell { delete, x, y },
            b'q' => Self::IntersectCellZ { delete, x, y, z },
            b'r' => {
                let first = keys.required(b'x')?;
                let last = keys.required(b'y')?;
                if first > last {
                    return Err(ParseError::InvalidFormat);
                }
                Self::Range {
                    delete,
                    first,
                    last,
                }
            }
            b'x' => Self::Column { delete, x },
            b'y' => Self::Row { delete, y },
            b'z' => Self::Z { delete, z },
            _ => return Err(ParseError::InvalidFormat),
        })
    }
}
