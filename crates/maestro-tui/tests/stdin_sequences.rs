//! Buffered terminal input: framing, deadlines, duplicate suppression and bracketed paste,
//! replayed from recorded operations against one buffer per case.
use std::time::{Duration, Instant};

use maestro_tui::{StdinBuffer, StdinBufferEventMap, StdinBufferInput, StdinBufferOptions};
use serde_json::{Value, json};

/// The recorded cases, each owned by one behavior test.
const CORPUS: &str = include_str!("fixtures/stdin_sequences.json");

/// Replays every recorded case of `test` and checks each step.
fn replay(test: &str) {
    let Ok(corpus) = serde_json::from_str::<Value>(CORPUS) else {
        unreachable!("the recorded cases are JSON");
    };
    let cases: Vec<&Value> = list(&corpus["cases"])
        .iter()
        .filter(|case| case["test"] == test)
        .collect();
    assert!(!cases.is_empty(), "no recorded cases for {test}");
    for case in cases {
        replay_case(case);
    }
}

/// Runs the steps of one case on a fresh buffer, advancing only a test instant.
fn replay_case(case: &Value) {
    let options =
        case["options"]["timeout"]
            .as_u64()
            .map_or_else(StdinBufferOptions::default, |ms| StdinBufferOptions {
                timeout: Duration::from_millis(ms),
            });
    let start = Instant::now();
    let mut now = start;
    let mut buffer = StdinBuffer::new(options);
    let mut observed = Vec::new();
    for (index, step) in list(&case["steps"]).iter().enumerate() {
        let label = format!("{} step {index}", case["id"]);
        let mut returned = None;
        match step["op"].as_str() {
            Some("process") => {
                let events = process(&mut buffer, &step["input"], now);
                observed.extend(events.iter().map(event_json));
            }
            Some("advance") => {
                now += Duration::from_millis(number(&step["ms"]));
                observed.extend(buffer.expire(now).iter().map(event_json));
            }
            Some("flush") => returned = Some(buffer.flush()),
            Some("clear") => buffer.clear(),
            Some("destroy") => buffer.destroy(),
            other => unreachable!("unknown operation {other:?}"),
        }
        let expected = &step["expected"];
        assert_eq!(json!(observed), expected["events"], "{label}: events");
        assert_eq!(
            json!(buffer.get_buffer()),
            expected["buffer"],
            "{label}: buffer"
        );
        let deadline = buffer
            .deadline()
            .map(|deadline| deadline.duration_since(start).as_millis());
        assert_eq!(json!(deadline), expected["deadline"], "{label}: deadline");
        if let Some(returned) = returned {
            assert_eq!(json!(returned), expected["returned"], "{label}: flushed");
        }
    }
}

/// Feeds the recorded text or byte input of a step to the buffer.
fn process(buffer: &mut StdinBuffer, input: &Value, now: Instant) -> Vec<StdinBufferEventMap> {
    if let Some(text) = input["text"].as_str() {
        return buffer.process(StdinBufferInput::Text(text), now);
    }
    let bytes: Vec<u8> = list(&input["bytes"])
        .iter()
        .map(|byte| u8::try_from(number(byte)).unwrap_or_default())
        .collect();
    buffer.process(StdinBufferInput::Bytes(&bytes), now)
}

/// The items of a recorded list.
fn list(value: &Value) -> &[Value] {
    value
        .as_array()
        .map_or_else(|| unreachable!("{value} is not a list"), Vec::as_slice)
}

/// A recorded unsigned number.
fn number(value: &Value) -> u64 {
    value
        .as_u64()
        .unwrap_or_else(|| unreachable!("{value} is not a number"))
}

/// An event in the recorded `[kind, text]` form.
fn event_json(event: &StdinBufferEventMap) -> Value {
    match event {
        StdinBufferEventMap::Data(text) => json!(["data", text]),
        StdinBufferEventMap::Paste(text) => json!(["paste", text]),
    }
}

#[test]
fn maestro_stdin_pass_through_regular_characters_immediately() {
    replay("maestro_stdin_pass_through_regular_characters_immediately");
}

#[test]
fn maestro_stdin_pass_through_multiple_regular_characters() {
    replay("maestro_stdin_pass_through_multiple_regular_characters");
}

#[test]
fn maestro_stdin_handle_unicode_characters() {
    replay("maestro_stdin_handle_unicode_characters");
}

