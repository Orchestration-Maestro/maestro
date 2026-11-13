//! Recorded event results with immediate carriage-return dispatch.

use serde::Deserialize;

use super::{ServerSentEvent, SseMessages, SseOptions};

/// Chunked bodies with the events the client library delivered for them.
const CORPUS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/server_sent_events.json"
));

/// The recorded corpus.
#[derive(Deserialize)]
struct Corpus {
    /// Every recorded body.
    cases: Vec<Case>,
}

/// One recorded body.
#[derive(Deserialize)]
struct Case {
    /// What the body exercises.
    name: String,
    /// Body chunks as hexadecimal bytes.
    chunks: Vec<String>,
    /// Events delivered before the next chunk was requested, after each chunk.
    emitted: Vec<usize>,
    /// Every event delivered, in order.
    events: Vec<Recorded>,
}

/// An event as the client library delivered it.
#[derive(Deserialize)]
struct Recorded {
    /// Event name, if the event had one.
    event: Option<String>,
    /// Event data.
    data: String,
}

/// The bytes a recorded chunk stands for.
fn bytes(chunk: &str) -> Vec<u8> {
    (0..chunk.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&chunk[at..at + 2], 16).unwrap())
        .collect()
}

#[test]
fn every_recorded_body_gives_the_events_the_client_library_gave() {
    let corpus: Corpus = serde_json::from_str(CORPUS).unwrap();
    assert!(!corpus.cases.is_empty());
    for case in corpus.cases {
        assert_eq!(case.chunks.len(), case.emitted.len(), "{}", case.name);
        let mut messages = SseMessages::default();
        let mut events = Vec::new();
        for (chunk, delivered) in case.chunks.iter().zip(&case.emitted) {
            events.extend(messages.push(&bytes(chunk)));
            assert_eq!(
                events.len(),
                *delivered,
                "{}: events after a chunk",
                case.name
            );
        }
        events.extend(messages.finish());
        let recorded: Vec<ServerSentEvent> = case
            .events
            .into_iter()
            .map(|event| ServerSentEvent {
                is_data: true,
                event: event.event,
                data: event.data,
                raw: Vec::new(),
            })
            .collect();
        assert_eq!(events, recorded, "{}", case.name);
    }
}

/// The event of a data field `data` that no name or other line accompanies.
fn data_event(data: &str) -> ServerSentEvent {
    ServerSentEvent {
        is_data: true,
        event: None,
        data: data.to_owned(),
        raw: Vec::new(),
    }
}

#[test]
fn maestro_http_dispatches_carriage_terminated_events() {
    let endings = ["\n", "\r\n", "\r"];
    for line in endings {
        for blank in endings {
            let mut reader = SseMessages::default();
            let frame = format!("data: first{line}{blank}");
            if line == "\r" && blank == "\n" {
                assert!(reader.push(frame.as_bytes()).is_empty());
                assert_eq!(reader.push(b"\n"), vec![data_event("first")]);
                continue;
            }
            assert_eq!(
                reader.push(frame.as_bytes()),
                vec![data_event("first")],
                "{frame:?}"
            );
            assert!(reader.push(b"").is_empty());
        }
    }
    let mut reader = SseMessages::default();
    assert_eq!(reader.push(b"data: first\r\r"), vec![data_event("first")]);
    assert!(reader.push(b"\n").is_empty());
    assert_eq!(
        reader.push(b"data: second\r\n\r"),
        vec![data_event("second")]
    );
    assert!(reader.push(b"\n").is_empty());
    assert_eq!(reader.push(b"data: second\n\n"), vec![data_event("second")]);
    assert!(reader.finish().is_empty());
}

/// Options that give a reader the framing of a message stream.
const MESSAGE_STREAM: SseOptions = SseOptions {
    stream_start_mark: true,
    raw_lines: true,
    flush_pending: true,
};

#[test]
fn raw_lines_keep_every_nonblank_line_of_each_event() {
    let mut reader = SseMessages::with_options(MESSAGE_STREAM);
    let events = reader.push(b": note\nid: 7\nevent: ping\ndata: a\n\ndata: b\n\n");
    let raw: Vec<_> = events.iter().map(|event| event.raw.clone()).collect();
    assert_eq!(
        raw,
        [
            vec![": note", "id: 7", "event: ping", "data: a"],
            vec!["data: b"]
        ]
    );
    let mut plain = SseMessages::default();
    assert!(plain.push(b"data: a\n\n")[0].raw.is_empty());
}

#[test]
fn a_stream_start_mark_is_dropped_once_while_the_default_drops_every_line_mark() {
    let body = "\u{feff}data: a\n\u{feff}data: b\n\n";
    let mut once = SseMessages::with_options(MESSAGE_STREAM);
    assert_eq!(once.push(body.as_bytes())[0].data, "a");
    let mut every = SseMessages::default();
    assert_eq!(every.push(body.as_bytes())[0].data, "a\nb");
}

#[test]
fn the_end_of_the_body_dispatches_the_pending_event_only_when_asked() {
    let mut flushing = SseMessages::with_options(MESSAGE_STREAM);
    assert!(flushing.push(b"event: stop\ndata: z").is_empty());
    let events = flushing.finish();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event.as_deref(), Some("stop"));
    assert_eq!(events[0].data, "z");
    let mut plain = SseMessages::default();
    assert!(plain.push(b"event: stop\ndata: z").is_empty());
    assert!(plain.finish().is_empty());
}
