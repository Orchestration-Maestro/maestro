//! Producer gates expose live tool arguments before the next input is released.

use super::super::Progress;
use super::{AssistantMessageEvent, Cancellation, TestResult, Wire, framed, messages};
use maestro_models::AssistantContent;
use serde_json::Value;
use std::collections::VecDeque;

/// One existing body feed with its recorded argument observations.
pub(super) struct Gate<'a> {
    /// Pending read and body release witnesses.
    feed: Option<messages::Feed>,
    /// Expected deltas in source order.
    progress: &'a [Progress],
    /// Number of observed deltas.
    observed: usize,
    /// Next segment, ending at a delta or terminal.
    chunks: VecDeque<Vec<u8>>,
    /// Body release after the final sender has been closed.
    released: Option<tokio::sync::oneshot::Receiver<()>>,
}

impl<'a> Gate<'a> {
    /// Feed only through the first delta, leaving later fragments behind the public witness.
    pub(super) fn new(
        wire: &Wire,
        progress: Option<&'a [Progress]>,
        feed: Option<messages::Feed>,
    ) -> TestResult<Self> {
        let mut chunks = VecDeque::new();
        if let Some(progress) = progress {
            let events = wire.events.as_ref().ok_or("gated events")?;
            let mut start = 0;
            for expected in progress {
                let end = events[start..]
                    .iter()
                    .position(|event| delta(event) == Some(expected.delta.as_str()))
                    .ok_or("delta frame")?
                    + start
                    + 1;
                chunks.push_back(framed(wire, &events[start..end]));
                start = end;
            }
            chunks.push_back(framed(wire, &events[start..]));
            feed.as_ref()
                .ok_or("gated feed")?
                .chunks
                .send(chunks.pop_front().ok_or("first segment")?)?;
        }
        Ok(Self {
            feed,
            progress: progress.unwrap_or_default(),
            observed: 0,
            chunks,
            released: None,
        })
    }

    /// Check the live call before admitting the next frame; cancellation keeps its prior witness.
    pub(super) async fn observe(
        &mut self,
        event: &AssistantMessageEvent,
        signal: &Cancellation,
    ) -> TestResult {
        if let AssistantMessageEvent::ToolcallDelta {
            content_index,
            delta,
            partial,
        } = event
            && !self.progress.is_empty()
        {
            let expected = self.progress.get(self.observed).ok_or("extra tool delta")?;
            assert_eq!(delta, &expected.delta, "gated delta");
            assert_eq!(*content_index, expected.index, "gated content index");
            live_call(partial, *content_index, expected)?;
            let feed = self.feed.as_mut().ok_or("gated feed")?;
            feed.read_pending().await?;
            feed.chunks
                .send(self.chunks.pop_front().ok_or("next segment")?)?;
            self.observed += 1;
            if self.observed == self.progress.len() {
                let feed = self.feed.take().ok_or("final feed")?;
                drop(feed.chunks);
                self.released = Some(feed.released);
            }
        } else if self.progress.is_empty()
            && matches!(event, AssistantMessageEvent::TextDelta { delta, .. } if delta.len() > 50)
            && let Some(feed) = self.feed.as_mut()
        {
            feed.read_pending().await?;
            signal.abort();
        }
        Ok(())
    }

    /// Confirm every recorded gate fired and the finite producer was released.
    pub(super) async fn finish(self) -> TestResult {
        assert_eq!(
            self.observed,
            self.progress.len(),
            "argument observation count"
        );
        assert!(self.chunks.is_empty(), "all gated segments consumed");
        if let Some(feed) = self.feed {
            feed.released.await?;
        }
        if let Some(released) = self.released {
            released.await?;
        }
        Ok(())
    }
}

/// Protocol-specific argument fragment, before any application parsing.
fn delta(event: &Value) -> Option<&str> {
    event["delta"]
        .as_str()
        .or_else(|| event["delta"]["partial_json"].as_str())
        .or_else(|| event["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"].as_str())
}

/// Read the same retained message while the producer is still behind its gate.
fn live_call(
    partial: &maestro_models::SharedAssistantMessage,
    index: usize,
    expected: &Progress,
) -> TestResult {
    let message = partial.read().map_err(|_| "partial poisoned")?;
    let AssistantContent::ToolCall(call) = &message.content[index] else {
        return Err("partial call".into());
    };
    assert_eq!(call.id, expected.id, "gated call identity");
    assert_eq!(call.name, expected.name, "gated tool name");
    assert_eq!(
        serde_json::to_value(&call.arguments)?,
        expected.arguments,
        "gated partial arguments"
    );
    Ok(())
}