#[test]
fn maestro_stdin_keeps_supplementary_characters_whole() {
    replay("maestro_stdin_keeps_supplementary_characters_whole");
}

#[test]
fn maestro_stdin_preserves_whitespace_and_control_scalars() {
    replay("maestro_stdin_preserves_whitespace_and_control_scalars");
}

#[test]
fn maestro_stdin_handle_empty_input() {
    replay("maestro_stdin_handle_empty_input");
}

#[test]
fn maestro_stdin_pass_through_complete_mouse_sgr_sequences() {
    replay("maestro_stdin_pass_through_complete_mouse_sgr_sequences");
}

#[test]
fn maestro_stdin_pass_through_complete_arrow_key_sequences() {
    replay("maestro_stdin_pass_through_complete_arrow_key_sequences");
}

#[test]
fn maestro_stdin_pass_through_complete_function_key_sequences() {
    replay("maestro_stdin_pass_through_complete_function_key_sequences");
}

#[test]
fn maestro_stdin_pass_through_meta_key_sequences() {
    replay("maestro_stdin_pass_through_meta_key_sequences");
}

#[test]
fn maestro_stdin_pass_through_ss3_sequences() {
    replay("maestro_stdin_pass_through_ss3_sequences");
}

#[test]
fn maestro_stdin_handle_characters_followed_by_escape_sequence() {
    replay("maestro_stdin_handle_characters_followed_by_escape_sequence");
}

#[test]
fn maestro_stdin_handle_escape_sequence_followed_by_characters() {
    replay("maestro_stdin_handle_escape_sequence_followed_by_characters");
}

#[test]
fn maestro_stdin_handle_multiple_complete_sequences() {
    replay("maestro_stdin_handle_multiple_complete_sequences");
}

#[test]
fn maestro_stdin_handle_kitty_csi_u_press_events() {
    replay("maestro_stdin_handle_kitty_csi_u_press_events");
}

#[test]
fn maestro_stdin_handle_kitty_csi_u_release_events() {
    replay("maestro_stdin_handle_kitty_csi_u_release_events");
}

#[test]
fn maestro_stdin_handle_batched_kitty_press_and_release() {
    replay("maestro_stdin_handle_batched_kitty_press_and_release");
}

#[test]
fn maestro_stdin_handle_multiple_batched_kitty_events() {
    replay("maestro_stdin_handle_multiple_batched_kitty_events");
}

#[test]
fn maestro_stdin_handle_kitty_arrow_keys_with_event_type() {
    replay("maestro_stdin_handle_kitty_arrow_keys_with_event_type");
}

#[test]
fn maestro_stdin_handle_kitty_functional_keys_with_event_type() {
    replay("maestro_stdin_handle_kitty_functional_keys_with_event_type");
}

#[test]
fn maestro_stdin_handle_plain_characters_mixed_with_kitty_sequences() {
    replay("maestro_stdin_handle_plain_characters_mixed_with_kitty_sequences");
}

#[test]
fn maestro_stdin_handle_rapid_typing_simulation_with_kitty_protocol() {
    replay("maestro_stdin_handle_rapid_typing_simulation_with_kitty_protocol");
}

#[test]
fn maestro_stdin_handle_mouse_press_event() {
    replay("maestro_stdin_handle_mouse_press_event");
}

#[test]
fn maestro_stdin_handle_mouse_release_event() {
    replay("maestro_stdin_handle_mouse_release_event");
}

#[test]
fn maestro_stdin_handle_multiple_mouse_events() {
    replay("maestro_stdin_handle_multiple_mouse_events");
}

#[test]
fn maestro_stdin_handle_old_style_mouse_sequence_esc_m_3_bytes() {
    replay("maestro_stdin_handle_old_style_mouse_sequence_esc_m_3_bytes");
}

#[test]
fn maestro_stdin_handle_very_long_sequences() {
    replay("maestro_stdin_handle_very_long_sequences");
}

#[test]
fn maestro_stdin_counts_complete_scalars_in_legacy_mouse_reports() {
    replay("maestro_stdin_counts_complete_scalars_in_legacy_mouse_reports");
}

#[test]
fn maestro_stdin_frames_meta_ss3_and_legacy_mouse() {
    replay("maestro_stdin_frames_meta_ss3_and_legacy_mouse");
}

#[test]
fn maestro_stdin_recognizes_csi_final_byte_boundaries() {
    replay("maestro_stdin_recognizes_csi_final_byte_boundaries");
}

