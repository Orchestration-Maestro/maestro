//! Exact retained bytes and bounded host reply windows.

use super::*;

/// Dark appearance report.
const DARK_REPORT: &[u8] = b"\x1b[?997;1n";
/// Light appearance report.
const LIGHT_REPORT: &[u8] = b"\x1b[?997;2n";

/// Drain through the real retained owner while preserving byte assertions.
fn drain_complete_input_bytes(buffer: &mut Vec<u8>) -> Vec<Vec<u8>> {
    let mut framer = RawInputByteFramer {
        buffer: std::mem::take(buffer),
        ..Default::default()
    };
    let chunks = framer.drain_available_chunks();
    *buffer = framer.buffer;
    chunks
}

/// Flush through the real retained owner without a test parser.
fn flush_incomplete_input_bytes(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
    let mut framer = RawInputByteFramer {
        buffer: std::mem::take(buffer),
        ..Default::default()
    };
    let mut chunks = framer.flush_timeout();
    *buffer = framer.buffer;
    chunks.pop()
}

#[test]
fn host_cell_size_report_parser_is_exact() {
    for bytes in [
        // Zero dimensions carry no usable cell size.
        b"\x1b[6;0;10t".as_slice(),
        b"\x1b[6;21;0t".as_slice(),
        // Missing or extra parameters.
        b"\x1b[6;21t".as_slice(),
        b"\x1b[6;21;10;3t".as_slice(),
        // Other XTWINOPS reports must not be mistaken for a cell size.
        b"\x1b[4;1610;777t".as_slice(),
        b"\x1b[8;37;161t".as_slice(),
        // Non-numeric parameters.
        b"\x1b[6;21;1-t".as_slice(),
    ] {
        assert!(host_cell_size(bytes).is_none(), "bytes: {bytes:?}");
    }
}

#[test]
fn host_split_color_scheme_timeout_does_not_swallow_legacy_alt_bracket() {
    let mut framer = RawInputByteFramer::default();

    assert!(framer.push(b"\x1b[").is_empty());
    assert_eq!(framer.flush_timeout(), vec![b"\x1b[".to_vec()]);
}

