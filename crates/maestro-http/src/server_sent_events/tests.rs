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

/// One recorded body with the events each framing reads from it.
struct Framed {
    /// What the body exercises.
    name: &'static str,
    /// Body chunks in delivery order.
    chunks: &'static [&'static str],
    /// Events of the default field framing.
    fields: Vec<ServerSentEvent>,
    /// Events of the message-stream framing.
    message: Vec<ServerSentEvent>,
    /// Segments of the whole-segment framing.
    segments: Vec<ServerSentEvent>,
    /// Text the whole-segment framing holds when the body ends.
    tail: &'static str,
}

/// A field event named `event` with `data`, formed by the lines `raw` when they are kept.
fn kept(event: Option<&str>, data: &str, raw: &[&str]) -> ServerSentEvent {
    ServerSentEvent {
        is_data: true,
        event: event.map(str::to_owned),
        data: data.to_owned(),
        raw: raw.iter().map(|&line| line.to_owned()).collect(),
    }
}

/// A whole segment holding `data`, which is event data when `is_data`.
fn segment(is_data: bool, data: &str) -> ServerSentEvent {
    ServerSentEvent {
        is_data,
        event: None,
        data: data.to_owned(),
        raw: Vec::new(),
    }
}

/// Recorded bodies that differ in their marks and line endings.
fn line_bodies() -> Vec<Framed> {
    vec![
        Framed {
            name: "mark at the start",
            chunks: &["\u{feff}data: 1\n\n"],
            fields: vec![data_event("1")],
            message: vec![kept(None, "1", &["data: 1"])],
            segments: vec![segment(true, "1")],
            tail: "",
        },
        Framed {
            name: "mark in mid-stream",
            chunks: &["data: 1\n\n\u{feff}data: 2\n\n"],
            fields: vec![data_event("1"), data_event("2")],
            message: vec![kept(None, "1", &["data: 1"])],
            segments: vec![segment(true, "1"), segment(true, "2")],
            tail: "",
        },
        Framed {
            name: "line endings cut between their two bytes",
            chunks: &["data: 1\r", "\n\r", "\ndata: 2\r\n\r\n"],
            fields: vec![data_event("1"), data_event("2")],
            message: vec![kept(None, "1", &["data: 1"]), kept(None, "2", &["data: 2"])],
            segments: vec![segment(true, "1"), segment(true, "2")],
            tail: "",
        },
        Framed {
            name: "lone carriage returns",
            chunks: &["event: a\rdata: 1\r\r"],
            fields: vec![kept(Some("a"), "1", &[])],
            message: vec![kept(Some("a"), "1", &["event: a", "data: 1"])],
            segments: vec![segment(false, "event: a\rdata: 1")],
            tail: "",
        },
    ]
}

/// Recorded bodies that differ in their fields and in how they end.
fn field_bodies() -> Vec<Framed> {
    vec![
        Framed {
            name: "comment lines",
            chunks: &[": note\ndata: 1\n: more\n\n"],
            fields: vec![data_event("1")],
            message: vec![kept(None, "1", &[": note", "data: 1", ": more"])],
            segments: vec![segment(false, ": note\ndata: 1\n: more")],
            tail: "",
        },
        Framed {
            name: "data on several lines",
            chunks: &["data: 1\ndata: 2\n\n"],
            fields: vec![data_event("1\n2")],
            message: vec![kept(None, "1\n2", &["data: 1", "data: 2"])],
            segments: vec![segment(true, "1\ndata: 2")],
            tail: "",
        },
        Framed {
            name: "no blank line before the end",
            chunks: &["data: 1\n\ndata: 2"],
            fields: vec![data_event("1")],
            message: vec![kept(None, "1", &["data: 1"]), kept(None, "2", &["data: 2"])],
            segments: vec![segment(true, "1")],
            tail: "data: 2",
        },
        Framed {
            name: "event without data",
            chunks: &["event: ping\n\ndata: 1\n\n"],
            fields: vec![kept(Some("ping"), "", &[]), data_event("1")],
            message: vec![
                kept(Some("ping"), "", &["event: ping"]),
                kept(None, "1", &["data: 1"]),
            ],
            segments: vec![segment(false, "event: ping"), segment(true, "1")],
            tail: "",
        },
    ]
}

/// Every event `reader` gives for `chunks` and the end of the body, with the text it still holds.
fn read(mut reader: SseMessages, chunks: &[Vec<u8>]) -> (Vec<ServerSentEvent>, String) {
    let mut events: Vec<_> = chunks.iter().flat_map(|chunk| reader.push(chunk)).collect();
    events.extend(reader.finish());
    (events, reader.segment_tail())
}

#[test]
fn every_framing_gives_its_recorded_events_for_each_body() {
    for case in line_bodies().into_iter().chain(field_bodies()) {
        let delivered: Vec<Vec<u8>> = case
            .chunks
            .iter()
            .map(|chunk| chunk.as_bytes().to_vec())
            .collect();
        let bytewise: Vec<Vec<u8>> = delivered
            .concat()
            .into_iter()
            .map(|byte| vec![byte])
            .collect();
        for chunks in [&delivered, &bytewise] {
            let cut = chunks.len();

            let fields = read(SseMessages::default(), chunks);
            let message = read(SseMessages::with_options(MESSAGE_STREAM), chunks);
            let segments = read(SseMessages::data_segments(), chunks);

            assert_eq!(
                fields.0, case.fields,
                "{}: fields in {cut} chunks",
                case.name
            );
            assert_eq!(
                message.0, case.message,
                "{}: message in {cut} chunks",
                case.name
            );
            assert_eq!(
                segments.0, case.segments,
                "{}: segments in {cut} chunks",
                case.name
            );
            assert_eq!(segments.1, case.tail, "{}: tail in {cut} chunks", case.name);
        }
    }
}
