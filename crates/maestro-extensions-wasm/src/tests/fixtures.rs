//! Typed values of the generated records, instantiated once per binding family.
//!
//! The component host and the author side generate the same records separately, so each
//! family module imports its own `events`, `guest` and `session` modules and expands
//! [`define_fixtures`] to build values against them and to describe what handlers answered.

/// Defines the fixture builders and the answer descriptions against the `events`, `guest` and
/// `session` modules in scope at the call site.
macro_rules! define_fixtures {
    () => {
        /// Compaction data for an agent that answered once.
        pub fn compact_data() -> events::SessionBeforeCompactEventData {
            events::SessionBeforeCompactEventData {
                preparation: session::CompactionPreparation {
                    first_kept_entry_id: "e2".to_owned(),
                    is_split_turn: false,
                    tokens_before: 1234.0,
                    previous_summary: None,
                },
                custom_instructions: None,
            }
        }

        /// An interactive input of `text`.
        pub fn input(text: &str) -> events::InputEvent {
            events::InputEvent {
                text: text.to_owned(),
                images: None,
                source: events::InputSource::Interactive,
            }
        }

        /// How a handler's answer to an input reads in the transcript.
        pub fn input_decision(
            decision: &Result<Option<events::ExtensionEventResult>, String>,
        ) -> String {
            match decision {
                Ok(Some(events::ExtensionEventResult::Input(
                    events::InputEventResult::Transform(replacement),
                ))) => format!("transform:{}", replacement.text),
                Ok(_) => "other".to_owned(),
                Err(message) => format!("error:{message}"),
            }
        }

        /// How a handler's answer to a compaction reads in the transcript.
        pub fn compact_decision(
            decision: &Result<Option<events::ExtensionEventResult>, String>,
        ) -> String {
            match decision {
                Ok(Some(events::ExtensionEventResult::SessionBeforeCompact(result))) => {
                    let summary = result
                        .compaction
                        .as_ref()
                        .map(|compaction| compaction.summary.as_str())
                        .unwrap_or_default();
                    format!(
                        "cancel={} summary={summary}",
                        result.cancel.unwrap_or_default()
                    )
                }
                Ok(_) => "other".to_owned(),
                Err(message) => format!("error:{message}"),
            }
        }

        /// How a handler that edits its event answered.
        fn edit_decision(decision: &guest::Decision) -> String {
            match decision {
                Ok(None) => "ok".to_owned(),
                Ok(Some(_)) => "answered".to_owned(),
                Err(message) => format!("error:{message}"),
            }
        }

        /// What the trimming handler decided and the input as it left it.
        pub fn trimmed(outcome: &guest::InputOutcome) -> String {
            let edited = outcome.event.as_ref().map(|event| event.text.as_str());
            format!("{} edited={edited:?}", edit_decision(&outcome.decision))
        }

        /// What the noting handler decided and the preparation as it left it.
        pub fn noted(outcome: &guest::SessionBeforeCompactOutcome) -> String {
            let edited = outcome
                .event
                .as_ref()
                .map(|event| event.preparation.previous_summary.as_deref());
            format!("{} edited={edited:?}", edit_decision(&outcome.decision))
        }
    };
}
