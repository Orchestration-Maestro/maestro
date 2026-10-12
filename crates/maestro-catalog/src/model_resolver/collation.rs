//! Native ID comparison using caller-supplied locale operands.
use maestro_text::collation::Collator;
use std::{cmp::Ordering, io};
/// Prepare ID comparison from the first present locale environment operand.
///
/// Locale selection is decided by [`Collator::new`]; the returned closure retains
/// the prepared collator, not the reader.
///
/// # Errors
/// Returns a native collator construction failure.
pub fn model_id_comparator(
    environment: &dyn Fn(&str) -> Option<String>,
) -> io::Result<impl Fn(&str, &str) -> Ordering + use<>> {
    let collator = Collator::new(environment)?;
    Ok(move |left: &str, right: &str| collator.compare(left, right))
}