#[test]
fn host_input_byte_framer_discards_timed_out_split_color_scheme_report_tail() {
    let mut framer = RawInputByteFramer::default();

    assert!(framer.push(b"\x1b[?997;").is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert!(framer.push(b"1n").is_empty());
    assert_eq!(framer.push(b"a"), vec![b"a".to_vec()]);
    assert!(framer.flush_timeout().is_empty());
}

#[test]
fn host_non_macos_host_input_splits_lone_escape_from_arrow() {
    let mut framer = RawInputByteFramer::with_host_input_policy(false);

    assert_eq!(
        framer.push(b"\x1b\x1b[D"),
        vec![b"\x1b".to_vec(), b"\x1b[D".to_vec()]
    );
}

#[test]
fn host_macos_host_input_policy_preserves_legacy_doubled_escape_alt_arrow() {
    let mut framer = RawInputByteFramer::with_host_input_policy(true);

    assert_eq!(framer.push(b"\x1b\x1b[D"), vec![b"\x1b\x1b[D".to_vec()]);
}

#[test]
fn host_timed_out_sgr_mouse_discard_state_clears_at_quiescence() {
    let mut framer = RawInputByteFramer::default();

    assert!(framer.push(b"\x1b[<3").is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert_eq!(framer.push(b"M"), vec![b"M".to_vec()]);
}

#[test]
fn host_complete_utf8_char_before_incomplete_char_is_drained() {
    let mut buffer = "你".as_bytes().to_vec();
    buffer.push("好".as_bytes()[0]);

    let chunks = drain_complete_input_bytes(&mut buffer);

    assert_eq!(chunks, vec!["你".as_bytes().to_vec()]);
    assert_eq!(buffer, vec!["好".as_bytes()[0]]);
}

#[test]
fn host_incomplete_utf8_prefix_is_not_flushed_on_timeout() {
    let mut buffer = vec!["好".as_bytes()[0]];

    assert_eq!(flush_incomplete_input_bytes(&mut buffer), None);
    assert_eq!(buffer, vec!["好".as_bytes()[0]]);
}

#[test]
fn host_invalid_utf8_lead_byte_is_flushed_instead_of_buffered_forever() {
    let mut buffer = vec![0xC0];

    assert_eq!(flush_incomplete_input_bytes(&mut buffer), None);
    assert!(buffer.is_empty());
}

#[test]
fn host_complete_utf8_char_before_incomplete_char_survives_timeout_and_next_chunk() {
    let mut buffer = "你".as_bytes().to_vec();
    buffer.push("好".as_bytes()[0]);

    let chunks = drain_complete_input_bytes(&mut buffer);
    assert_eq!(chunks, vec!["你".as_bytes().to_vec()]);
    assert_eq!(flush_incomplete_input_bytes(&mut buffer), None);
    assert_eq!(buffer, vec!["好".as_bytes()[0]]);

    buffer.extend_from_slice(&"好".as_bytes()[1..]);
    let chunks = drain_complete_input_bytes(&mut buffer);
    assert_eq!(chunks, vec!["好".as_bytes().to_vec()]);
    assert!(buffer.is_empty());
}

#[test]
fn host_long_multilingual_voice_like_burst_drains_without_truncation() {
    let text = "你好，今天我们测试一段比较长的语音输入。こんにちは。안녕하세요.🙂".repeat(128);
    assert!(
        text.len() > 4096,
        "test input should exceed the client read buffer"
    );
    let mut buffer = text.as_bytes().to_vec();

    let chunks = drain_complete_input_bytes(&mut buffer);
    let rebuilt: Vec<u8> = chunks.into_iter().flatten().collect();

    assert!(buffer.is_empty());
    assert_eq!(rebuilt, text.as_bytes());
}

#[test]
fn host_long_multilingual_burst_survives_one_byte_chunks_and_timeouts() {
    let text = "中文かなカナ한글🙂，。".repeat(64);
    let mut buffer = Vec::new();
    let mut rebuilt = Vec::new();

    for byte in text.as_bytes() {
        buffer.push(*byte);
        for chunk in drain_complete_input_bytes(&mut buffer) {
            rebuilt.extend(chunk);
        }
        if !buffer.is_empty() {
            assert_eq!(flush_incomplete_input_bytes(&mut buffer), None);
        }
    }

    for chunk in drain_complete_input_bytes(&mut buffer) {
        rebuilt.extend(chunk);
    }

    assert!(buffer.is_empty());
    assert_eq!(rebuilt, text.as_bytes());
}

#[test]
fn host_drain_complete_input_bytes_keeps_split_default_background_response_buffered() {
    let mut buffer = b"\x1b]11;rgb:2828".to_vec();

    let chunks = drain_complete_input_bytes(&mut buffer);

    assert!(chunks.is_empty());
    assert_eq!(buffer, b"\x1b]11;rgb:2828");
}

#[test]
fn host_flush_incomplete_input_bytes_keeps_split_default_background_response_buffered() {
    let mut buffer = b"\x1b]11;rgb:2828".to_vec();

    let flushed = flush_incomplete_input_bytes(&mut buffer);

    assert!(flushed.is_none());
    assert_eq!(buffer, b"\x1b]11;rgb:2828");
}

#[test]
fn host_flush_incomplete_input_bytes_keeps_default_background_response_split_after_command() {
    let mut buffer = b"\x1b]11;".to_vec();

    let flushed = flush_incomplete_input_bytes(&mut buffer);

    assert!(flushed.is_none());
    assert_eq!(buffer, b"\x1b]11;");
}

#[test]
fn host_flush_incomplete_input_bytes_keeps_default_background_response_split_inside_st() {
    let mut buffer = b"\x1b]11;rgb:2828/2a2a/3636\x1b".to_vec();

    let flushed = flush_incomplete_input_bytes(&mut buffer);

    assert!(flushed.is_none());
    assert_eq!(buffer, b"\x1b]11;rgb:2828/2a2a/3636\x1b");
}

#[test]
fn host_drain_complete_input_bytes_keeps_bare_osc_introducer_buffered() {
    let mut buffer = b"\x1b]".to_vec();

    let chunks = drain_complete_input_bytes(&mut buffer);

    assert!(chunks.is_empty());
    assert_eq!(buffer, b"\x1b]");
}

#[test]
fn host_flush_incomplete_input_bytes_drops_bare_osc_introducer_after_timeout() {
    let mut buffer = b"\x1b]".to_vec();

    let flushed = flush_incomplete_input_bytes(&mut buffer);

    assert!(flushed.is_none());
    assert!(buffer.is_empty());
}

#[test]
fn host_flush_incomplete_input_bytes_drops_string_introducers_after_timeout() {
    for bytes in [
        b"\x1b]".as_slice(),
        b"\x1bP".as_slice(),
        b"\x1b_".as_slice(),
        b"\x1b^".as_slice(),
        b"\x1bX".as_slice(),
    ] {
        let mut buffer = bytes.to_vec();

        let flushed = flush_incomplete_input_bytes(&mut buffer);

        assert!(flushed.is_none(), "flushed {bytes:?}");
        assert!(buffer.is_empty(), "kept {bytes:?}");
    }
}

#[test]
fn host_input_byte_framer_discards_split_control_string_after_timeout() {
    let mut framer = RawInputByteFramer::default();

    assert!(framer.push(b"\x1b]").is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert!(framer.push(b"11;#123456\x07").is_empty());
    assert_eq!(framer.push(b"a"), vec![b"a".to_vec()]);
}

#[test]
fn host_input_byte_framer_keeps_discarding_tail_across_timeout() {
    let mut framer = RawInputByteFramer::default();

    assert!(framer.push(b"\x1b]").is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert!(framer.push(b"1").is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert!(framer.push(b"1;#123456\x07").is_empty());
    assert_eq!(framer.push(b"a"), vec![b"a".to_vec()]);
}

#[test]
fn host_input_byte_framer_releases_discard_on_implausible_tail() {
    let mut framer = RawInputByteFramer::default();

    assert!(framer.push(b"\x1b]").is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert!(framer.push(b"a").is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert_eq!(framer.push(b"b"), vec![b"b".to_vec()]);
}

#[test]
fn host_flush_incomplete_input_bytes_does_not_hold_non_osc_default_color_text() {
    let mut framer = RawInputByteFramer::default();

    let chunks = framer.push(b"11;rgb:2828");

    assert_eq!(chunks.len(), 11);
    assert!(framer.flush_timeout().is_empty());
}

#[test]
fn host_timed_out_host_cell_size_reply_fragments_do_not_leak() {
    for (prefix, tail) in [
        (b"\x1b[6".as_slice(), b";21;10t".as_slice()),
        (b"\x1b[6;".as_slice(), b"21;10t".as_slice()),
        (b"\x1b[6;21;".as_slice(), b"10t".as_slice()),
    ] {
        let mut framer = RawInputByteFramer::default();
        framer.host_cell_size_query_sent();

        assert!(framer.push(prefix).is_empty(), "prefix: {prefix:?}");
        assert!(framer.flush_timeout().is_empty(), "prefix: {prefix:?}");
        assert!(framer.push(tail).is_empty(), "tail: {tail:?}");
        assert_eq!(framer.push(b"a"), vec![b"a".to_vec()]);
    }
}

#[test]
fn host_split_host_cell_size_reply_after_csi_intro_gets_one_more_flush() {
    let mut framer = RawInputByteFramer::default();
    framer.host_cell_size_query_sent();

    assert!(framer.push(b"\x1b[").is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert_eq!(framer.push(b"6;21;10t"), vec![b"\x1b[6;21;10t".to_vec()]);

    let mut alt_bracket = RawInputByteFramer::default();
    alt_bracket.host_cell_size_query_sent();
    assert!(alt_bracket.push(b"\x1b[").is_empty());
    assert!(alt_bracket.flush_timeout().is_empty());
    assert_eq!(alt_bracket.flush_timeout(), vec![b"\x1b[".to_vec()]);
}

#[test]
fn host_malformed_host_reply_tail_preserves_following_input() {
    let mut framer = RawInputByteFramer::default();
    framer.host_cell_size_query_sent();

    assert!(framer.push(b"\x1b[6;21").is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert_eq!(
        framer.push(b";10xabc"),
        vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]
    );
}

#[test]
fn host_reply_tail_discard_is_bounded_across_pushes() {
    let mut framer = RawInputByteFramer::default();
    framer.host_cell_size_query_sent();

    assert!(framer.push(b"\x1b[6;21").is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert!(framer.push(&[b'1'; 64]).is_empty());
    assert_eq!(
        framer.push(&[b'2'; 67]),
        vec![b"2".to_vec(), b"2".to_vec(), b"2".to_vec()]
    );
}

#[test]
fn host_stops_holding_lone_escape_after_host_cell_size_reply_completes() {
    let mut framer = RawInputByteFramer::default();
    framer.host_cell_size_query_sent();

    assert_eq!(
        framer.push(b"\x1b[6;21;10t"),
        vec![b"\x1b[6;21;10t".to_vec()]
    );

    // Window closed: a later lone Escape flushes immediately.
    assert!(framer.push(b"\x1b").is_empty());
    assert_eq!(framer.flush_timeout(), vec![b"\x1b".to_vec()]);
}

#[test]
fn host_default_byte_framer_does_not_rearm_after_color_scheme_report() {
    let mut framer = RawInputByteFramer::default();

    assert_eq!(framer.push(DARK_REPORT), vec![DARK_REPORT.to_vec()]);
    assert!(framer.push(b"\x1b").is_empty());
    assert_eq!(framer.flush_timeout(), vec![b"\x1b".to_vec()]);
}

#[test]
fn host_opt_in_does_not_delay_plain_escape_without_color_scheme_report() {
    let mut framer = RawInputByteFramer::default();
    framer.enable_host_color_scheme_change_tracking();

    assert!(framer.push(b"\x1b").is_empty());
    assert_eq!(framer.flush_timeout(), vec![b"\x1b".to_vec()]);
}

#[test]
fn host_opted_in_byte_framer_rearms_after_outer_focus_gained() {
    let mut framer = RawInputByteFramer::default();
    framer.enable_host_color_scheme_change_tracking();
    framer.enable_host_appearance_query_on_focus();

    assert_eq!(framer.push(b"\x1b[I"), vec![b"\x1b[I".to_vec()]);
    assert!(framer.push(b"\x1b").is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert_eq!(framer.push(b"[?997;2n"), vec![LIGHT_REPORT.to_vec()]);
}

#[test]
fn host_opted_in_byte_framer_reassembles_appearance_reply_split_after_csi() {
    let mut framer = RawInputByteFramer::default();
    framer.enable_host_color_scheme_change_tracking();
    framer.enable_host_appearance_query_on_focus();

    assert_eq!(framer.push(b"\x1b[I"), vec![b"\x1b[I".to_vec()]);
    assert!(framer.push(b"\x1b[").is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert_eq!(framer.push(b"?997;2n"), vec![LIGHT_REPORT.to_vec()]);
}

#[test]
fn host_opted_in_byte_framer_reassembles_delayed_appearance_reply() {
    let mut framer = RawInputByteFramer::default();
    framer.enable_host_color_scheme_change_tracking();
    framer.enable_host_appearance_query_on_focus();

    assert_eq!(framer.push(b"\x1b[I"), vec![b"\x1b[I".to_vec()]);
    assert!(framer.push(b"\x1b[?997;").is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert_eq!(framer.push(b"2n"), vec![LIGHT_REPORT.to_vec()]);
}

#[test]
fn host_timed_out_appearance_reply_preserves_pending_color_reply_window() {
    let mut framer = RawInputByteFramer::default();
    framer.host_color_query_sent();
    framer.enable_host_appearance_query_on_focus();

    assert_eq!(framer.push(b"\x1b[I"), vec![b"\x1b[I".to_vec()]);
    assert!(framer.push(b"\x1b[?997;").is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert!(framer.push(b"2n").is_empty());

    assert!(framer.push(b"\x1b").is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert_eq!(
        framer.push(b"]10;rgb:aaaa/bbbb/cccc\x1b\\"),
        vec![b"\x1b]10;rgb:aaaa/bbbb/cccc\x1b\\".to_vec()]
    );
}

#[test]
fn host_disabled_focus_query_does_not_rearm_byte_framer() {
    let mut framer = RawInputByteFramer::default();
    framer.enable_host_color_scheme_change_tracking();

    assert_eq!(framer.push(b"\x1b[I"), vec![b"\x1b[I".to_vec()]);
    assert!(framer.push(b"\x1b").is_empty());
    assert_eq!(framer.flush_timeout(), vec![b"\x1b".to_vec()]);
}

#[test]
fn host_focus_query_policy_does_not_delay_plain_escape_without_focus() {
    let mut framer = RawInputByteFramer::default();
    framer.enable_host_appearance_query_on_focus();

    assert!(framer.push(b"\x1b").is_empty());
    assert_eq!(framer.flush_timeout(), vec![b"\x1b".to_vec()]);
}

#[test]
fn host_focus_query_without_reply_holds_escape_for_only_one_flush() {
    let mut framer = RawInputByteFramer::default();
    framer.enable_host_appearance_query_on_focus();

    assert_eq!(framer.push(b"\x1b[I"), vec![b"\x1b[I".to_vec()]);
    assert!(framer.push(b"\x1b").is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert_eq!(framer.flush_timeout(), vec![b"\x1b".to_vec()]);
}

#[test]
fn host_flushes_lone_escape_when_not_awaiting_host_color_reply() {
    let mut framer = RawInputByteFramer::default();

    assert!(framer.push(b"\x1b").is_empty());
    assert_eq!(framer.flush_timeout(), vec![b"\x1b".to_vec()]);
}

#[test]
fn host_gives_up_holding_lone_escape_after_one_idle_flush() {
    let mut framer = RawInputByteFramer::default();
    framer.host_color_query_sent();

    assert!(framer.push(b"\x1b").is_empty());
    // First idle flush holds the escape.
    assert!(framer.flush_timeout().is_empty());
    // No continuation arrived; the second idle flush releases it as Escape.
    assert_eq!(framer.flush_timeout(), vec![b"\x1b".to_vec()]);
}

#[test]
fn host_stops_holding_lone_escape_after_host_color_reply_completes() {
    use std::fmt::Write as _;

    let mut framer = RawInputByteFramer::default();
    framer.host_color_query_sent();
    let mut replies =
        String::from("\x1b]10;rgb:6565/7b7b/8383\x1b\\\x1b]11;rgb:2424/2727/3a3a\x1b\\");
    for index in 0..=u8::MAX {
        let _ = write!(replies, "\x1b]4;{index};rgb:1111/2222/3333\x1b\\");
    }

    let chunks = framer.push(replies.as_bytes());
    assert_eq!(chunks.len(), 258);

    // Window closed: a later lone Escape flushes immediately.
    assert!(framer.push(b"\x1b").is_empty());
    assert_eq!(framer.flush_timeout(), vec![b"\x1b".to_vec()]);
}

#[test]
fn host_flushes_lone_escape_after_timeout_pending_bytes() {
    let mut framer = RawInputByteFramer::default();
    framer.push(&[27]);
    framer.flush_timeout();
    assert!(framer.buffer.is_empty());
}

#[test]
fn host_chunked_legacy_arrow_waits_for_completion_pending_bytes() {
    let mut framer = RawInputByteFramer::default();

    framer.push(b"\x1b");
    assert_eq!(framer.buffer, b"\x1b");

    framer.push(b"[A");
    assert!(framer.buffer.is_empty());
}

#[test]
fn host_lone_escape_is_buffered_until_timeout_flush_pending_bytes() {
    let mut framer = RawInputByteFramer::default();

    framer.push(b"\x1b");
    assert_eq!(framer.buffer, b"\x1b");

    framer.flush_timeout();
    assert!(framer.buffer.is_empty());
}

#[test]
fn host_escape_followed_by_arrow_before_flush_does_not_emit_escape_pending_bytes() {
    let mut framer = RawInputByteFramer::default();

    framer.push(b"\x1b");
    assert_eq!(framer.buffer, b"\x1b");

    framer.push(b"[B");
    assert!(framer.buffer.is_empty());
}

#[test]
fn host_escape_followed_by_alt_char_before_flush_becomes_alt_key_pending_bytes() {
    let mut framer = RawInputByteFramer::default();

    framer.push(b"\x1b");
    assert_eq!(framer.buffer, b"\x1b");

    framer.push(b"b");
    assert!(framer.buffer.is_empty());
}

#[test]
fn host_chunked_extended_sequence_waits_for_completion_pending_bytes() {
    let mut framer = RawInputByteFramer::default();

    framer.push(b"\x1b[49:33;2:");
    assert_eq!(framer.buffer, b"\x1b[49:33;2:");

    framer.push(b"1u");
    assert!(framer.buffer.is_empty());
}

#[test]
fn host_chunked_bracketed_paste_waits_for_terminator_pending_bytes() {
    let mut framer = RawInputByteFramer::default();

    framer.push(b"\x1b[200~hello");
    assert_eq!(framer.buffer, b"\x1b[200~hello");

    framer.push(b"\x1b[201~");
    assert!(framer.buffer.is_empty());
}

#[test]
fn host_incomplete_bracketed_paste_is_not_flushed_on_timeout_pending_bytes() {
    let mut framer = RawInputByteFramer::default();

    framer.push(b"\x1b[200~hello\nworld");
    assert_eq!(framer.buffer, b"\x1b[200~hello\nworld");

    framer.flush_timeout();
    assert_eq!(framer.buffer, b"\x1b[200~hello\nworld");

    framer.push(b"\x1b[201~");
    assert!(framer.buffer.is_empty());
}

#[test]
fn host_chunked_alt_utf8_waits_for_continuation_byte_after_escape_pending_bytes() {
    let mut framer = RawInputByteFramer::default();
    let bytes = "\x1bé".as_bytes();

    framer.push(&bytes[..2]);
    assert_eq!(framer.buffer, bytes[..2]);
    framer.flush_timeout();
    assert_eq!(framer.buffer, bytes[..2]);

    framer.push(&bytes[2..]);
    assert!(framer.buffer.is_empty());
}

#[test]
fn host_chunked_utf8_waits_for_continuation_byte_pending_bytes() {
    let mut framer = RawInputByteFramer::default();

    framer.push("é".as_bytes().get(..1).unwrap());
    assert_eq!(framer.buffer, vec![0xC3]);

    framer.push("é".as_bytes().get(1..).unwrap());
    assert!(framer.buffer.is_empty());
}

#[test]
fn host_chunked_cjk_utf8_waits_for_all_continuation_bytes_pending_bytes() {
    let mut framer = RawInputByteFramer::default();
    let bytes = "好".as_bytes();

    framer.push(&bytes[..1]);
    assert_eq!(framer.buffer, bytes[..1]);
    framer.flush_timeout();
    assert_eq!(framer.buffer, bytes[..1]);

    framer.push(&bytes[1..2]);
    assert_eq!(framer.buffer, bytes[..2]);
    framer.flush_timeout();
    assert_eq!(framer.buffer, bytes[..2]);

    framer.push(&bytes[2..]);
    assert!(framer.buffer.is_empty());
}

#[test]
fn host_chunked_four_byte_utf8_waits_for_all_continuation_bytes_pending_bytes() {
    let mut framer = RawInputByteFramer::default();
    let bytes = "🙂".as_bytes();

    for split in 1..bytes.len() {
        framer.push(&bytes[split - 1..split]);
        assert_eq!(framer.buffer, bytes[..split]);
        framer.flush_timeout();
        assert_eq!(framer.buffer, bytes[..split]);
    }

    framer.push(&bytes[bytes.len() - 1..]);
    assert!(framer.buffer.is_empty());
}
