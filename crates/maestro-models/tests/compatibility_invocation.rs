#[cfg(test)]
mod tests {
    use maestro_models::records::api_registry::{
        ApiProvider, register_api_provider, unregister_api_providers,
    };
    use maestro_models::records::stream::{stream, stream_simple};
    use maestro_models::{AssistantMessageEventStream, Context, Model};
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};

    #[test]
    fn custom_callbacks_receive_complete_compatibility() {
        let expected = json!({"supportsStore":false,"sendSessionIdHeader":true,"open":{"order":["b","a","b"],"null":null}});
        let model: Model = serde_json::from_value(json!({"id":"m","name":"M","api":"compatibility-custom","provider":"custom","baseUrl":"https://fixture.invalid","reasoning":false,"input":["text"],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":100,"maxTokens":10,"compat":expected})).unwrap();
        let observed = Arc::new(Mutex::new(Vec::<Value>::new()));
        let raw = Arc::clone(&observed);
        let simple = Arc::clone(&observed);
        register_api_provider(
            ApiProvider {
                api: model.api.clone(),
                stream: Arc::new(move |model, _, _| {
                    let value = serde_json::to_value(model.compat).unwrap();
                    raw.lock().unwrap().push(value);
                    let stream = AssistantMessageEventStream::new();
                    stream.end(None);
                    Ok(stream)
                }),
                stream_simple: Arc::new(move |model, _, _| {
                    let value = serde_json::to_value(model.compat).unwrap();
                    simple.lock().unwrap().push(value);
                    let stream = AssistantMessageEventStream::new();
                    stream.end(None);
                    Ok(stream)
                }),
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
}