#[test]
fn maestro_stdin_buffer_incomplete_mouse_sgr_sequence() {
    replay("maestro_stdin_buffer_incomplete_mouse_sgr_sequence");
}

#[test]
fn maestro_stdin_buffer_incomplete_csi_sequence() {
    replay("maestro_stdin_buffer_incomplete_csi_sequence");
}

#[test]
fn maestro_stdin_buffer_split_across_many_chunks() {
    replay("maestro_stdin_buffer_split_across_many_chunks");
}

#[test]
fn maestro_stdin_flush_incomplete_sequence_after_timeout() {
    replay("maestro_stdin_flush_incomplete_sequence_after_timeout");
}

#[test]
fn maestro_stdin_handle_partial_sequence_with_preceding_characters() {
    replay("maestro_stdin_handle_partial_sequence_with_preceding_characters");
}

#[test]
fn maestro_stdin_buffer_incomplete_old_style_mouse_sequence() {
    replay("maestro_stdin_buffer_incomplete_old_style_mouse_sequence");
}

#[test]
fn maestro_stdin_handle_split_mouse_events() {
    replay("maestro_stdin_handle_split_mouse_events");
}

#[test]
fn maestro_stdin_handle_lone_escape_character_with_timeout() {
    replay("maestro_stdin_handle_lone_escape_character_with_timeout");
}

#[test]
fn maestro_stdin_handle_lone_escape_character_with_explicit_flush() {
    replay("maestro_stdin_handle_lone_escape_character_with_explicit_flush");
}

#[test]
fn maestro_stdin_flush_incomplete_sequences() {
    replay("maestro_stdin_flush_incomplete_sequences");
}

#[test]
fn maestro_stdin_return_empty_array_if_nothing_to_flush() {
    replay("maestro_stdin_return_empty_array_if_nothing_to_flush");
}

#[test]
fn maestro_stdin_clear_buffered_content_without_emitting() {
    replay("maestro_stdin_clear_buffered_content_without_emitting");
}

#[test]
fn maestro_stdin_clear_buffer_on_destroy() {
    replay("maestro_stdin_clear_buffer_on_destroy");
}

#[test]
fn maestro_stdin_clear_pending_timeouts_on_destroy() {
    replay("maestro_stdin_clear_pending_timeouts_on_destroy");
}

#[test]
fn maestro_stdin_uses_the_configured_timeout() {
    replay("maestro_stdin_uses_the_configured_timeout");
}

#[test]
fn maestro_stdin_rearms_only_the_current_incomplete_fragment() {
    replay("maestro_stdin_rearms_only_the_current_incomplete_fragment");
}

#[test]
fn maestro_stdin_cancels_deadline_after_completion() {
    replay("maestro_stdin_cancels_deadline_after_completion");
}

#[test]
fn maestro_stdin_empty_input_rearms_a_pending_fragment() {
    replay("maestro_stdin_empty_input_rearms_a_pending_fragment");
}

#[test]
fn maestro_stdin_flush_returns_without_emitting_and_cancels_deadline() {
    replay("maestro_stdin_flush_returns_without_emitting_and_cancels_deadline");
}

#[test]
fn maestro_stdin_retains_split_ss3_and_meta_sequences() {
    replay("maestro_stdin_retains_split_ss3_and_meta_sequences");
}

#[test]
fn maestro_stdin_frames_string_controls_by_their_terminators() {
    replay("maestro_stdin_frames_string_controls_by_their_terminators");
}

#[test]
fn maestro_stdin_requires_three_ascii_mouse_coordinates() {
    replay("maestro_stdin_requires_three_ascii_mouse_coordinates");
}

#[test]
fn maestro_stdin_drop_raw_duplicate_character_after_matching_kitty_printable_sequence() {
    replay("maestro_stdin_drop_raw_duplicate_character_after_matching_kitty_printable_sequence");
}

#[test]
fn maestro_stdin_drop_raw_duplicate_character_after_matching_kitty_printable_sequence_across_chunks()
 {
    replay(
        "maestro_stdin_drop_raw_duplicate_character_after_matching_kitty_printable_sequence_across_chunks",
    );
}

#[test]
fn maestro_stdin_keep_non_matching_plain_character_after_kitty_printable_sequence() {
    replay("maestro_stdin_keep_non_matching_plain_character_after_kitty_printable_sequence");
}

