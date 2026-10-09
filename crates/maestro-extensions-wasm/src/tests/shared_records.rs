//! Shared records through both extension adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::documents::{Answer, ask};
use super::scenario::{Decision, Driver};
use serde_json::{Value, json};

/// Representative finite extremes, integer-valued floats, subnormals and signed zeros.
const FINITE: [u64; 15] = [
    0,
    0x8000_0000_0000_0000,
    1,
    0x8000_0000_0000_0001,
    0x000f_ffff_ffff_ffff,
    0x0010_0000_0000_0000,
    0x3ff0_0000_0000_0000,
    0xbff0_0000_0000_0000,
    0x7fef_ffff_ffff_ffff,
    0xffef_ffff_ffff_ffff,
    0x4009_21fb_5444_2d18,
    0x4340_0000_0000_0000,
    0x4340_0000_0000_0001,
    0x3fd5_5555_5555_5555,
    0xbff8_0000_0000_0000,
];

/// Reads the returned user timestamp without floating-point equality losing signed zero.
fn timestamp(answer: &Answer) -> Result<u64, String> {
    answer
        .event
        .as_ref()
        .and_then(|e| e["messages"][0]["timestamp"].as_f64())
        .map(f64::to_bits)
        .ok_or("missing timestamp".to_owned())
}

/// Roundtrips both host-supplied values and actual guest field assignments.
async fn finite(driver: &mut impl Driver) -> Result<(), String> {
    for bits in FINITE {
        let event = json!({"type":"context", "messages":[{
            "role":"user", "content":"f64:7ff8000000000042", "timestamp":f64::from_bits(bits)
        }]});
        assert_eq!(timestamp(&ask(driver, &event, &json!({})).await?)?, bits);
        assert_eq!(
            timestamp(
                &ask(
                    driver,
                    &json!({"type":"context", "messages":[{"role":"user","content":"assigned","timestamp":42.5}]}),
                    &json!({"numberBits":format!("{bits:016x}")})
                )
                .await?
            )?,
            bits
        );
    }
    Ok(())
}

#[test]
fn maestro_shared_finite_numbers_keep_bits_across_adapters() -> Result<(), String> {
    on_both_adapters!(finite)
}

/// A typed guest assignment must fail before JSON can silently turn it into null.
async fn nonfinite(driver: &mut impl Driver) -> Result<(), String> {
    let handler = driver.identity("event probe")?;
    for bits in [
        f64::INFINITY.to_bits(),
        f64::NEG_INFINITY.to_bits(),
        f64::NAN.to_bits(),
        0x7ff8_0000_0000_0042,
    ] {
        for event in [
            r#"{"type":"context","messages":[{"role":"user","content":"x","timestamp":0.0}]}"#,
            r#"{"type":"turn_start","turnIndex":0.0,"timestamp":0.0}"#,
        ] {
            let directive =
                json!({"numberBits":format!("{bits:016x}"),"ending":{"fails":"authored failure"}});
            let delivery = driver
                .deliver(handler, event, &directive.to_string(), None)
                .await?;
            assert_eq!(
                delivery.event,
                Err("extension wrote a non-finite number (Infinity or NaN)".to_owned())
            );
            assert_eq!(
                delivery.decision,
                Decision::Failed("authored failure".to_owned())
            );
            println!(
                "nonfinite {bits:016x}: event rejected before JSON; handler failure preserved"
            );
        }
    }
    for bits in [
        f64::INFINITY.to_bits(),
        f64::NEG_INFINITY.to_bits(),
        f64::NAN.to_bits(),
    ] {
        let event = r#"{"type":"agent_start"}"#;
        let directive = json!({"resultNumberBits":format!("{bits:016x}")});
        let delivery = driver
            .deliver(handler, event, &directive.to_string(), None)
            .await?;
        assert_eq!(delivery.event, Ok(Some(event.into())));
        assert_eq!(
            delivery.decision,
            Decision::Returned(Err(
                "extension wrote a non-finite number (Infinity or NaN)".into()
            ))
        );
    }
    Ok(())
}

#[test]
fn maestro_shared_nonfinite_guest_edits_are_errors_across_adapters() -> Result<(), String> {
    on_both_adapters!(nonfinite)
}

