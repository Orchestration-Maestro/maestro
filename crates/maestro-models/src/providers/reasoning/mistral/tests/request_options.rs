//! Model-selected reasoning settings.

#[test]
fn simple_options_select_model_reasoning() {
    let model = crate::get_model("mistral", "mistral-small-2603").unwrap();
    let options = crate::SimpleStreamOptions {
        reasoning: Some(crate::ThinkingLevel::Medium),
        ..Default::default()
    };
    let (raw, effort) = super::super::request::simple_options(&model, &options);
    assert_eq!(effort.as_deref(), Some("high"));
    assert!(raw.prompt_mode.is_none());
    super::request_corpus::corpus("simple_options_select_model_reasoning", 74);
}

#[test]
fn simple_options_keep_open_mapping_values() {
    super::request_corpus::corpus("simple_options_keep_open_mapping_values", 11);
}

#[test]
fn request_headers_keep_authored_layers() {
    super::request_corpus::corpus("request_headers_keep_authored_layers", 42);
}
