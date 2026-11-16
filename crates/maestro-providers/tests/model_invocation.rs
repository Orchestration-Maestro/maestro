//! Protocol callback storage remains independent of interface registries.
#[cfg(test)]
mod tests {
    pub(crate) fn assert_retained_timestamp<K: maestro_models::StreamOptionsFamily>(
        callback: &maestro_models::StreamFunction<K>,
        expected: f64,
    ) -> crate::chat::TestResult {
        let stream = callback.call(
            model("separate"),
            context(),
            maestro_models::ProviderInvocation::Threaded(None, None),
        )?;
        assert_timestamp(&stream, expected);
        Ok(())
    }

    use crate::invocation::{assert_timestamp, context, model, provider};
    use maestro_models::{
        SharedHandle, cleanup_session_resources, register_session_resource_cleanup,
    };
    use std::sync::{Arc, Mutex};
    #[cfg(not(target_arch = "wasm32"))]
    fn assert_cloud_selection() -> crate::chat::TestResult {
        for (call, expected) in [
            (
                maestro_models::stream(model("bedrock-converse-stream"), context(), None)?,
                81.0_f64,
            ),
            (
                maestro_models::stream_simple(model("bedrock-converse-stream"), context(), None)?,
                82.0,
            ),
        ] {
            assert_timestamp(&call, expected);
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn maestro_platform_stores_keep_distinct_owners() -> crate::chat::TestResult {
        use maestro_models::records::api_registry::*;
        use maestro_models::{get_oauth_provider, reset_oauth_providers};
        use maestro_providers::BedrockProviderModule;
        let _serial = crate::bundled::registry();
        if !maestro_test_harness::selection().is_child() {
            let name = "maestro_platform_stores_keep_distinct_owners";
            return {
                let output = maestro_test_harness::TestProcess::current(name)?
                    .nocapture()
                    .env_clear()
                    .case("owners")
                    .output()?;
                assert!(output.status.success(), "{output:?}");
                Ok(())
            };
        }
        let visits = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&visits);
        let removal = register_session_resource_cleanup(SharedHandle::new(move |session| {
            recorded.lock().unwrap().push(session.map(str::to_owned));
            Ok(())
        }));
        register_api_provider(provider("separate", 71.0), None);
        let retained = get_api_provider("separate").unwrap();
        let cloud = provider("bedrock-converse-stream", 81.0);
        maestro_providers::set_bedrock_provider_module(BedrockProviderModule {
            stream_bedrock: cloud.stream,
            stream_simple_bedrock: cloud.stream_simple,
        });
        let built_in: maestro_models::OAuthProviderHandle =
            Arc::new(crate::named_account::Named("anthropic", "Anthropic"));
        maestro_models::install_built_in_oauth_providers([built_in]);
        reset_oauth_providers();
        let account = get_oauth_provider("anthropic").unwrap();
        assert_retained_timestamp(&retained.stream, 71.0)?;
        cleanup_session_resources(Some("before"))?;
        clear_api_providers();
        assert!(Arc::ptr_eq(
            &account,
            &get_oauth_provider("anthropic").unwrap()
        ));
        assert_retained_timestamp(&retained.stream_simple, 72.0)?;
        cleanup_session_resources(Some("cleared"))?;
        maestro_providers::reset_api_providers();
        assert!(Arc::ptr_eq(
            &account,
            &get_oauth_provider("anthropic").unwrap()
        ));
        assert_cloud_selection()?;
        removal.remove();
        cleanup_session_resources(Some("removed"))?;
        assert_eq!(
            *visits.lock().unwrap(),
            [Some("before".into()), Some("cleared".into())]
        );
        assert_retained_timestamp(&retained.stream, 71.0)?;
        Ok(())
    }
}
