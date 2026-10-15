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
    /// Resolve an ID and render through the single selected-section evaluator.
    pub(super) fn format(
        &self,
        id: u32,
        value: GeneralValue<'_>,
        epoch: bool,
    ) -> Result<String, FormatError> {
        let code = {
            let table = self
                .table
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            table
                .get(&id)
                .or_else(|| super::default_map(id).and_then(|id| table.get(&id)))
                .map_or_else(
                    || super::default_str(id).unwrap_or("General").to_owned(),
                    Clone::clone,
                )
        };
        if is_general(&code, 0) {
            return Ok(general(value));
        }
        let (count, section) = choose_fmt(&code, value)?;
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
            _ => eval_fmt(section, value, epoch, count),
        }
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
