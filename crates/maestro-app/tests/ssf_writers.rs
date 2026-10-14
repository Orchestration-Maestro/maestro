//! Spreadsheet field writers through the application interface.
#![cfg(test)]

use maestro_app::browser::ssf::{
    DateCode, DateToken, FormatError, NumberToken, write_date, write_num,
};
use serde::Deserialize;

/// One consumed numeric query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NumberArgs {
    kind: String,
    format: String,
    value: String,
}

/// Complete writer outcome.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
enum Outcome {
    Ok(String),
    Error(String),
}

/// One independent oracle observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    id: String,
    group: String,
    operation: String,
    args: Args,
    expected: Outcome,
}

/// Typed arguments at the two primitive boundaries.
#[derive(Deserialize)]
#[serde(untagged)]
enum Args {
    Number(NumberArgs),
    Date(DateArgs),
}

/// Date field selection and supplied observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DateArgs {
    kind: String,
    format: String,
    date: DateObservation,
    subsecond_digits: u8,
}

/// Calendar record emitted by the independent oracle.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DateObservation {
    #[serde(rename = "D")]
    serial_day: i32,
    #[serde(rename = "T")]
    seconds: i32,
    #[serde(rename = "u")]
    fraction: f64,
    #[serde(rename = "y")]
    year: i32,
    #[serde(rename = "m")]
    month: u32,
    #[serde(rename = "d")]
    day: u32,
    #[serde(rename = "H")]
    hour: u32,
    #[serde(rename = "M")]
    minute: u32,
    #[serde(rename = "S")]
    second: u32,
    #[serde(rename = "q")]
    weekday: u32,
}

impl From<DateObservation> for DateCode {
    fn from(date: DateObservation) -> Self {
        Self {
            serial_day: date.serial_day,
            seconds: date.seconds,
            fraction: date.fraction,
            year: date.year,
            month: date.month,
            day: date.day,
            hour: date.hour,
            minute: date.minute,
            second: date.second,
            weekday: date.weekday,
        }
    }
}

/// Decode the closed date-writer vocabulary.
fn date_token(kind: &str) -> DateToken {
    match kind {
        "b" => DateToken::BuddhistYear,
        "y" => DateToken::Year,
        "m" => DateToken::Month,
        "d" => DateToken::Day,
        "h" => DateToken::Hour12,
        "H" => DateToken::Hour24,
        "M" => DateToken::Minute,
        "s" => DateToken::Second,
        "Z" => DateToken::Elapsed,
        "e" => DateToken::Era,
        _ => panic!("unknown date token"),
    }
}

/// Assert complete outcomes, including the selected failure category.
fn check(group: &str) {
    let queries: Vec<Query> =
        serde_json::from_str(include_str!("fixtures/ssf/writers.json")).unwrap();
    let mut count = 0;
    for query in queries.into_iter().filter(|query| query.group == group) {
        let (actual, error) = match query.args {
            Args::Number(args) => {
                assert_eq!(query.operation, "number");
                let kind = match args.kind.as_str() {
                    "n" | "?" => NumberToken::Number,
                    "(" => NumberToken::Parenthesis,
                    _ => panic!("unknown numeric token"),
                };
                (
                    write_num(kind, &args.format, args.value.parse().unwrap()),
                    FormatError::UnsupportedNumberFormat,
                )
            }
            Args::Date(args) => {
                assert_eq!(query.operation, "date");
                let kind = date_token(&args.kind);
                (
                    write_date(kind, &args.format, &args.date.into(), args.subsecond_digits),
                    FormatError::InvalidDateFormat(kind),
                )
            }
        };
        let expected = match query.expected {
            Outcome::Ok(text) => Ok(text),
            Outcome::Error(category) => {
                assert_eq!(
                    category,
                    match error {
                        FormatError::UnsupportedNumberFormat => "UnsupportedNumberFormat",
                        FormatError::InvalidDateFormat(_) => "InvalidDateFormat",
                        FormatError::UnterminatedString => "UnterminatedString",
                        FormatError::UnterminatedBlock => "UnterminatedBlock",
                        FormatError::UnrecognizedCharacter(_) => "UnrecognizedCharacter",
                    }
                );
                Err(error)
            }
        };
        assert_eq!(actual, expected, "{}", query.id);
        count += 1;
    }
    assert!(count > 0);
}

#[test]
fn integer_fields_keep_required_zeroes_and_optional_spaces() {
    check("integer_placeholders");
}

#[test]
fn rounding_preserves_sign_and_binary_half_boundaries() {
    check("rounding_signs");
}

#[test]
fn decimal_fields_keep_optional_digits_and_point_rules() {
    check("decimal_patterns");
}

#[test]
fn empty_fractional_fields_round_magnitude_before_restoring_sign() {
    for (format, near_zero) in [
        ("0.", "-0."),
        (".", "-."),
        ("#.", "-."),
        ("00.", "-0."),
        ("##0.", "-0."),
    ] {
        for (value, expected) in [(-1.5, "-2."), (-0.1, near_zero)] {
            assert_eq!(
                write_num(NumberToken::Number, format, value),
                Ok(expected.into()),
                "{format} at {value}"
            );
        }
    }
    for (format, value, expected) in [
        ("$0.", -1.5, "$-2."),
        ("0.%", -0.015, "-2.%"),
        ("0.,", -1500.0, "-2."),
    ] {
        assert_eq!(
            write_num(NumberToken::Number, format, value),
            Ok(expected.into()),
            "{format} at {value}"
        );
    }
}

