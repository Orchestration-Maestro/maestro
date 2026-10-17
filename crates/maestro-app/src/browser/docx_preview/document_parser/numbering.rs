//! Concrete instance preparation from owned raw numbering.
use super::super::document::common::integer_prefix;
use super::super::numbering::parser::{Parsed, PictureSelection, PreparedLevel, ReferenceKey};
use super::super::{IDomNumbering, NumberingPicBullet};
use std::collections::HashMap;
/// Hashable numeric identity with an explicit null value.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum NumericKey {
    /// Missing numeric attribute.
    Null,
    /// Canonical number bits.
    Number(u64),
}
/// Merge zero signs and exclude NaN from numeric matching.
fn numeric_key(value: Option<f64>) -> Option<NumericKey> {
    match value {
        Some(v) if v.is_nan() => None,
        Some(v) => Some(NumericKey::Number(if v == 0.0 { 0 } else { v.to_bits() })),
        None => Some(NumericKey::Null),
    }
}
/// Resolve all pictures once, keeping the first complete numeric match.
fn pictures(parsed: &Parsed) -> HashMap<NumericKey, NumberingPicBullet> {
    let mut result = HashMap::new();
    for picture in parsed.part.bullet_pictures.iter().flatten() {
        let id = picture.id.as_deref().map(|id| integer_prefix(id, 10));
        if let Some(key) = numeric_key(id) {
            result.entry(key).or_insert_with(|| NumberingPicBullet {
                id,
                src: picture.reference_id.clone(),
                style: picture.style.clone(),
            });
        }
    }
    result
}
/// Bind one deferred selector against the complete picture index.
fn bind(
    level: &PreparedLevel,
    pictures: &HashMap<NumericKey, NumberingPicBullet>,
) -> IDomNumbering {
    let mut dom = level.dom.clone();
    dom.bullet = match level.picture {
        PictureSelection::Absent => None,
        PictureSelection::Id(id) => {
            Some(numeric_key(id).and_then(|key| pictures.get(&key)).cloned())
        }
    };
    dom
}
/// Emit definition order, then concrete-reference order, then retained level order.
pub(crate) fn prepare(parsed: &Parsed) -> Vec<IDomNumbering> {
    let pictures = pictures(parsed);
    let mut references: HashMap<Option<&str>, Vec<usize>> = HashMap::new();
    for (index, key) in parsed.reference_keys.iter().enumerate() {
        if let ReferenceKey::Id(id) = key {
            references.entry(id.as_deref()).or_default().push(index);
        }
    }
    let mut output = Vec::new();
    for (definition, prepared) in parsed
        .part
        .abstract_numberings
        .iter()
        .zip(&parsed.definitions)
    {
        let base: Vec<_> = prepared
            .iter()
            .map(|level| bind(level, &pictures))
            .collect();
        if let Some(instances) = references.get(&definition.id.as_deref()) {
            for &index in instances {
                output.extend(instance(&base, parsed, index, &pictures));
            }
        } else {
            output.extend(base);
        }
    }
    output
}
/// Apply ordered replacements before starts, leaving raw arrays untouched.
fn instance(
    base: &[IDomNumbering],
    parsed: &Parsed,
    index: usize,
    pictures: &HashMap<NumericKey, NumberingPicBullet>,
) -> Vec<IDomNumbering> {
    let mut levels = base.to_vec();
    for (over, replacement) in parsed.part.numberings[index]
        .overrides
        .iter()
        .zip(&parsed.replacements[index])
    {
        let mut position = levels.iter().position(|level| level.level == over.level);
        if let Some(replacement) = replacement {
            let replacement = bind(replacement, pictures);
            if let Some(position) = position {
                levels[position] = replacement;
            } else {
                position = Some(levels.len());
                levels.push(replacement);
            }
        }
        if let (Some(position), Some(start)) = (position, over.start) {
            levels[position].start = start;
        }
    }
    for level in &mut levels {
        level.id = Some(parsed.part.numberings[index].id.clone());
    }
    levels
}