#[test]
fn maestro_stdin_keep_raw_character_after_modified_kitty_printable_sequence() {
    replay("maestro_stdin_keep_raw_character_after_modified_kitty_printable_sequence");
}

#[test]
fn maestro_stdin_bounds_printable_duplicate_identity() {
    replay("maestro_stdin_bounds_printable_duplicate_identity");
}

#[test]
fn maestro_stdin_matches_only_unmodified_printable_reports() {
    replay("maestro_stdin_matches_only_unmodified_printable_reports");
}

#[test]
fn maestro_stdin_consumes_only_one_adjacent_duplicate() {
    replay("maestro_stdin_consumes_only_one_adjacent_duplicate");
}

#[test]
fn maestro_stdin_resets_duplicate_identity_on_intervening_data() {
    replay("maestro_stdin_resets_duplicate_identity_on_intervening_data");
}

#[test]
fn maestro_stdin_suppresses_duplicate_enhanced_emoji() {
    replay("maestro_stdin_suppresses_duplicate_enhanced_emoji");
}

#[test]
fn maestro_stdin_empty_flush_keeps_pending_printable_identity() {
    replay("maestro_stdin_empty_flush_keeps_pending_printable_identity");
}

#[test]
fn maestro_stdin_emit_paste_event_for_complete_bracketed_paste() {
    replay("maestro_stdin_emit_paste_event_for_complete_bracketed_paste");
}

#[test]
fn maestro_stdin_handle_paste_arriving_in_chunks() {
    replay("maestro_stdin_handle_paste_arriving_in_chunks");
}

#[test]
fn maestro_stdin_handle_paste_with_input_before_and_after() {
    replay("maestro_stdin_handle_paste_with_input_before_and_after");
}

#[test]
fn maestro_stdin_handle_paste_with_newlines() {
    replay("maestro_stdin_handle_paste_with_newlines");
}

#[test]
fn maestro_stdin_handle_paste_with_unicode() {
    replay("maestro_stdin_handle_paste_with_unicode");
}

#[test]
fn maestro_stdin_retains_each_split_paste_start() {
    replay("maestro_stdin_retains_each_split_paste_start");
}

#[test]
fn maestro_stdin_retains_each_split_paste_end() {
    replay("maestro_stdin_retains_each_split_paste_end");
}

#[test]
fn maestro_stdin_orders_adjacent_pastes_and_surrounding_data() {
    replay("maestro_stdin_orders_adjacent_pastes_and_surrounding_data");
}

#[test]
fn maestro_stdin_emits_empty_paste_as_one_event() {
    replay("maestro_stdin_emits_empty_paste_as_one_event");
}

#[test]
fn maestro_stdin_empty_input_does_not_escape_paste_mode() {
    replay("maestro_stdin_empty_input_does_not_escape_paste_mode");
}

#[test]
fn maestro_stdin_preserves_incomplete_data_before_paste() {
    replay("maestro_stdin_preserves_incomplete_data_before_paste");
}

#[test]
fn maestro_stdin_paste_waits_for_end_without_a_fragment_timeout() {
    replay("maestro_stdin_paste_waits_for_end_without_a_fragment_timeout");
}

#[test]
fn maestro_stdin_paste_treats_inner_start_as_content_and_first_end_as_boundary() {
    replay("maestro_stdin_paste_treats_inner_start_as_content_and_first_end_as_boundary");
}

#[test]
fn maestro_stdin_expired_paste_start_prefix_is_ordinary_data() {
    replay("maestro_stdin_expired_paste_start_prefix_is_ordinary_data");
}

#[test]
fn maestro_stdin_paste_boundaries_reset_printable_identity() {
    replay("maestro_stdin_paste_boundaries_reset_printable_identity");
}

#[test]
fn maestro_stdin_clear_and_destroy_reset_all_pending_state() {
    replay("maestro_stdin_clear_and_destroy_reset_all_pending_state");
}

#[test]
fn maestro_stdin_handle_buffer_input() {
    replay("maestro_stdin_handle_buffer_input");
}

#[test]
fn maestro_stdin_decodes_buffer_inputs_without_guessing_stream_encoding() {
    replay("maestro_stdin_decodes_buffer_inputs_without_guessing_stream_encoding");
}

#[test]
fn maestro_stdin_preserves_buffer_chunk_decoding() {
    replay("maestro_stdin_preserves_buffer_chunk_decoding");
}
