#[cfg(test)]
mod tests {
    use maestro_models::records::api_registry::{
        ApiProvider, register_api_provider, unregister_api_providers,
    };
    use maestro_models::records::invocation::{stream, stream_simple};
    use maestro_models::{AssistantMessageEventStream, Context, Model, SharedHandle};
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};

    #[test]
    fn custom_callbacks_receive_complete_compatibility() {
        let _serial = crate::bundled::registry();
        let expected = json!({"supportsStore":false,"sendSessionIdHeader":true,"open":{"order":["b","a","b"],"null":null}});
        let model: Model = serde_json::from_value(json!({"id":"m","name":"M","api":"compatibility-custom","provider":"custom","baseUrl":"https://fixture.invalid","reasoning":false,"input":["text"],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":100,"maxTokens":10,"compat":expected})).unwrap();
        let observed = Arc::new(Mutex::new(Vec::<Value>::new()));
        register_api_provider(
            ApiProvider {
                api: model.api.clone(),
                stream: SharedHandle::new(ObserveCompatibility(Arc::clone(&observed))),
                stream_simple: SharedHandle::new(ObserveCompatibility(Arc::clone(&observed))),
            },
            Some("compatibility-observation".into()),
        );
        let context = Context {
            system_prompt: None,
            messages: vec![],
            tools: None,
        };
        stream(model.clone(), context.clone(), None).unwrap();
        stream_simple(model, context, None).unwrap();
        unregister_api_providers("compatibility-observation");
        assert_eq!(*observed.lock().unwrap(), vec![expected.clone(), expected]);
    }

    /// Retained invocation owner for the controlled provider.
    struct ObserveCompatibility(Arc<Mutex<Vec<Value>>>);
    impl<E: maestro_models::ProviderExecution, O>
        maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for ObserveCompatibility
    {
        type Return = Result<
            maestro_models::AssistantMessageEventStream,
            maestro_models::DiagnosticErrorInfo,
        >;
        fn invoke(&self, call: maestro_models::ProviderCall<E, O>) -> Self::Return {
            let value = serde_json::to_value(call.model.compat).unwrap();
            self.0.lock().unwrap().push(value);
            let stream = AssistantMessageEventStream::new();
            stream.end(None);
            Ok(stream)
        }
    }
}
