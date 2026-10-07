// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_232() -> [(&'static str, Model); 1] {
    [("stepfun/step-3.5-flash", stepfun_step_3_dot_5_flash())]
}
fn stepfun_step_3_dot_5_flash() -> Model {
    Model {
        id: "stepfun/step-3.5-flash".into(),
        name: "StepFun: Step 3.5 Flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}