/// Keeps the open role and its opaque data; known roles cannot enter that fallback.
async fn open_role(driver: &mut impl Driver) -> Result<(), String> {
    let event = json!({"type":"message_end", "message":{
        "role":"plugin", "data":" {\"a\":1,\"a\":2,\"float\":\"f64:7ff0000000000000\"} "
    }});
    assert_eq!(
        ask(driver, &event, &json!({})).await?,
        Answer::returned(&event, None)
    );
    let handler = driver.identity("event probe")?;
    assert!(
        driver
            .transcript()
            .iter()
            .any(|line| line == "entry entered none")
    );
    for role in [
        "user",
        "assistant",
        "toolResult",
        "bashExecution",
        "custom",
        "branchSummary",
        "compactionSummary",
    ] {
        let entered = driver
            .transcript()
            .iter()
            .filter(|line| line.as_str() == "entry entered none")
            .count();
        let invalid = json!({"type":"message_end","message":{"role":role,"data":"opaque"}});
        assert!(
            driver
                .deliver(handler, &invalid.to_string(), "{}", None)
                .await
                .is_err()
        );
        assert_eq!(
            driver
                .transcript()
                .iter()
                .filter(|line| line.as_str() == "entry entered none")
                .count(),
            entered
        );
    }
    Ok(())
}

#[test]
fn maestro_shared_open_roles_keep_opaque_data_across_adapters() -> Result<(), String> {
    on_both_adapters!(open_role)
}

/// Host Option fields collapse null and absence; nested host options keep explicit null.
async fn optional(driver: &mut impl Driver) -> Result<(), String> {
    for suffix in ["", ",\"textSignature\":null"] {
        let text = format!(
            r#"{{"type":"context","messages":[{{"role":"user","content":[{{"type":"text","text":"x"{suffix}}}],"timestamp":0.0}}]}}"#
        );
        let event: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let answer = ask(driver, &event, &json!({})).await?;
        let actual = answer.event.ok_or("no event")?;
        let host: maestro_request::types::Message =
            serde_json::from_value(event["messages"][0].clone()).map_err(|e| e.to_string())?;
        assert_eq!(
            actual["messages"][0],
            serde_json::to_value(host).map_err(|e| e.to_string())?
        );
        assert!(
            actual["messages"][0]["content"][0]
                .get("textSignature")
                .is_none()
        );
    }
    for suffix in ["", ",\"baseDir\":null", ",\"baseDir\":\"\""] {
        let source_text = format!(
            r#"{{"path":"p","source":"s","scope":"project","origin":"top-level"{suffix}}}"#
        );
        let source: Value = serde_json::from_str(&source_text).map_err(|e| e.to_string())?;
        let event = json!({"type":"before_agent_start","prompt":"p","systemPrompt":"s","systemPromptOptions":{
            "cwd":"c","skills":[{"name":"n","description":"d","filePath":"f","baseDir":"b","sourceInfo":source,"disableModelInvocation":false}]
        }});
        let actual = ask(driver, &event, &json!({}))
            .await?
            .event
            .ok_or("no event")?;
        let host: maestro_request::source_info::SourceInfo =
            serde_json::from_str(&source_text).map_err(|e| e.to_string())?;
        assert_eq!(
            actual["systemPromptOptions"]["skills"][0]["sourceInfo"],
            serde_json::to_value(host).map_err(|e| e.to_string())?
        );
    }
    Ok(())
}

#[test]
fn maestro_shared_optional_fields_match_host_records_across_adapters() -> Result<(), String> {
    on_both_adapters!(optional)
}

/// Numeric and textual diagnostic codes cross the actual event transport unambiguously.
async fn union_codes(driver: &mut impl Driver) -> Result<(), String> {
    for code in [
        json!(-0.0),
        json!(1.0),
        json!("1.0"),
        json!("Infinity"),
        json!("NaN"),
        json!("f64:7ff0000000000000"),
    ] {
        let event = json!({"type":"context","messages":[{
            "role":"assistant","content":[],"api":"a","provider":"p","model":"m",
            "usage":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0,"totalTokens":0.0,
                "cost":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0,"total":0.0}},
            "stopReason":"stop","timestamp":0.0,
            "diagnostics":[{"type":"error","timestamp":0.0,"error":{"message":"x","code":code}}]
        }]});
        let returned = ask(driver, &event, &json!({}))
            .await?
            .event
            .ok_or("no event")?;
        let actual = &returned["messages"][0]["diagnostics"][0]["error"]["code"];
        assert_eq!(actual, &code);
        if let Some(number) = code.as_f64() {
            assert_eq!(actual.as_f64().map(f64::to_bits), Some(number.to_bits()));
        }
    }
    Ok(())
}

#[test]
fn maestro_shared_number_or_string_unions_keep_their_branch_across_adapters() -> Result<(), String>
{
    on_both_adapters!(union_codes)
}
