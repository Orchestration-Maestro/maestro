//! The reader agrees with events recorded from the `OpenAI` client library's own reader.

use serde::Deserialize;

use super::{ServerSentEvent, SseMessages};

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
                event: event.event,
                data: event.data,
            })
            .collect();
        assert_eq!(events, recorded, "{}", case.name);
    }
}
