//! Authored xAI additions.
use super::defaults::xai_base;
use crate::{Model, ModelCost};

/// Construct an authored model descriptor.
pub(super) fn xai_grok_3() -> Model {
    Model {
        id: "grok-3".into(),
        name: "Grok 3".into(),
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.75,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        ..xai_base()
    }
}

/// Construct an authored model descriptor.
pub(super) fn xai_grok_3_fast() -> Model {
    Model {
        id: "grok-3-fast".into(),
        name: "Grok 3 Fast".into(),
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 1.25,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        ..xai_base()
    }
}

/// Construct an authored model descriptor.
pub(super) fn xai_grok_code_fast_1() -> Model {
    Model {
        id: "grok-code-fast-1".into(),
        name: "Grok Code Fast 1".into(),
        ..xai_base()
    }
}
