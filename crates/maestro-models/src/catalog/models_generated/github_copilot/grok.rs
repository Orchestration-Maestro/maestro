// Generated model descriptor data.
use crate::{Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat};
pub(super) fn models_25() -> [(&'static str, Model); 1] {
    [("grok-code-fast-1", grok_code_fast_1())]
}
fn grok_code_fast_1() -> Model {
    Model {
        id: "grok-code-fast-1".into(),
        name: "Grok Code Fast 1".into(),
        api: "openai-completions".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 64_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(false),
                supports_reasoning_effort: Some(false),
                ..Default::default()
            },
        ))),
    }
}
