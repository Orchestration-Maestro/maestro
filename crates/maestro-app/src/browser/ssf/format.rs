//! Stateful table resolution and selected scalar formatting.
use super::{
    FormatError, GeneralValue,
    chkcond::{chkcond, parse_condition},
    eval_fmt::eval_fmt,
    general,
    general::is_general,
    split_fmt::split_fmt,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

/// Shared mutable format table retained across namespace replacement.
pub type FormatTable = Arc<RwLock<BTreeMap<u32, String>>>;
/// Authored number-format code or table identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumberFormat {
    /// Authored code, including an explicitly empty code.
    Code(String),
    /// Numeric identifier resolved through the live format table.
    Id(u32),
}
/// Per-call format selectors borrowed for synchronous rendering.
#[derive(Debug, Default)]
pub struct FormatOptions<'a> {
    /// Whether calendar serials use the 1904 epoch.
    pub date1904: bool,
    /// Nonempty replacement for the standard short-date format.
    pub date_nf: Option<&'a str>,
    /// Supplied table owner replacing direct active-table lookup.
    pub table: Option<&'a FormatTable>,
}
/// Caller-owned spreadsheet formatting namespace.
#[derive(Debug)]
pub struct Ssf {
    /// Current table slot.
    pub(super) table: FormatTable,
}
impl Default for Ssf {
    fn default() -> Self {
        let mut table = BTreeMap::new();
        super::init_table(&mut table);
        Self {
            table: Arc::new(RwLock::new(table)),
        }
    }
}
impl Ssf {
    /// Retain the live current table, independently of later reads.
    #[must_use]
    pub fn get_table(&self) -> FormatTable {
        Arc::clone(&self.table)
    }
    /// Load an explicit slot or deduplicate before selecting the first hole below 392.
    /// A full automatic range replaces slot 391.
    #[must_use]
    pub fn load(&self, format: &str, index: Option<u32>) -> u32 {
        let mut table = self
            .table
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let index = index.unwrap_or_else(|| {
            (0..392)
                .find(|id| table.get(id).is_some_and(|s| s == format))
                .or_else(|| (0..392).find(|id| !table.contains_key(id)))
                .unwrap_or(391)
        });
        table.insert(index, format.to_owned());
        index
    }
    /// Copy supplied slots below 392, retaining the live destination owner.
    /// Loading from the destination itself leaves every entry untouched.
    pub fn load_table(&self, table: &FormatTable) {
        if Arc::ptr_eq(&self.table, table) {
            return;
        }
        if Arc::as_ptr(&self.table).addr() < Arc::as_ptr(table).addr() {
            let mut target = self
                .table
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let source = table
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            copy_formats(&mut target, &source);
        } else {
            let source = table
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut target = self
                .table
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            copy_formats(&mut target, &source);
        }
    }
    /// Resolve a textual or numeric format and render a scalar.
    ///
    /// # Errors
    /// Returns the selected format's syntax or unsupported-field error.
    pub fn format(
        &self,
        format: &NumberFormat,
        value: &super::CellValue,
        options: &FormatOptions<'_>,
    ) -> Result<String, FormatError> {
        match self.format_locked(format, value, options)? {
            Dispatch::Text(text) => Ok(text),
            Dispatch::DateGeneral(serial) => self.format(
                &NumberFormat::Id(14),
                &super::CellValue::Number(serial),
                options,
            ),
        }
    }
    /// Keep table borrows local so Date-General recursion acquires no retained guard.
    fn format_locked(
        &self,
        format: &NumberFormat,
        value: &super::CellValue,
        options: &FormatOptions<'_>,
    ) -> Result<Dispatch, FormatError> {
        let supplied = options.table.unwrap_or(&self.table);
        let active = &self.table;
        if Arc::ptr_eq(active, supplied) {
            let table = active
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            return format_tables(format, value, options, (&table, &table));
        }
        let (first, second) = if Arc::as_ptr(active).addr() < Arc::as_ptr(supplied).addr() {
            (active, supplied)
        } else {
            (supplied, active)
        };
        let first_guard = first
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let second_guard = second
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let tables = if Arc::ptr_eq(first, active) {
            (&*first_guard, &*second_guard)
        } else {
            (&*second_guard, &*first_guard)
        };
        format_tables(format, value, options, tables)
    }
}
/// Rendering result or numeric date deferred until table guards are released.
enum Dispatch {
    /// Completed owned output.
    Text(String),
    /// Real-date serial requiring short-date dispatch with the same options.
    DateGeneral(f64),
}
/// Retain owned copies of bounded entries in the destination table.
fn copy_formats(target: &mut BTreeMap<u32, String>, source: &BTreeMap<u32, String>) {
    target.extend(source.range(..392).map(|(id, code)| (*id, code.clone())));
}
/// Resolve code references while both ordered table guards remain held.
fn format_tables(
    format: &NumberFormat,
    value: &super::CellValue,
    options: &FormatOptions<'_>,
    (active, supplied): (&BTreeMap<u32, String>, &BTreeMap<u32, String>),
) -> Result<Dispatch, FormatError> {
    let short_date = matches!(format, NumberFormat::Id(14))
        || matches!(format, NumberFormat::Code(s) if s == "m/d/yy");
    let code = if let Some(code) = options.date_nf.filter(|s| short_date && !s.is_empty()) {
        code
    } else {
        match format {
            NumberFormat::Code(code) => code,
            NumberFormat::Id(id) => supplied
                .get(id)
                .or_else(|| {
                    let mapped = super::default_map(*id)?;
                    supplied
                        .get(&mapped)
                        .filter(|s| !s.is_empty())
                        .or_else(|| active.get(&mapped))
                })
                .map_or_else(
                    || super::default_str(*id).unwrap_or("General"),
                    String::as_str,
                ),
        }
    };
    if let super::CellValue::Date(date) = value {
        let serial = super::datenum::datenum_date(date, options.date1904);
        if is_general(code, 0) {
            return Ok(Dispatch::DateGeneral(serial));
        }
        return render(code, GeneralValue::Number(serial), options.date1904).map(Dispatch::Text);
    }
    render(code, value.scalar(), options.date1904).map(Dispatch::Text)
}
/// Render a resolved code through the single selected-section evaluator.
fn render(code: &str, value: GeneralValue<'_>, epoch: bool) -> Result<String, FormatError> {
    if is_general(code, 0) {
        return Ok(general(value));
    }
    let (count, section) = choose_fmt(code, value)?;
    if is_general(section, 0) {
        return Ok(general(value));
    }
    match value {
        GeneralValue::Empty | GeneralValue::Text("") => Ok(String::new()),
        GeneralValue::Boolean(v) => eval_fmt(
            section,
            GeneralValue::Text(if v { "TRUE" } else { "FALSE" }),
            epoch,
            count,
        ),
        GeneralValue::Number(n) if !n.is_finite() && section.contains('0') => {
            Ok(if n.is_nan() { "#NUM!" } else { "#DIV/0!" }.to_owned())
        }
        _ => eval_fmt(section, value, epoch, count),
    }
}
/// Select a section by value class, sign and the first two conditions.
fn choose_fmt<'a>(code: &'a str, value: GeneralValue<'_>) -> Result<(u8, &'a str), FormatError> {
    let sections = split_fmt(code)?;
    let last = sections.last().copied().unwrap_or("");
    let text = last.contains('@');
    let count = u8::try_from(sections.len()).map_err(|_| FormatError::TooManySections)?;
    if count > 4 {
        return Err(FormatError::TooManySections);
    }
    let GeneralValue::Number(number) = value else {
        return Ok((4, if count == 4 || text { last } else { "@" }));
    };
    let numeric_count = count - u8::from(count < 4 && text);
    let numeric = &sections[..usize::from(numeric_count.min(3))];
    let first = numeric.first().copied().unwrap_or("General");
    let second = numeric.get(1).copied().unwrap_or(first);
    let third = numeric.get(2).copied().unwrap_or(first);
    let number = if number.is_finite() { number } else { 0.0 };
    let a = parse_condition(first);
    let b = parse_condition(second);
    let selected = if a.is_some() || b.is_some() {
        let both = a.is_some() && b.is_some();
        match (chkcond(number, a), chkcond(number, b), both) {
            (true, _, _) => first,
            (_, true, _) => second,
            (_, _, true) => third,
            _ => second,
        }
    } else if number > 0.0 {
        first
    } else if number < 0.0 {
        second
    } else {
        third
    };
    Ok((numeric_count, selected))
}
