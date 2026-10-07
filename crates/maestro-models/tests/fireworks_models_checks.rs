use maestro_models::get_model;

#[test]
fn fireworks_kimi_descriptor_keeps_messages_metadata() {
    let handle = get_model("fireworks", "accounts/fireworks/models/kimi-k2p6").unwrap();
    let model = handle.read().unwrap();
    assert_eq!(model.api, "anthropic-messages");
    assert_eq!(model.provider, "fireworks");
    assert_eq!(model.base_url, "https://api.fireworks.ai/inference");
    assert!(model.reasoning);
    assert_eq!(model.input, ["text", "image"]);
    assert_eq!(model.context_window, 262000.0);
    assert_eq!(model.max_tokens, 262000.0);
    assert_eq!(
        [
            model.cost.input,
            model.cost.output,
            model.cost.cache_read,
            model.cost.cache_write
        ],
        [0.95, 4.0, 0.16, 0.0]
    );
}

#[test]
fn fireworks_turbo_router_keeps_messages_metadata() {
    let handle = get_model("fireworks", "accounts/fireworks/routers/kimi-k2p5-turbo").unwrap();
    let model = handle.read().unwrap();
    assert_eq!(model.api, "anthropic-messages");
    assert_eq!(model.base_url, "https://api.fireworks.ai/inference");
    assert_eq!(model.input, ["text", "image"]);
}

#[path = "support/environment_child.rs"]
mod environment_child;
#[test]
fn fireworks_key_is_reported_and_resolved() {
    if !environment_child::child() {
        environment_child::Fixture::new().run(
            "fireworks_key_is_reported_and_resolved",
            &[("FIREWORKS_API_KEY", "test-fireworks-key")],
        );
        return;
    }
    assert_eq!(
        maestro_models::find_env_keys("fireworks").unwrap(),
        Some(vec!["FIREWORKS_API_KEY".into()])
    );
    assert_eq!(
        maestro_models::get_env_api_key("fireworks").unwrap(),
        Some("test-fireworks-key".into())
    );
}
