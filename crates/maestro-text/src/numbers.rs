//! Prepared locale-sensitive finite-number presentation.
use fixed_decimal::{Decimal, FloatPrecision, SignedRoundingMode, UnsignedRoundingMode};
use icu_decimal::{DecimalFormatter, options::DecimalFormatterOptions};
use std::{fmt, io};

/// Preserves the decimal library error, which does not implement the error trait.
#[derive(Debug)]
struct ConversionError(fixed_decimal::LimitError);
impl fmt::Display for ConversionError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(output)
    }
}
impl std::error::Error for ConversionError {}

/// A locale formatter captured from supplied environment observations.
pub struct LocaleNumberFormat(DecimalFormatter);
impl LocaleNumberFormat {
    /// Prepares grouping and decimal presentation using the shared locale selector.
    ///
    /// # Errors
    /// Returns a native formatter construction failure.
    pub fn new(environment: &dyn Fn(&str) -> Option<String>) -> io::Result<Self> {
        DecimalFormatter::try_new(
            crate::locale::select(environment).into(),
            DecimalFormatterOptions::default(),
        )
        .map(Self)
        .map_err(io::Error::other)
    }
    /// Formats a finite binary64 number with at most three fractional digits,
    /// half-expand rounding and locale grouping, retaining signed zero.
    ///
    /// # Errors
    /// Returns the decimal conversion failure for nonfinite values.
    pub fn format(&self, value: f64) -> io::Result<String> {
        let mut decimal = Decimal::try_from_f64(value, FloatPrecision::RoundTrip)
            .map_err(|error| io::Error::other(ConversionError(error)))?;
        decimal.round_with_mode(
            -3,
            SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        );
        decimal.trim_end();
        Ok(self.0.format_to_string(&decimal))
    }
}
