use serde_json::Value;
pub fn compact(value: &Value) -> String {
    match value {
        Value::Number(number) => ryu_js::Buffer::new()
            .format(number.as_f64().unwrap())
            .to_owned(),
        Value::Array(values) => format!(
            "[{}]",
            values.iter().map(compact).collect::<Vec<_>>().join(",")
        ),
        Value::Object(values) => format!(
            "{{{}}}",
            values
                .iter()
                .map(|(key, value)| format!(
                    "{}:{}",
                    serde_json::to_string(key).unwrap(),
                    compact(value)
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
        _ => serde_json::to_string(value).unwrap(),
    }
}
pub fn snapshot() -> Value {
    Value::Array(
        maestro_models::get_providers()
            .into_iter()
            .map(|provider| {
                let models = maestro_models::get_models(&provider)
                    .into_iter()
                    .map(|handle| {
                        let model = handle.read().unwrap().clone();
                        serde_json::json!([model.id, serde_json::to_value(&model).unwrap()])
                    })
                    .collect::<Vec<_>>();
                serde_json::json!([provider, models])
            })
            .collect(),
    )
}
