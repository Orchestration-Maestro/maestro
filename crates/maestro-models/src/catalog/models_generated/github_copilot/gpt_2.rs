// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models_24() -> [(&'static str, Model); 1] {
    [("gpt-5.5", gpt_5_dot_5())]
}
fn gpt_5_dot_5() -> Model {
    Model {
        id: "gpt-5.5".into(),
        name: "GPT-5.5".into(),
        api: "openai-responses".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: None,
    }
}
