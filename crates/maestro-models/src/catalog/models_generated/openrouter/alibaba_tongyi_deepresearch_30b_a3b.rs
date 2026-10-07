// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 1] {
    [(
        "alibaba/tongyi-deepresearch-30b-a3b",
        alibaba_tongyi_deepresearch_30b_a3b(),
    )]
}
fn alibaba_tongyi_deepresearch_30b_a3b() -> Model {
    Model {
        id: "alibaba/tongyi-deepresearch-30b-a3b".into(),
        name: "Tongyi DeepResearch 30B A3B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.09,
            output: 0.449_999_999_999_999_96,
            cache_read: 0.09,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}
