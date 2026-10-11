//! Original pattern validation.

/// Failure to admit a package pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternError {
    /// The pattern exceeds 65,536 Unicode scalars.
    TooLong,
}

impl std::fmt::Display for PatternError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("pattern is too long")
    }
}

impl std::error::Error for PatternError {}

/// Checks the original pattern before interpreting its syntax.
pub(super) fn assert_valid_pattern(pattern: &str) -> Result<(), PatternError> {
    if pattern.chars().take(65_537).count() > 65_536 {
        Err(PatternError::TooLong)
    } else {
        Ok(())
    }
}
