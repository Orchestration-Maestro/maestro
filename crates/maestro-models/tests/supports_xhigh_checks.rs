use maestro_models::{
    ModelThinkingLevel::{High, Off, Xhigh},
    get_model, get_supported_thinking_levels,
};
fn supported(provider: &str, id: &str) -> Vec<maestro_models::ModelThinkingLevel> {
    let handle = get_model(provider, id).expect("catalog model");
    let model = handle.read().unwrap().clone();
    get_supported_thinking_levels(&model)
}
#[test]
fn anthropic_opus_46_lists_xhigh() {
    assert!(supported("anthropic", "claude-opus-4-6").contains(&Xhigh));
}
#[test]
fn anthropic_opus_47_lists_xhigh() {
    assert!(supported("anthropic", "claude-opus-4-7").contains(&Xhigh));
}
#[test]
fn anthropic_sonnet_45_omits_xhigh() {
    assert!(!supported("anthropic", "claude-sonnet-4-5").contains(&Xhigh));
}
#[test]
fn codex_gpt_54_and_55_list_xhigh() {
    for id in ["gpt-5.4", "gpt-5.5"] {
        assert!(supported("openai-codex", id).contains(&Xhigh));
    }
}
#[test]
fn deepseek_flash_lists_off_high_xhigh() {
    assert_eq!(
        supported("deepseek", "deepseek-v4-flash"),
        vec![Off, High, Xhigh]
    );
}
#[test]
fn opencode_go_flash_lists_off_high_xhigh() {
    assert_eq!(
        supported("opencode-go", "deepseek-v4-flash"),
        vec![Off, High, Xhigh]
    );
}
#[test]
fn openrouter_flash_lists_off_high_xhigh() {
    assert_eq!(
        supported("openrouter", "deepseek/deepseek-v4-flash"),
        vec![Off, High, Xhigh]
    );
}
#[test]
fn openrouter_opus_46_lists_xhigh() {
    assert!(supported("openrouter", "anthropic/claude-opus-4.6").contains(&Xhigh));
}
