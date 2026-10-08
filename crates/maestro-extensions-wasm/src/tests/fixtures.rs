//! Typed values of the generated records, instantiated once per binding family.
//!
//! The component host and the author side generate the same records separately, so each
//! family module imports its own `events` and `session` modules and expands
//! [`define_fixtures`] to build values against them.

/// Defines the fixture builders against the `events` and `session` modules in scope at the
/// call site.
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
    };
}
