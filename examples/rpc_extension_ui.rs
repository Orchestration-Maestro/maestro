//! A terminal client for streamed RPC events and extension dialogs.

use serde_json::Value;
use std::io::{self, BufRead};

/// Reads one complete JSON event per physical line without splitting Unicode text.
fn read_events(reader: impl BufRead, mut receive: impl FnMut(Value)) -> io::Result<()> {
    for line in reader.lines() {
        if let Ok(event) = serde_json::from_str(&line?) {
            receive(event);
        }
    }
    Ok(())
}

/// Starts the opt-in terminal demonstration.
fn main() -> io::Result<()> {
    read_events(io::stdin().lock(), |_| {})
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Unicode line separator belongs to the JSON string, not to the wire framing.
    #[test]
    fn streamed_unicode_line_separator_arrives_as_one_event() {
        let wire = "{\"type\":\"message_update\",\"assistantMessageEvent\":{\"type\":\"text_delta\",\"delta\":\"first\u{2028}second\"}}\r\n";
        let mut events = Vec::new();
        read_events(wire.as_bytes(), |event| events.push(event)).unwrap();
        assert_eq!(
            events,
            vec![serde_json::json!({"type":"message_update",
            "assistantMessageEvent":{"type":"text_delta","delta":"first\u{2028}second"}})]
        );
    }
}
