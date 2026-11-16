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
fn stdin_pass_through_regular_characters_immediately() {
    replay("stdin_pass_through_regular_characters_immediately");
}

#[test]
fn stdin_pass_through_multiple_regular_characters() {
    replay("stdin_pass_through_multiple_regular_characters");
}

#[test]
fn stdin_handle_unicode_characters() {
    replay("stdin_handle_unicode_characters");
}

#[test]
fn stdin_keeps_supplementary_characters_whole() {
    replay("stdin_keeps_supplementary_characters_whole");
}

#[test]
fn stdin_preserves_whitespace_and_control_scalars() {
    replay("stdin_preserves_whitespace_and_control_scalars");
}

#[test]
fn stdin_handle_empty_input() {
    replay("stdin_handle_empty_input");
}

#[test]
fn stdin_complete_sgr_mouse_sequences_reach_the_handler() {
    replay("stdin_complete_sgr_mouse_sequences_reach_the_handler");
}

#[test]
fn stdin_complete_arrow_sequences_reach_the_handler() {
    replay("stdin_complete_arrow_sequences_reach_the_handler");
}

#[test]
fn stdin_complete_function_key_sequences_reach_the_handler() {
    replay("stdin_complete_function_key_sequences_reach_the_handler");
}

#[test]
fn stdin_pass_through_meta_key_sequences() {
    replay("stdin_pass_through_meta_key_sequences");
}

#[test]
fn stdin_pass_through_ss3_sequences() {
    replay("stdin_pass_through_ss3_sequences");
}

#[test]
fn stdin_characters_before_an_escape_sequence_are_read() {
    replay("stdin_characters_before_an_escape_sequence_are_read");
}

#[test]
fn stdin_characters_after_an_escape_sequence_are_read() {
    replay("stdin_characters_after_an_escape_sequence_are_read");
}

#[test]
fn stdin_handle_multiple_complete_sequences() {
    replay("stdin_handle_multiple_complete_sequences");
}

#[test]
fn stdin_kitty_csi_u_press_events_are_read() {
    replay("stdin_kitty_csi_u_press_events_are_read");
}

#[test]
fn stdin_kitty_csi_u_release_events_are_read() {
    replay("stdin_kitty_csi_u_release_events_are_read");
}

#[test]
fn stdin_a_batched_kitty_press_and_release_is_read() {
    replay("stdin_a_batched_kitty_press_and_release_is_read");
}

#[test]
fn stdin_handle_multiple_batched_kitty_events() {
    replay("stdin_handle_multiple_batched_kitty_events");
}

#[test]
fn stdin_kitty_arrow_keys_keep_their_event_type() {
    replay("stdin_kitty_arrow_keys_keep_their_event_type");
}

#[test]
fn stdin_kitty_function_keys_keep_their_event_type() {
    replay("stdin_kitty_function_keys_keep_their_event_type");
}

#[test]
fn stdin_plain_characters_and_kitty_sequences_mix() {
    replay("stdin_plain_characters_and_kitty_sequences_mix");
}

#[test]
fn stdin_fast_typing_keeps_kitty_order() {
    replay("stdin_fast_typing_keeps_kitty_order");
}

#[test]
fn stdin_handle_mouse_press_event() {
    replay("stdin_handle_mouse_press_event");
}

#[test]
fn stdin_handle_mouse_release_event() {
    replay("stdin_handle_mouse_release_event");
}

#[test]
fn stdin_handle_multiple_mouse_events() {
    replay("stdin_handle_multiple_mouse_events");
}

#[test]
fn stdin_an_old_style_mouse_report_is_read() {
    replay("stdin_an_old_style_mouse_report_is_read");
}

#[test]
fn stdin_handle_very_long_sequences() {
    replay("stdin_handle_very_long_sequences");
}

#[test]
fn stdin_counts_complete_scalars_in_legacy_mouse_reports() {
    replay("stdin_counts_complete_scalars_in_legacy_mouse_reports");
}

#[test]
fn stdin_frames_meta_ss3_and_legacy_mouse() {
    replay("stdin_frames_meta_ss3_and_legacy_mouse");
}

