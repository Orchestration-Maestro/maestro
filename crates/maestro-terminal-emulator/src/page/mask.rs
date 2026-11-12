//! Scalar comparisons of selected packed fields.

/// Select packed fields by their backing-integer bit mask.
#[cfg(test)]
pub(crate) struct Mask<const BITS: u64>;

#[cfg(test)]
impl<const BITS: u64> Mask<BITS> {
    /// Extract selected fields.
    pub(crate) fn pattern(value: u64) -> u64 {
        value & BITS
    }

    /// Clear only selected fields.
    pub(crate) fn strip(value: u64) -> u64 {
        value & !BITS
    }

    /// Whether every selected field is zero.
    pub(crate) fn matches(values: &[u64]) -> bool {
        Self::eql(values, 0)
    }

    /// Whether every masked value equals an already-masked pattern.
    pub(crate) fn eql(values: &[u64], expected: u64) -> bool {
        values.iter().all(|&value| Self::pattern(value) == expected)
    }

    /// Length of the leading run with the same masked pattern.
    pub(crate) fn eql_prefix(values: &[u64], expected: u64) -> usize {
        values
            .iter()
            .take_while(|&&value| Self::pattern(value) == expected)
            .count()
    }

    /// Whether every complete value equals the expected backing integer.
    pub(crate) fn eql_exact(values: &[u64], expected: u64) -> bool {
        values.iter().all(|&value| value == expected)
    }
}

#[cfg(test)]
mod tests;
