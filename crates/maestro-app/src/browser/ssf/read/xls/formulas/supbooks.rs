//! Canonical workbook labels with indexed name views.
use super::super::super::blob::{ParseError, Result};
use super::super::{
    records::Payload,
    strings::{self, Encoding},
};
/// Defined-name spellings selected by their built-in identifier.
const BUILTINS: &[&str] = &[
    "_xlnm.Consolidate_Area",
    "_xlnm.Auto_Open",
    "_xlnm.Auto_Close",
    "_xlnm.Extract",
    "_xlnm.Database",
    "_xlnm.Criteria",
    "_xlnm.Print_Area",
    "_xlnm.Print_Titles",
    "_xlnm.Recorder",
    "_xlnm.Data_Form",
    "_xlnm.Auto_Activate",
    "_xlnm.Auto_Deactivate",
    "_xlnm.Sheet_Title",
    "_xlnm._FilterDatabase",
];
/// Owned label identity shared by global and book-local views.
struct Lbl {
    /// Authored name.
    name: String,
    /// One-based local sheet scope, or zero for global.
    itab: u16,
}
/// One source book's interpretation of external sheet indices.
enum SupBook {
    /// Current workbook.
    Internal,
    /// Name table without an explicit descriptor.
    Implicit,
    /// Add-in function names.
    Addin,
    /// External workbook sheet names.
    External(Vec<String>),
}
/// One book with indices into the canonical label owner.
struct Book {
    /// Book interpretation.
    kind: SupBook,
    /// Labels in book-local order.
    names: Vec<usize>,
}
/// A declared external-sheet selection.
enum Xti {
    /// Older layout directly stores the sheet name.
    Name(String),
    /// Newer layout selects a book and inclusive sheet pair.
    Range {
        /// Selected book descriptor.
        book: u16,
        /// First sheet index, or the deleted-sheet marker.
        first: i16,
        /// Last sheet index.
        last: i16,
    },
}
/// Owned workbook context constructed once during the record pass.
#[derive(Default)]
pub(in super::super) struct SupBooks {
    /// Canonical labels.
    labels: Vec<Lbl>,
    /// Global label view.
    names: Vec<usize>,
    /// Book-local label views.
    books: Vec<Book>,
    /// Ordered external-sheet selections.
    xti: Vec<Xti>,
    /// Canonical local sheet names, also used by final projection.
    pub(in super::super) sheet_names: Vec<String>,
}
impl SupBooks {
    /// Ensure that older or descriptor-free inputs have their implicit name owner.
    fn implicit(&mut self) {
        if self.books.is_empty() {
            self.books.push(Book {
                kind: SupBook::Implicit,
                names: Vec::new(),
            });
        }
    }
    /// Read a defined label without borrowing its record payload.
    pub(in super::super) fn label(
        &mut self,
        payload: &mut Payload<'_, '_>,
        encoding: Encoding,
    ) -> Result<()> {
        let flags = payload.u16()?;
        payload.u8()?;
        let count = usize::from(payload.u8()?);
        payload.skip(2)?;
        let wide = encoding == Encoding::Unicode;
        if wide {
            payload.skip(2)?;
        }
        let itab = payload.u16()?;
        if !wide {
            payload.skip(2)?;
        }
        payload.skip(4)?;
        let name = encoding.text(payload, count)?;
        let name = if flags & 0x20 != 0 {
            let id = name
                .chars()
                .next()
                .ok_or(ParseError::Invalid("built-in name"))?;
            BUILTINS
                .get(
                    usize::try_from(u32::from(id))
                        .map_err(|_| ParseError::Invalid("built-in name"))?,
                )
                .ok_or(ParseError::Invalid("built-in name"))?
                .to_string()
        } else {
            name
        };
        self.implicit();
        let index = self.labels.len();
        self.labels.push(Lbl { name, itab });
        self.names.push(index);
        if let Some(book) = self.books.last_mut() {
            book.names.push(index);
        }
        Ok(())
    }
    /// Read an explicit internal, add-in or external book descriptor.
    pub(in super::super) fn book(&mut self, payload: &mut Payload<'_, '_>) -> Result<()> {
        payload.u16()?;
        let count = payload.u16()?;
        let kind = match count {
            0x401 => SupBook::Internal,
            0x3a01 => SupBook::Addin,
            1..=255 => {
                Encoding::Unicode.text(payload, usize::from(count))?;
                let mut names = Vec::new();
                while payload.remaining() > 0 {
                    names.push(strings::unicode_string(payload, Encoding::Unicode)?);
                }
                SupBook::External(names)
            }
            _ => return Err(ParseError::Invalid("external book")),
        };
        self.books.push(Book {
            kind,
            names: Vec::new(),
        });
        Ok(())
    }
    /// Read the workbook's external-sheet index table in its declared layout.
    pub(in super::super) fn external_sheets(
        &mut self,
        payload: &mut Payload<'_, '_>,
        encoding: Encoding,
    ) -> Result<()> {
        self.implicit();
        if encoding == Encoding::Unicode {
            for _ in 0..payload.u16()? {
                self.xti.push(Xti::Range {
                    book: payload.u16()?,
                    first: i16::from_le_bytes(payload.u16()?.to_le_bytes()),
                    last: i16::from_le_bytes(payload.u16()?.to_le_bytes()),
                });
            }
        } else {
            self.xti.push(Xti::Name(older_sheet(payload, encoding)?));
        }

        Ok(())
    }
    /// Read an add-in name through its counted-string and padding boundary.
    pub(in super::super) fn external_name(
        &mut self,
        payload: &mut Payload<'_, '_>,
        encoding: Encoding,
    ) -> Result<()> {
        payload.u16()?;
        let Some(book) = self.books.last_mut() else {
            return Err(ParseError::Invalid("external name book"));
        };
        if !matches!(book.kind, SupBook::Addin) {
            return Ok(());
        }
        payload.skip(4)?;
        let name = strings::short_string(payload, encoding)?;
        let padding = usize::from(payload.u16()?);
        if padding != payload.remaining() {
            return Err(ParseError::Invalid("add-in padding"));
        }
        payload.skip(padding)?;
        book.names.push(self.labels.len());
        self.labels.push(Lbl { name, itab: 0 });
        Ok(())
    }
    /// Select a name from its original book and scope without copying the owner.
    pub(super) fn external(&self, book: i16, name: u32, encoding: Encoding) -> Result<String> {
        let book_index = usize::from(book.unsigned_abs());
        let book = self.books.get(book_index);
        let name_index = name
            .checked_sub(1)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(ParseError::Invalid("external name"))?;
        let label = book
            .and_then(|b| b.names.get(name_index))
            .or_else(|| self.books.first().and_then(|b| b.names.get(name_index)))
            .and_then(|i| self.labels.get(*i));
        let Some(label) = label else {
            return Err(ParseError::Invalid("external name"));
        };
        let prefix = match book.map(|b| &b.kind) {
            _ if encoding != Encoding::Unicode => String::new(),
            Some(SupBook::Addin) => String::new(),
            Some(SupBook::Internal) => {
                if label.itab == 0 {
                    String::new()
                } else {
                    format!(
                        "{}!",
                        sheet_name(
                            &self.sheet_names,
                            i16::try_from(label.itab - 1)
                                .map_err(|_| ParseError::Invalid("name scope"))?
                        )?
                    )
                }
            }
            _ => format!(
                "{}!",
                self.sheet_names
                    .get(name_index)
                    .ok_or(ParseError::Invalid("name scope"))?
            ),
        };
        Ok(format!("{prefix}{}", label.name))
    }
    /// Select a global label and strip only its leading function prefix.
    pub(super) fn name(&self, index: u32) -> String {
        index
            .checked_sub(1)
            .and_then(|n| usize::try_from(n).ok())
            .and_then(|i| self.names.get(i))
            .and_then(|i| self.labels.get(*i))
            .map_or_else(
                || format!("SH33TJSNAME{index}"),
                |label| {
                    label
                        .name
                        .strip_prefix("_xlfn.")
                        .unwrap_or(&label.name)
                        .to_owned()
                },
            )
    }
    /// Select and quote a 3-D sheet qualifier without normalizing its identity.
    pub(super) fn sheet(&self, index: u16, encoding: Encoding) -> Result<String> {
        let raw = if encoding == Encoding::Unicode {
            self.sheet_range(usize::from(index))?
        } else {
            let signed = i16::from_le_bytes(index.to_le_bytes());
            let index = usize::from(signed.unsigned_abs());
            if index == 0 {
                String::new()
            } else {
                match self.xti.get(index - 1) {
                    Some(Xti::Name(name)) => name.clone(),
                    _ => return Err(ParseError::Invalid("external sheet")),
                }
            }
        };
        Ok(if raw == "#REF" { raw } else { quote(&raw) })
    }
    /// Resolve a newer external-sheet pair against the selected book owner.
    fn sheet_range(&self, index: usize) -> Result<String> {
        let Some(Xti::Range { book, first, last }) = self.xti.get(index) else {
            return Err(ParseError::Invalid("external sheet"));
        };
        let book = self
            .books
            .get(usize::from(*book))
            .ok_or(ParseError::Invalid("external book"))?;
        let names = match &book.kind {
            SupBook::External(names) => names,
            _ => &self.sheet_names,
        };
        let first_name = if *first == -1 {
            "#REF"
        } else {
            sheet_name(names, *first)?
        };
        if first == last {
            return Ok(first_name.to_owned());
        }
        Ok(format!("{first_name}:{}", sheet_name(names, *last)?))
    }
}
/// Select one valid sheet index from the canonical name owner.
fn sheet_name(names: &[String], index: i16) -> Result<&str> {
    usize::try_from(index)
        .ok()
        .and_then(|i| names.get(i))
        .map(String::as_str)
        .ok_or(ParseError::Invalid("sheet index"))
}
/// Quote precisely the characters excluded by the binary formula grammar.
fn quote(name: &str) -> String {
    if name.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || c == '_'
            || ('\u{4e00}'..='\u{9fff}').contains(&c)
            || ('\u{3040}'..='\u{30ff}').contains(&c)
    }) {
        name.to_owned()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

/// The older sheet marker is outside the producer's declared character count.
fn older_sheet(payload: &mut Payload<'_, '_>, encoding: Encoding) -> Result<String> {
    let count = usize::from(payload.u8()?);
    if count == 0 {
        return Ok(String::new());
    }
    let first = payload.u8()?;
    if first == 3 {
        return encoding.text(payload, count);
    }
    let mut bytes = vec![first];
    bytes.extend(payload.bytes(count - 1)?);
    let Encoding::CodePage(page) = encoding else {
        return Err(ParseError::Invalid("older sheet encoding"));
    };
    Ok(strings::code_page(&bytes, page))
}