#[test]
fn stdin_recognizes_csi_final_byte_boundaries() {
    replay("stdin_recognizes_csi_final_byte_boundaries");
}

#[test]
fn stdin_buffer_incomplete_mouse_sgr_sequence() {
    replay("stdin_buffer_incomplete_mouse_sgr_sequence");
}

#[test]
fn stdin_buffer_incomplete_csi_sequence() {
    replay("stdin_buffer_incomplete_csi_sequence");
}

#[test]
fn stdin_buffer_split_across_many_chunks() {
    replay("stdin_buffer_split_across_many_chunks");
}

#[test]
fn stdin_flush_incomplete_sequence_after_timeout() {
    replay("stdin_flush_incomplete_sequence_after_timeout");
}

#[test]
fn stdin_a_partial_sequence_after_characters_is_buffered() {
    replay("stdin_a_partial_sequence_after_characters_is_buffered");
}

#[test]
fn stdin_a_partial_old_style_mouse_report_waits() {
    replay("stdin_a_partial_old_style_mouse_report_waits");
}

#[test]
fn stdin_handle_split_mouse_events() {
    replay("stdin_handle_split_mouse_events");
}

#[test]
fn stdin_the_timeout_reads_a_lone_escape() {
    replay("stdin_the_timeout_reads_a_lone_escape");
}

#[test]
fn stdin_an_explicit_flush_reads_a_lone_escape() {
    replay("stdin_an_explicit_flush_reads_a_lone_escape");
}

#[test]
fn stdin_flush_incomplete_sequences() {
    replay("stdin_flush_incomplete_sequences");
}

#[test]
fn stdin_a_flush_with_nothing_pending_returns_empty() {
    replay("stdin_a_flush_with_nothing_pending_returns_empty");
}

#[test]
fn stdin_clear_buffered_content_without_emitting() {
    replay("stdin_clear_buffered_content_without_emitting");
}

#[test]
fn stdin_clear_buffer_on_destroy() {
    replay("stdin_clear_buffer_on_destroy");
}

#[test]
fn stdin_clear_pending_timeouts_on_destroy() {
    replay("stdin_clear_pending_timeouts_on_destroy");
}

#[test]
fn stdin_uses_the_configured_timeout() {
    replay("stdin_uses_the_configured_timeout");
}

#[test]
fn stdin_rearms_only_the_current_incomplete_fragment() {
    replay("stdin_rearms_only_the_current_incomplete_fragment");
}

#[test]
fn stdin_cancels_deadline_after_completion() {
    replay("stdin_cancels_deadline_after_completion");
}

#[test]
fn stdin_empty_input_rearms_a_pending_fragment() {
    replay("stdin_empty_input_rearms_a_pending_fragment");
}

#[test]
fn stdin_flush_returns_without_emitting_and_cancels_deadline() {
    replay("stdin_flush_returns_without_emitting_and_cancels_deadline");
}

#[test]
fn stdin_retains_split_ss3_and_meta_sequences() {
    replay("stdin_retains_split_ss3_and_meta_sequences");
}

#[test]
fn stdin_frames_string_controls_by_their_terminators() {
    replay("stdin_frames_string_controls_by_their_terminators");
}

#[test]
fn stdin_requires_three_ascii_mouse_coordinates() {
    replay("stdin_requires_three_ascii_mouse_coordinates");
}

#[test]
fn stdin_a_duplicate_scalar_after_a_kitty_printable_is_dropped() {
    replay("stdin_a_duplicate_scalar_after_a_kitty_printable_is_dropped");
}

#[test]
fn stdin_a_duplicate_scalar_split_across_chunks_is_dropped() {
    replay("stdin_a_duplicate_scalar_split_across_chunks_is_dropped");
}

#[test]
fn stdin_a_different_character_after_a_kitty_printable_is_kept() {
    replay("stdin_a_different_character_after_a_kitty_printable_is_kept");
}

#[test]
fn stdin_the_raw_character_after_a_modified_kitty_printable_is_kept() {
    replay("stdin_the_raw_character_after_a_modified_kitty_printable_is_kept");
}

#[test]
fn stdin_bounds_printable_duplicate_identity() {
    replay("stdin_bounds_printable_duplicate_identity");
}

