//! Selected spreadsheet format behavior.

use super::split_fmt::split_fmt;

#[test]
fn section_splitting_preserves_empty_and_extra_sections() {
    partition(&["section_boundaries"]);
}

#[test]
fn section_splitting_protects_quotes_escapes_skip_and_fill() {
    partition(&["protected_section_separators"]);
}

#[test]
fn unclosed_section_literals_fail_before_evaluation() {
    partition(&["unterminated_quotes"]);
}

#[test]
fn general_recognition_uses_seven_ascii_case_insensitive_characters() {
    partition(&["general_recognition"]);
}

#[test]
fn conditions_distinguish_all_six_operators_at_the_threshold() {
    partition(&["condition_operators"]);
}

#[test]
fn alternate_calendar_matches_documented_date_witnesses() {
    partition(&["alternate_known_dates"]);
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    id: String,
    group: String,
    operation: String,
    args: serde_json::Value,
    expected: serde_json::Value,
}

fn partition(groups: &[&str]) {
    let queries: Vec<Query> =
        serde_json::from_str(include_str!("../../../tests/fixtures/ssf/evaluator.json")).unwrap();
    let mut count = 0;
    for query in queries
        .into_iter()
        .filter(|q| groups.contains(&q.group.as_str()))
    {
        let observed = observe(&query);
        assert_eq!(observed, query.expected, "{}", query.id);
        count += 1;
    }
    assert!(count > 0);
}

fn observe(query: &Query) -> serde_json::Value {
    use super::chkcond::{chkcond, parse_condition};
    use super::parse_date_code::{Calendar, parse_date_code_with_calendar};
    use serde_json::json;
    let args = &query.args;
    match query.operation.as_str() {
        "split" => {
            assert_eq!(args.as_object().unwrap().len(), 1);
            match split_fmt(args["format"].as_str().unwrap()) {
                Ok(sections) => json!({"ok": sections}),
                Err(error) => error_value(error),
            }
        }
        "is_general" => {
            assert_eq!(args.as_object().unwrap().len(), 2);
            json!({"ok": super::general::is_general(args["format"].as_str().unwrap(), usize::try_from(args["start"].as_u64().unwrap()).unwrap())})
        }
        "condition" => {
            assert_eq!(args.as_object().unwrap().len(), 2);
            json!({"ok": chkcond(args["value"].as_str().unwrap().parse().unwrap(), parse_condition(args["format"].as_str().unwrap()))})
        }
        "date_code" => {
            assert_eq!(args.as_object().unwrap().len(), 2);
            let date = parse_date_code_with_calendar(
                args["value"].as_str().unwrap().parse().unwrap(),
                args["date1904"].as_bool().unwrap(),
                Calendar::Hijri,
            )
            .unwrap();
            json!({"ok": {"D": date.serial_day, "T": date.seconds, "u": serde_json::from_str::<serde_json::Value>(ryu_js::Buffer::new().format(date.fraction)).unwrap(), "y": date.year, "m": date.month, "d": date.day, "H": date.hour, "M": date.minute, "S": date.second, "q": date.weekday}})
        }
        "eval" => {
            assert_eq!(args.as_object().unwrap().len(), 4);
            let value = if let Some(number) = args["value"].get("number") {
                super::GeneralValue::Number(number.as_str().unwrap().parse().unwrap())
            } else {
                super::GeneralValue::Text(args["value"]["text"].as_str().unwrap())
            };
            assert_eq!(args["value"].as_object().unwrap().len(), 1);
            match super::eval_fmt::eval_fmt(
                args["format"].as_str().unwrap(),
                value,
                args["date1904"].as_bool().unwrap(),
                u8::try_from(args["sections"].as_u64().unwrap()).unwrap(),
            ) {
                Ok(text) => json!({"ok": text}),
                Err(error) => error_value(error),
            }
        }
        _ => panic!("unknown operation"),
    }
}

fn error_value(error: super::FormatError) -> serde_json::Value {
    use super::FormatError;
    use serde_json::json;
    match error {
        FormatError::UnterminatedBlock => json!({"error": {"kind": "unterminated_block"}}),
        FormatError::UnrecognizedCharacter(character) => {
            json!({"error": {"kind": "unrecognized_character", "character": character}})
        }
        FormatError::UnterminatedString => json!({"error": {"kind": "unterminated_string"}}),
        FormatError::UnsupportedNumberFormat => {
            json!({"error": {"kind": "unsupported_number_format"}})
        }
        FormatError::InvalidDateFormat(token) => {
            json!({"error": {"kind": "invalid_date_format", "token": format!("{token:?}")}})
        }
    }
}

#[test]
fn selected_sections_emit_literals_escapes_skip_and_fill_exactly() {
    partition(&["literal_emission"]);
}

#[test]
fn condition_parsing_keeps_the_first_matching_ascii_threshold() {
    partition(&["condition_grammar"]);
}

#[test]
fn text_placeholders_repeat_and_preserve_supplied_text() {
    partition(&["text_placeholder"]);
}

#[test]
fn general_tokens_delegate_without_changing_numeric_precision() {
    partition(&["general_tokens"]);
}

#[test]
fn numeric_text_placeholders_use_shortest_spelling_not_general() {
    partition(&["numeric_text_placeholder"]);
}

#[test]
fn unknown_characters_fail_instead_of_becoming_literals() {
    partition(&["unsupported_characters"]);
}

#[test]
fn color_condition_and_metadata_blocks_do_not_emit_text() {
    partition(&["ignored_blocks"]);
}

#[test]
fn calendar_tokens_reuse_year_month_day_and_era_writers() {
    partition(&["calendar_fields"]);
}

