//! Provider registration records through both component adapters.
use super::scenario::Driver;

/// What the host receives for each authored registration, in call order.
const RECORDS: [&str; 3] = [
    "provider register absent {}",
    r#"provider register empty {"name":"","baseUrl":"","apiKey":"","api":"","headers":{},"authHeader":false,"models":[]}"#,
    concat!(
        r#"provider register full {"name":" Full ","baseUrl":"https://full.invalid","apiKey":"KEY","#,
        r#""api":"openai-completions","headers":{"z":"1","a":""},"authHeader":true,"models":["#,
        r#"{"id":"first","name":"first name","api":"openai-responses","baseUrl":"","reasoning":false,"#,
        r#""thinkingLevelMap":{"high":null,"low":""},"input":[],"#,
        r#""cost":{"input":0.0,"output":-1.5,"cacheRead":0.25,"cacheWrite":-0.0},"#,
        r#""contextWindow":128000.0,"maxTokens":-0.0,"headers":{"z":"1","a":""},"#,
        r#""compat":{"open":{"nested":[1,null]}}},"#,
        r#"{"id":"repeated","name":"repeated name","reasoning":false,"input":["text","image","text"],"#,
        r#""cost":{"input":0.0,"output":-1.5,"cacheRead":0.25,"cacheWrite":-0.0},"#,
        r#""contextWindow":128000.0,"maxTokens":-0.0}]}"#,
    ),
];

/// Registers each record, then a nonfinite one that never reaches the host.
async fn records(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command("capture", "provider records")
        .await?;
    let transcript = driver.transcript();
    let start = transcript
        .iter()
        .position(|line| line.starts_with("provider "))
        .ok_or("no provider registration")?;
    assert_eq!(transcript[start..], RECORDS);
    driver
        .run_plain_command("capture", "provider nonfinite")
        .await?;
    assert_eq!(
        driver.transcript()[start + RECORDS.len()..],
        [
            r#"entry provider-error "extension wrote a non-finite number (Infinity or NaN)""#,
            "provider unregister absent",
        ]
    );
    Ok(())
}

#[test]
fn provider_config_keeps_optional_metadata_and_order() -> Result<(), String> {
    on_both_adapters!(records)
}