#[test]
fn stdin_matches_only_unmodified_printable_reports() {
    replay("stdin_matches_only_unmodified_printable_reports");
}

#[test]
fn stdin_consumes_only_one_adjacent_duplicate() {
    replay("stdin_consumes_only_one_adjacent_duplicate");
}

#[test]
fn stdin_resets_duplicate_identity_on_intervening_data() {
    replay("stdin_resets_duplicate_identity_on_intervening_data");
}

#[test]
fn stdin_suppresses_duplicate_enhanced_emoji() {
    replay("stdin_suppresses_duplicate_enhanced_emoji");
}

#[test]
fn stdin_empty_flush_keeps_pending_printable_identity() {
    replay("stdin_empty_flush_keeps_pending_printable_identity");
}

#[test]
fn stdin_a_complete_bracketed_paste_emits_one_event() {
    replay("stdin_a_complete_bracketed_paste_emits_one_event");
}

#[test]
fn stdin_handle_paste_arriving_in_chunks() {
    replay("stdin_handle_paste_arriving_in_chunks");
}

#[test]
fn stdin_handle_paste_with_input_before_and_after() {
    replay("stdin_handle_paste_with_input_before_and_after");
}

#[test]
fn stdin_handle_paste_with_newlines() {
    replay("stdin_handle_paste_with_newlines");
}

#[test]
fn stdin_handle_paste_with_unicode() {
    replay("stdin_handle_paste_with_unicode");
}

#[test]
fn stdin_retains_each_split_paste_start() {
    replay("stdin_retains_each_split_paste_start");
}

#[test]
fn stdin_retains_each_split_paste_end() {
    replay("stdin_retains_each_split_paste_end");
}

#[test]
fn stdin_orders_adjacent_pastes_and_surrounding_data() {
    replay("stdin_orders_adjacent_pastes_and_surrounding_data");
}

#[test]
fn stdin_emits_empty_paste_as_one_event() {
    replay("stdin_emits_empty_paste_as_one_event");
}

#[test]
fn stdin_empty_input_does_not_escape_paste_mode() {
    replay("stdin_empty_input_does_not_escape_paste_mode");
}

#[test]
fn stdin_preserves_incomplete_data_before_paste() {
    replay("stdin_preserves_incomplete_data_before_paste");
}

#[test]
fn stdin_paste_waits_for_end_without_a_fragment_timeout() {
    replay("stdin_paste_waits_for_end_without_a_fragment_timeout");
}

#[test]
fn stdin_paste_treats_inner_start_as_content_and_first_end_as_boundary() {
    replay("stdin_paste_treats_inner_start_as_content_and_first_end_as_boundary");
}

#[test]
fn stdin_expired_paste_start_prefix_is_ordinary_data() {
    replay("stdin_expired_paste_start_prefix_is_ordinary_data");
}

#[test]
fn stdin_paste_boundaries_reset_printable_identity() {
    replay("stdin_paste_boundaries_reset_printable_identity");
}

#[test]
fn stdin_clear_and_destroy_reset_all_pending_state() {
    replay("stdin_clear_and_destroy_reset_all_pending_state");
}

#[test]
fn stdin_handle_buffer_input() {
    replay("stdin_handle_buffer_input");
}

#[test]
fn stdin_decodes_buffer_inputs_without_guessing_stream_encoding() {
    replay("stdin_decodes_buffer_inputs_without_guessing_stream_encoding");
}

#[test]
fn stdin_preserves_buffer_chunk_decoding() {
    replay("stdin_preserves_buffer_chunk_decoding");
}

#[test]
fn stdin_holds_a_fragment_when_the_deadline_is_not_representable() {
    let mut buffer = StdinBuffer::new(StdinBufferOptions {
        timeout: Duration::MAX,
    });
    let now = Instant::now();
    assert!(
        buffer
            .process(StdinBufferInput::Text("\x1b["), now)
            .is_empty()
    );
    assert_eq!(buffer.get_buffer(), "\x1b[");
    assert_eq!(buffer.deadline(), None);
    assert!(buffer.expire(now + Duration::from_secs(3600)).is_empty());
    assert_eq!(buffer.flush(), ["\x1b["]);
}