#[test]
fn minute_tokens_follow_previous_hours_and_following_seconds() {
    partition(&["minute_context"]);
}

#[test]
fn meridiem_tokens_select_hour_mode_and_exact_labels() {
    partition(&["meridiem"]);
}

#[test]
fn elapsed_tokens_keep_serial_totals_and_clock_neighbors() {
    partition(&["elapsed_fields"]);
}

#[test]
fn time_fields_round_at_the_selected_subsecond_precision() {
    partition(&["time_rounding"]);
}

#[test]
fn midnight_carry_preserves_the_selected_date_epoch() {
    partition(&["midnight_epoch_carry"]);
}

#[test]
fn invalid_serials_return_empty_before_calendar_rendering() {
    partition(&["invalid_date_serials"]);
}

#[test]
fn date_epoch_reaches_calendar_and_elapsed_consumers_independently() {
    partition(&["epoch_consumption"]);
}

#[test]
fn calendar_markers_capture_the_first_parsed_calendar() {
    partition(&["calendar_activation"]);
}

#[test]
fn alternate_midnight_carry_keeps_the_active_calendar() {
    partition(&["alternate_midnight_carry"]);
}

#[test]
fn alternate_date_records_keep_epochs_sentinels_and_time_fields() {
    partition(&["alternate_calendar_records"]);
}

#[test]
fn unclosed_brackets_keep_the_syntax_failure_category() {
    partition(&["malformed_blocks"]);
}

#[test]
fn unsupported_date_patterns_keep_the_selected_field_error() {
    partition(&["unsupported_date_fields"]);
}

#[test]
fn date_carry_crosses_synthetic_days_without_losing_calendar_options() {
    partition(&["date_sentinel_carry"]);
}

#[test]
fn alternate_calendar_obeys_every_leap_cycle_position() {
    partition(&["alternate_cycle_boundaries"]);
}

#[test]
fn locale_blocks_emit_currency_only_for_nondates() {
    partition(&["currency_locale"]);
}

#[test]
fn numeric_fragments_keep_masks_decimals_exponents_and_modifiers() {
    partition(&["numeric_reassembly"]);
}

#[test]
fn fraction_fragments_reuse_mixed_fixed_and_improper_writers() {
    partition(&["fraction_reassembly"]);
}

#[test]
fn fixed_fraction_denominators_keep_adjacent_spaces() {
    partition(&["fraction_spacing_correction"]);
}

#[test]
fn unsupported_numeric_sections_keep_writer_failure_categories() {
    partition(&["unsupported_number_fields"]);
}

#[test]
fn selected_section_count_controls_magnitude_and_sign_position() {
    partition(&["section_signs"]);
    for format in ["[Red] 0", "*  0"] {
        assert_eq!(
            super::eval_fmt::eval_fmt(format, super::GeneralValue::Number(-1.0), false, 1).unwrap(),
            " -1"
        );
    }
}

#[test]
fn zero_fraction_denominators_use_the_existing_writer_error() {
    partition(&["zero_denominator_correction"]);
}

#[test]
fn selected_numeric_sections_keep_landed_rounding_and_precision_fixes() {
    partition(&["renderer_regressions"]);
}

#[test]
fn meridiem_labels_follow_noon_and_midnight_rounding() {
    for (period, noon, midnight) in [
        ("AM/PM", "PM", "AM"),
        ("A/P", "P", "A"),
        ("上午/下午", "下午", "上午"),
    ] {
        for (seconds, label) in [(43199.75, noon), (86399.75, midnight)] {
            assert_eq!(
                super::eval_fmt::eval_fmt(
                    &format!("h:mm:ss {period}"),
                    super::GeneralValue::Number(44927.0 + seconds / 86400.0),
                    false,
                    1,
                )
                .unwrap(),
                format!("12:00:00 {label}")
            );
        }
    }
}

#[test]
fn nonfraction_digit_literals_keep_their_separating_spaces() {
    for format in ["0 2024", "0\" \"2024", "0_)2024"] {
        assert_eq!(
            super::eval_fmt::eval_fmt(format, super::GeneralValue::Number(1.0), false, 1).unwrap(),
            "1 2024"
        );
    }
}

#[test]
fn general_tokens_resolve_after_preceding_numeric_joins() {
    assert_eq!(
        super::eval_fmt::eval_fmt("0General?", super::GeneralValue::Number(1.0), false, 1).unwrap(),
        "011"
    );
}

#[test]
fn negative_sections_use_magnitude_for_every_exponent_field() {
    let outputs: Vec<_> = [
        "(0.00E+00)",
        "(##0.0E+0)",
        "\"Mass=\"0.00E+00",
        "\\(0.00E+00\\)",
    ]
    .into_iter()
    .map(|format| {
        super::eval_fmt::eval_fmt(format, super::GeneralValue::Number(-1.0), false, 2).unwrap()
    })
    .collect();
    assert_eq!(
        outputs,
        ["(1.00E+00)", "(1.0E+0)", "Mass=1.00E+00", "(1.00E+00)"]
    );
}

#[test]
fn literal_prefixed_exponents_emit_each_sign_once() {
    for (format, positive, negative) in [
        ("\"Mass=\"0.00E+00", "Mass=1.00E+00", "-Mass=1.00E+00"),
        ("\"Mass=\"##0.0E+0", "Mass=1.0E+0", "-Mass=1.0E+0"),
    ] {
        for (value, expected) in [(1.0, positive), (-1.0, negative)] {
            assert_eq!(
                super::eval_fmt::eval_fmt(format, super::GeneralValue::Number(value), false, 1)
                    .unwrap(),
                expected
            );
        }
    }
}
