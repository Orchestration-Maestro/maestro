//! One authored registration carries both stream setup and OAuth operations.
#[test]
fn registered_guest_provider_keeps_streaming_and_oauth_together() {
    super::super::components::local(async {
        let bus = super::bus();
        let (_owner, runner, _) =
            super::super::retained_components::loaded_on(bus.for_extension()).await;
        for route in ["create-oauth", "catalog-oauth"] {
            super::register(&runner, route).await;
            let provider = maestro_models::get_oauth_provider("compiled-stream")
                .expect("OAuth from the same stream registration");
            assert_eq!(provider.id(), "compiled-stream");
            assert_eq!(provider.name(), " Guest account ");
            assert_eq!(provider.uses_callback_server(), Some(false));
            let credentials = serde_json::from_value(serde_json::json!({
                "access":"combined", "refresh":"r", "expires":0
            }))
            .unwrap();
            assert_eq!(
                provider.get_api_key(&credentials).unwrap(),
                "guest-computed:combined:r"
            );
            let refreshed = provider.refresh_token(credentials, None).await.unwrap();
            assert_eq!(refreshed.access, "combined-rotated");
            for simple in [false, true] {
                super::exercise(&runner, simple, "keep").await;
            }
        }
        maestro_models::unregister_api_providers("provider:compiled-stream");
        maestro_models::unregister_oauth_provider("compiled-stream");
        bus.clear();
    });
}