#[test]
fn grouped_fields_keep_commas_zero_suppression_and_carry() {
    check("grouping");
    for format in ["#,##0.#", "00,000", "##,##0.00", "#,###."] {
        assert_eq!(
            write_num(NumberToken::Number, format, 1.25),
            Err(FormatError::UnsupportedNumberFormat)
        );
    }
    for format in ["#,##0.#", "#,###."] {
        assert_eq!(write_num(NumberToken::Number, format, 1.0), Ok("1.".into()));
    }
    assert_eq!(
        write_num(NumberToken::Number, "#,##0.00", 123_456.555),
        Ok("123,456.55".into())
    );
}

#[test]
fn padded_groups_carry_rounding_into_the_integer_field() {
    check("padded_grouping");
}

#[test]
fn percent_fields_scale_once_per_marker() {
    check("percent_scaling");
}

#[test]
fn trailing_commas_scale_before_remaining_modifiers() {
    check("comma_scaling");
}

#[test]
fn currency_and_parentheses_keep_sign_placement() {
    check("currency_parentheses");
}

#[test]
fn decimal_fields_align_question_placeholders() {
    check("decimal_placeholder_alignment");
}

#[test]
fn fixed_fractions_keep_denominator_and_blank_padding() {
    check("fixed_fractions");
    for format in ["0 # ?/2", "# ?/2foo"] {
        assert_eq!(
            write_num(NumberToken::Number, format, 1.25),
            Ok("1 1/2".into())
        );
    }
    assert_eq!(
        write_num(NumberToken::Number, "# ?  /2", 1.25),
        Err(FormatError::UnsupportedNumberFormat)
    );
}

#[test]
fn improper_fractions_align_numerator_and_denominator() {
    check("improper_fractions");
    assert_eq!(
        write_num(NumberToken::Number, "?/?foo", 1.25),
        Ok("5/4".into())
    );
}

#[test]
fn mixed_fractions_keep_whole_parts_and_precision_bound() {
    check("mixed_fractions");
    assert_eq!(
        write_num(NumberToken::Number, "# ?/?foo", 1.25),
        Ok("1 1/4".into())
    );
}

#[test]
fn digit_masks_keep_hyphens_and_phone_layout() {
    check("hyphens_phone");
}

#[test]
fn phone_masks_do_not_select_accounting_parentheses() {
    check("phone_parenthesis_guard");
}

#[test]
fn scientific_fields_keep_precision_and_exponent_signs() {
    check("scientific");
}

#[test]
fn zero_fraction_scientific_fields_omit_the_decimal_point() {
    for (format, exponent) in [
        ("0.E+00", "+00"),
        ("0.E-00", "0"),
        ("0.E+0", "+0"),
        ("0.E-0", "0"),
        ("0.E+000", "+0"),
    ] {
        for (value, mantissa) in [(1.0, "1"), (-1.0, "-1"), (0.0, "0")] {
            assert_eq!(
                write_num(NumberToken::Number, format, value),
                Ok(format!("{mantissa}E{exponent}")),
                "{format} at {value}"
            );
        }
        let negative_exponent = if format == "0.E+00" { "-02" } else { "-2" };
        for (value, mantissa) in [(0.01, "1"), (-0.01, "-1")] {
            assert_eq!(
                write_num(NumberToken::Number, format, value),
                Ok(format!("{mantissa}E{negative_exponent}")),
                "{format} at {value}"
            );
        }
    }
}

#[test]
fn engineering_fields_preserve_value_and_rounding_carry() {
    check("engineering");
}

#[test]
fn number_writers_keep_integer_dispatch_boundaries() {
    check("integer_dispatch");
}

#[test]
fn number_formats_keep_their_failure_category() {
    check("unsupported_number");
}

#[test]
fn zero_fixed_denominators_fail_before_rendering() {
    check("invalid_denominator");
}

#[test]
fn finite_integral_values_survive_fractional_precision() {
    check("builtin_precision_edges");
}

#[test]
fn year_fields_keep_width_buddhist_offset_and_era() {
    check("year_fields");
}

#[test]
fn month_fields_cover_every_name_and_width() {
    check("month_names");
}

#[test]
fn day_fields_keep_weekday_names_and_calendar_sentinels() {
    check("day_names");
}

#[test]
fn clock_fields_distinguish_twelve_and_twenty_four_hours() {
    check("clock_fields");
}

#[test]
fn second_fields_keep_selected_precision_and_wrap() {
    check("seconds_precision");
}

#[test]
fn elapsed_fields_use_serial_days_without_modulo() {
    check("elapsed_fields");
}

#[test]
fn date_writers_use_the_supplied_epoch_observations() {
    check("date_epoch_consumption");
}

#[test]
fn invalid_date_tokens_keep_their_failure_category() {
    check("unsupported_date");
}
