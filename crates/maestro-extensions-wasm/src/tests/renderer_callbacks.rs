//! Retained renderer behavior through the controlled and compiled adapters.
use super::{
    renderer_host::{Call, Reply},
    scenario::Driver,
};
use serde_json::{Value, json};

/// A complete message whose fields differ from defaults.
pub(super) fn message() -> String {
    json!({"role":"custom", "customType":"雪", "content":[{"type":"text", "text":"z", "textSignature":"署名"},{"type":"image", "data":"literal", "mimeType":"image/custom"},{"type":"text", "text":"z"}], "display":true, "details":"{\"n\":7}", "timestamp":1.25}).to_string()
}
/// Registers selected authored behavior through the shared capture command.
async fn register(
    driver: &mut impl Driver,
    names: &[&str],
    mode: &str,
    error: Option<&str>,
    reenter: bool,
) -> Result<(), String> {
    driver
        .run_plain_command(
            "capture",
            &format!(
                "renderer-register {}",
                json!({"names":names,"mode":mode,"error":error,"reenter":reenter})
            ),
        )
        .await
}
/// Reads the exact ordered lines of the retained view.
async fn lines(
    driver: &mut impl Driver,
    id: usize,
    width: u32,
    text: &str,
    invalidations: u32,
) -> Result<(), String> {
    assert_eq!(
        driver.component(id, Call::Render(width)).await?,
        Reply::Lines(vec![
            format!("{width}:{text}:{invalidations}"),
            String::new(),
            "a".into(),
            "z".into(),
            "\u{1b}[31m雪".into()
        ])
    );
    Ok(())
}
/// Checks the complete emission from the just-completed synchronous operation.
fn emitted(driver: &impl Driver, before: usize, expected: (usize, &str, Value)) {
    assert_eq!(
        driver.renderer_emissions()[before..],
        [(expected.0, expected.1.into(), expected.2)]
    );
}
/// Invokes literal registrations and observes complete operands and live state.
async fn invocation(driver: &mut impl Driver) -> Result<(), String> {
    for name in ["", " x ", "雪", "__proto__", "constructor", "a/b"] {
        register(driver, &[name], "stateful", None, false).await?;
        assert_eq!(
            driver
                .transcript()
                .iter()
                .rev()
                .find(|s| s.starts_with("renderer-register "))
                .unwrap(),
            &format!("renderer-register {}", json!(name))
        );
        let handler = driver.identity(&format!("renderer {name}"))?;
        for expanded in [false, true] {
            live_component(driver, handler, expanded).await?;
        }
    }
    optional_components(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_renderer_invokes_and_retains_component() -> Result<(), String> {
    on_both_adapters!(invocation)
}

/// Proves registration, resource, facade and theme owners release independently.
async fn ownership(driver: &mut impl Driver) -> Result<(), String> {
    register(driver, &["one", "two"], "alias", None, true).await?;
    let one = driver.identity("renderer one")?;
    let two = driver.identity("renderer two")?;
    let first = driver
        .renderer(one, &message(), true, "theme-one")
        .await?
        .ok_or("missing first")?;
    let second = driver
        .renderer(two, &message(), false, "theme-two")
        .await?
        .ok_or("missing second")?;
    driver.component(first, Call::Input("shared")).await?;
    lines(driver, second, 1, "shared", 0).await?;
    let start = driver.transcript().len();
    let third = release_renderer_aliases(driver, two, start).await?;
    retained_resources(driver, [first, second, third], start).await?;
    rejection(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
/// A rejected replacement drops new captures and leaves the prior renderer callable.
async fn rejection(driver: &mut impl Driver) -> Result<(), String> {
    register(driver, &["retained"], "stateful", None, false).await?;
    let prior = driver.identity("renderer retained")?;
    for error in ["", "registration refused: Ω\nline"] {
        driver.registration_error(Some(error.into()));
        let start = driver.transcript().len();
        let before = driver.dropped().len();
        assert_eq!(
            register(driver, &["retained"], "stateful", None, false)
                .await
                .unwrap_err(),
            error
        );
        assert_eq!(driver.dropped().len() - before, 1);
        assert!(driver.transcript()[start..].contains(&"entry released \"renderer\"".into()));
        assert_eq!(driver.identity("renderer retained")?, prior);
        let id = driver
            .renderer(prior, &message(), false, "retained-theme")
            .await?
            .ok_or("prior missing")?;
        lines(driver, id, 7, "z", 0).await?;
        driver.drop_component(id).await?;
    }
    driver.registration_error(None);
    Ok(())
}
#[test]
fn maestro_renderer_owners_release_independently() -> Result<(), String> {
    on_both_adapters!(ownership)
}

/// All synchronous renderer/view emitter routes retain resource identity and errors.
async fn emissions(driver: &mut impl Driver) -> Result<(), String> {
    register(driver, &["emit"], "emit", Some("author Ω\nline"), false).await?;
    let handler = driver.identity("renderer emit")?;
    let id = driver
        .renderer(handler, &message(), true, "theme-emitter")
        .await?
        .ok_or("missing emitter view")?;
    assert_eq!(
        driver.renderer_emissions(),
        [(1, "renderer".into(), json!(true))]
    );
    for (index, call, channel) in [
        (2, Call::Render(7), "render"),
        (3, Call::Input("ok"), "input"),
        (4, Call::Release, "release"),
        (5, Call::Invalidate, "invalidate"),
    ] {
        let before = driver.renderer_emissions().len();
        driver.component(id, call).await?;
        emitted(
            driver,
            before,
            (
                index,
                channel,
                json!({"text":if index==2 {"z"} else {"ok"},"invalidations":u32::from(index==5)}),
            ),
        );
    }
    host_emission_errors(driver, handler, id).await?;
    author_emission_errors(driver, handler, id).await?;
    invalid_renderer(driver, handler).await?;
    empty_author_error(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_renderer_callbacks_emit_and_recover() -> Result<(), String> {
    on_both_adapters!(emissions)
}

/// Normalizes only the shared floating-point timestamp representation.
fn wire_message(mut message: Value) -> Value {
    if let Some(timestamp) = message.get_mut("timestamp") {
        *timestamp = json!(timestamp.as_f64().unwrap());
    }
    message
}
/// Reads the invocation's complete author observation without flattening optional fields.
fn received(driver: &impl Driver, start: usize) -> Vec<Value> {
    driver.transcript()[start..]
        .iter()
        .filter_map(|s| s.strip_prefix("entry renderer-message "))
        .map(|s| serde_json::from_str(s).unwrap())
        .collect()
}
/// Complete native input domain, observed through actual selected callbacks.
async fn decoding(driver: &mut impl Driver) -> Result<(), String> {
    use super::renderer_host::{Decoder, Native};
    register(driver, &["decode"], "stateful", None, false).await?;
    let handler = driver.identity("renderer decode")?;
    let cases: Vec<Decoder> = serde_json::from_str(include_str!("renderer_decoder.json")).unwrap();
    let mut queries = std::collections::HashSet::new();
    for case in cases {
        assert!(queries.insert(case.raw.clone()), "duplicate native query");
        let start = driver.transcript().len();
        let result = driver.renderer(handler, &case.raw, true, &case.name).await;
        match case.native {
            Native::Ok { ok, negative_zero } => {
                let id = result?.ok_or("accepted record missing view")?;
                assert_eq!(
                    received(driver, start),
                    [json!({"message":ok,"expanded":true,"negative_zero":negative_zero})],
                    "case {}",
                    case.name
                );
                driver.drop_component(id).await?;
            }
            Native::Error { error } => {
                assert_eq!(result.unwrap_err(), error, "case {}", case.name);
                assert!(received(driver, start).is_empty(), "case {}", case.name);
            }
        }
        assert!(driver.transcript()[start..].contains(&format!("theme-drop {}", case.name)));
    }
    let unknown = driver.unregistered()?;
    let start = driver.transcript().len();
    assert_eq!(
        driver
            .renderer(unknown, "{", false, "unknown-malformed")
            .await
            .unwrap_err(),
        "no callback registered for identity 4242"
    );
    assert_eq!(
        driver.transcript()[start..],
        ["theme-drop unknown-malformed"]
    );
    Ok(())
}
/// Forwards every recorded source query through its owning public operation.
async fn corpus(driver: &mut impl Driver) -> Result<(), String> {
    use super::renderer_host::{Case, Group};
    let cases: Vec<Case> = serde_json::from_str(include_str!("renderer_corpus.json")).unwrap();
    let mut queries = std::collections::HashSet::new();
    for case in cases {
        assert!(
            queries.insert(format!("{:?}:{}", case.group, case.input)),
            "duplicate source query"
        );
        match case.group {
            Group::Message => corpus_message(driver, case.input, case.expected).await?,
            Group::Outcome => corpus_outcome(driver, case.input, case.expected).await?,
            Group::Registration => corpus_registration(driver, case.input, case.expected).await?,
            Group::RejectedRegistration => {
                corpus_rejected(driver, case.input, case.expected).await?;
            }
            Group::Component => corpus_component(driver, case.input, case.expected).await?,
            Group::ReleasePreference => corpus_release(driver, case.input, case.expected).await?,
            Group::LineSequence => corpus_lines(driver, case.input, case.expected).await?,
            Group::AbsentInput => corpus_default(driver, case.input, case.expected).await?,
            Group::SignedText => corpus_signed(driver, case.input, case.expected).await?,
            Group::Timestamp => corpus_timestamp(driver, case.input, case.expected).await?,
        }
    }
    Ok(())
}
/// Asserts complete message, expansion, optional return and independently supplied theme.
async fn corpus_message(
    driver: &mut impl Driver,
    input: Value,
    expected: Value,
) -> Result<(), String> {
    use super::renderer_host::{MessageExpected, MessageInput};
    let input: MessageInput = serde_json::from_value(input).unwrap();
    let expected: MessageExpected = serde_json::from_value(expected).unwrap();
    register(driver, &["corpus"], "stateful", None, false).await?;
    let handler = driver.identity("renderer corpus")?;
    let start = driver.transcript().len();
    let id = driver
        .renderer(
            handler,
            &wire_message(input.message).to_string(),
            input.expanded,
            &expected.theme,
        )
        .await?;
    assert_eq!(id.is_some(), expected.returned);
    assert_eq!(
        received(driver, start),
        [
            json!({"message":wire_message(expected.message),"expanded":expected.expanded,"negative_zero":false})
        ]
    );
    assert!(driver.transcript()[start..].contains(&format!("theme-drop {}", expected.theme)));
    if let Some(id) = id {
        driver.drop_component(id).await?;
    }
    Ok(())
}
/// Distinguishes absent, returned and authored-error outcomes.
async fn corpus_outcome(
    driver: &mut impl Driver,
    input: Value,
    expected: Value,
) -> Result<(), String> {
    use super::renderer_host::{OutcomeExpected, OutcomeInput};
    let input: OutcomeInput = serde_json::from_value(input).unwrap();
    let expected: OutcomeExpected = serde_json::from_value(expected).unwrap();
    let mode = if input.mode == "component" {
        "stateful"
    } else {
        input.mode.as_str()
    };
    register(driver, &["outcome"], mode, expected.error.as_deref(), false).await?;
    let handler = driver.identity("renderer outcome")?;
    let start = driver.transcript().len();
    let result = driver
        .renderer(handler, &message(), input.expanded, "outcome-theme")
        .await;
    assert_eq!(received(driver, start).len(), expected.called);
    if let Some(error) = expected.error {
        assert_eq!(result.unwrap_err(), error);
    } else {
        let id = result?;
        assert_eq!(id.is_some(), expected.returned.expect("recorded presence"));
        if let Some(id) = id {
            driver.drop_component(id).await?;
        }
    }
    Ok(())
}
/// Installs a distinguishable callback marker to observe host-selected replacement.
async fn marker(driver: &mut impl Driver, name: &str, marker: u32) -> Result<(), String> {
    driver
        .run_plain_command(
            "capture",
            &format!(
                "renderer-register {}",
                json!({"names":[name],"mode":"none","error":null,"reenter":false,"marker":marker})
            ),
        )
        .await
}
/// Selected callback identity is observed through the invocation, not key equality alone.
async fn selected(driver: &mut impl Driver, name: &str, marker: u32) -> Result<(), String> {
    let handler = driver.identity(&format!("renderer {name}"))?;
    let start = driver.transcript().len();
    assert!(
        driver
            .renderer(handler, &message(), false, "selected-theme")
            .await?
            .is_none()
    );
    assert!(driver.transcript()[start..].contains(&format!("entry renderer-selected {marker}")));
    Ok(())
}
/// Literal keys reach the host unchanged and the selected replacement remains distinct.
async fn corpus_registration(
    driver: &mut impl Driver,
    input: Value,
    expected: Value,
) -> Result<(), String> {
    use super::renderer_host::{RegistrationExpected, RegistrationInput};
    let input: RegistrationInput = serde_json::from_value(input).unwrap();
    let expected: RegistrationExpected = serde_json::from_value(expected).unwrap();
    let start = driver.transcript().len();
    marker(driver, &input.name, 1).await?;
    marker(driver, "decoy", 9).await?;
    marker(driver, &input.name, 2).await?;
    let keys = driver.transcript()[start..]
        .iter()
        .filter_map(|s| s.strip_prefix("renderer-register "))
        .map(|s| serde_json::from_str::<String>(s).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(keys[..2], expected.keys);
    assert_eq!(&keys[2..], std::slice::from_ref(&input.name));
    selected(driver, &input.name, expected.selected).await?;
    assert_eq!(
        driver.identity("renderer never-registered").is_err(),
        expected.missing
    );
    Ok(())
}
/// Host rejection does not replace the already selected callback.
async fn corpus_rejected(
    driver: &mut impl Driver,
    input: Value,
    expected: Value,
) -> Result<(), String> {
    use super::renderer_host::{RegistrationInput, RejectedExpected};
    let input: RegistrationInput = serde_json::from_value(input).unwrap();
    let expected: RejectedExpected = serde_json::from_value(expected).unwrap();
    driver.registration_error(Some(expected.error.clone()));
    assert_eq!(
        marker(driver, &input.name, 3).await.unwrap_err(),
        expected.error
    );
    driver.registration_error(None);
    selected(driver, &input.name, expected.selected).await
}
/// Width, edit and invalidation observe the same retained view in source order.
async fn corpus_component(
    driver: &mut impl Driver,
    input: Value,
    expected: Value,
) -> Result<(), String> {
    use super::renderer_host::{ComponentExpected, ComponentInput};
    let input: ComponentInput = serde_json::from_value(input).unwrap();
    let expected: ComponentExpected = serde_json::from_value(expected).unwrap();
    register(driver, &["component-corpus"], "oracle", None, false).await?;
    let handler = driver.identity("renderer component-corpus")?;
    let id = driver
        .renderer(handler, &message(), false, "component-theme")
        .await?
        .ok_or("missing component")?;
    assert_eq!(
        driver.component(id, Call::Render(input.width)).await?,
        Reply::Lines(expected.before)
    );
    driver.component(id, Call::Input(&input.data)).await?;
    driver.component(id, Call::Invalidate).await?;
    assert_eq!(
        driver.component(id, Call::Render(input.width)).await?,
        Reply::Lines(expected.after)
    );
    driver.drop_component(id).await
}
/// Both states of the live release getter are observed on one retained view.
async fn corpus_release(
    driver: &mut impl Driver,
    input: Value,
    expected: Value,
) -> Result<(), String> {
    use super::renderer_host::{ReleaseExpected, ReleaseInput};
    let input: ReleaseInput = serde_json::from_value(input).unwrap();
    let expected: ReleaseExpected = serde_json::from_value(expected).unwrap();
    driver.run_plain_command("capture",&format!("renderer-register {}",json!({"names":["release-corpus"],"mode":"stateful","error":null,"reenter":false,"preference":input.preference}))).await?;
    let handler = driver.identity("renderer release-corpus")?;
    let id = driver
        .renderer(handler, &message(), false, "release-theme")
        .await?
        .ok_or("missing component")?;
    assert_eq!(
        driver.component(id, Call::Release).await?,
        Reply::Release(!expected.filter_release_before)
    );
    driver.component(id, Call::Input("release")).await?;
    assert_eq!(
        driver.component(id, Call::Release).await?,
        Reply::Release(!expected.filter_release_after)
    );
    driver.drop_component(id).await
}
/// Exact authored vectors retain blank, repeated, ANSI and Unicode lines.
async fn corpus_lines(
    driver: &mut impl Driver,
    input: Value,
    expected: Value,
) -> Result<(), String> {
    use super::renderer_host::Lines;
    let input: Lines = serde_json::from_value(input).unwrap();
    let expected: Lines = serde_json::from_value(expected).unwrap();
    driver.run_plain_command("capture",&format!("renderer-register {}",json!({"names":["line-corpus"],"mode":"lines","error":null,"reenter":false,"lines":input.lines}))).await?;
    let handler = driver.identity("renderer line-corpus")?;
    let id = driver
        .renderer(handler, &message(), false, "line-theme")
        .await?
        .ok_or("missing component")?;
    assert_eq!(
        driver.component(id, Call::Render(7)).await?,
        Reply::Lines(expected.lines)
    );
    driver.drop_component(id).await
}
/// Synchronous optional input defaults neither render nor change the view.
async fn corpus_default(
    driver: &mut impl Driver,
    input: Value,
    expected: Value,
) -> Result<(), String> {
    use super::renderer_host::{AbsentExpected, AbsentInput};
    let input: AbsentInput = serde_json::from_value(input).unwrap();
    let expected: AbsentExpected = serde_json::from_value(expected).unwrap();
    register(driver, &["default-corpus"], "default", None, false).await?;
    let handler = driver.identity("renderer default-corpus")?;
    let id = driver
        .renderer(handler, &message(), false, "default-theme")
        .await?
        .ok_or("missing component")?;
    let start = driver.transcript().len();
    driver.component(id, Call::Input(&input.data)).await?;
    assert_eq!(
        driver.transcript()[start..]
            .iter()
            .filter(|s| s.starts_with("entry default-render "))
            .count(),
        expected.renders
    );
    assert_eq!(
        driver.component(id, Call::Render(7)).await?,
        Reply::Lines(vec!["default:7".into()])
    );
    driver.drop_component(id).await
}
/// Actual callback records witness optional text signatures.
async fn corpus_signed(
    driver: &mut impl Driver,
    input: Value,
    expected: Value,
) -> Result<(), String> {
    use super::renderer_host::Signed;
    let input: Signed = serde_json::from_value(input).unwrap();
    let expected: Signed = serde_json::from_value(expected).unwrap();
    register(driver, &["signed-corpus"], "none", None, false).await?;
    let handler = driver.identity("renderer signed-corpus")?;
    let start = driver.transcript().len();
    assert!(
        driver
            .renderer(handler, &input.message.to_string(), false, "signed-theme")
            .await?
            .is_none()
    );
    assert_eq!(
        received(driver, start),
        [json!({"message":wire_message(expected.message),"expanded":false,"negative_zero":false})]
    );
    Ok(())
}
/// Finite timestamps retain value and signed-zero state after shared decoding.
async fn corpus_timestamp(
    driver: &mut impl Driver,
    input: Value,
    expected: Value,
) -> Result<(), String> {
    use super::renderer_host::Timestamp;
    let input: Timestamp = serde_json::from_value(input).unwrap();
    let expected: Timestamp = serde_json::from_value(expected).unwrap();
    register(driver, &["timestamp-corpus"], "none", None, false).await?;
    let handler = driver.identity("renderer timestamp-corpus")?;
    let raw = format!(
        "{{\"customType\":\"timestamp\",\"content\":\"x\",\"display\":false,\"timestamp\":{}}}",
        input.literal
    );
    let start = driver.transcript().len();
    assert!(
        driver
            .renderer(handler, &raw, false, "timestamp-theme")
            .await?
            .is_none()
    );
    let received = received(driver, start);
    assert_eq!(received.len(), 1);
    assert_eq!(
        received[0]["message"]["timestamp"]
            .as_f64()
            .unwrap()
            .to_bits(),
        expected.literal.parse::<f64>().unwrap().to_bits()
    );
    assert_eq!(
        received[0]["negative_zero"],
        json!(expected.literal == "-0")
    );
    Ok(())
}
/// Source corpus and native ingress are both observed through selected author callbacks.
async fn payloads(driver: &mut impl Driver) -> Result<(), String> {
    corpus(driver).await?;
    decoding(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_renderer_payloads_keep_existing_record_semantics() -> Result<(), String> {
    on_both_adapters!(payloads)
}

/// Optional results remain distinct from successful empty rendering.
async fn optional_components(driver: &mut impl Driver) -> Result<(), String> {
    for (mode, expected) in [
        ("none", None),
        ("empty", Some(vec![])),
        ("default", Some(vec!["default:7".into()])),
    ] {
        register(driver, &["result"], mode, None, false).await?;
        let handler = driver.identity("renderer result")?;
        let component = driver
            .renderer(handler, &message(), false, "theme-default")
            .await?;
        assert_eq!(component.is_some(), expected.is_some());
        if let Some(id) = component {
            driver.component(id, Call::Input("discarded")).await?;
            assert_eq!(
                driver.component(id, Call::Release).await?,
                Reply::Release(false)
            );
            assert_eq!(
                driver.component(id, Call::Render(7)).await?,
                Reply::Lines(expected.unwrap())
            );
            driver.drop_component(id).await?;
        }
    }
    Ok(())
}

/// One renderer invocation keeps its state live through subsequent methods.
async fn live_component(
    driver: &mut impl Driver,
    handler: u32,
    expanded: bool,
) -> Result<(), String> {
    let start = driver.transcript().len();
    let id = driver
        .renderer(handler, &message(), expanded, "theme-A")
        .await?
        .ok_or("component missing")?;
    assert_eq!(
        driver.transcript()[start],
        format!(
            "entry renderer-message {}",
            json!({"message":serde_json::from_str::<Value>(&message()).unwrap(),"expanded":expanded,"negative_zero":false})
        )
    );
    assert_eq!(
        driver.component(id, Call::Release).await?,
        Reply::Release(false)
    );
    for (width, input) in [
        (0, ""),
        (1, "雪"),
        (7, "\u{3}"),
        (80, "\u{1b}[97;1:3u"),
        (240, "\u{1b}[200~paste\u{1b}[201~"),
    ] {
        driver.component(id, Call::Input(input)).await?;
        lines(driver, id, width, input, u32::from(expanded)).await?;
    }
    driver.component(id, Call::Input("release")).await?;
    assert_eq!(
        driver.component(id, Call::Release).await?,
        Reply::Release(true)
    );
    driver.component(id, Call::Invalidate).await?;
    lines(driver, id, 7, "release", u32::from(expanded) + 1).await?;
    driver.component(id, Call::Input("z")).await?;
    driver.drop_component(id).await?;
    Ok(())
}

/// Returned resource aliases outlive the renderer and release their own owners.
async fn retained_resources(
    driver: &mut impl Driver,
    ids: [usize; 3],
    start: usize,
) -> Result<(), String> {
    let [first, second, third] = ids;
    driver.run_plain_command("renderer-control", "view").await?;
    driver.drop_component(first).await?;
    driver.component(second, Call::Invalidate).await?;
    lines(driver, third, 7, "shared", 1).await?;
    driver.drop_component(second).await?;
    assert!(
        !driver.transcript()[start..]
            .iter()
            .any(|s| s.starts_with("entry view-drop "))
    );
    driver.drop_component(third).await?;
    assert_eq!(
        driver.transcript()[start..]
            .iter()
            .filter(|s| s.starts_with("entry view-drop "))
            .count(),
        1
    );
    assert!(
        driver.transcript()[start..].contains(&"entry renderer-reentered {\"Ok\":null}".into())
    );
    assert!(
        !driver.transcript()[start..]
            .iter()
            .any(|s| s.starts_with("theme-drop "))
    );
    driver
        .run_plain_command("renderer-control", "themes")
        .await?;
    let drops = driver.transcript()[start..]
        .iter()
        .filter(|s| s.starts_with("theme-drop "))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        drops,
        [
            "theme-drop theme-one",
            "theme-drop theme-two",
            "theme-drop theme-three"
        ]
    );
    released_theme_owners(driver, start).await?;
    Ok(())
}

/// Each host-emission rejection keeps the earlier view mutation and later recovery.
async fn host_emission_errors(
    driver: &mut impl Driver,
    handler: u32,
    id: usize,
) -> Result<(), String> {
    for (attempt, error) in ["", "host Ω\nsecond line"].into_iter().enumerate() {
        driver.reject_bus_emission(Some(error.into()));
        let before = driver.renderer_emissions().len();
        for call in [
            Call::Input("before-host-error"),
            Call::Render(80),
            Call::Release,
            Call::Invalidate,
        ] {
            assert_eq!(driver.component(id, call).await.unwrap_err(), error);
        }
        assert_eq!(
            driver
                .renderer(handler, &message(), false, "theme-error")
                .await
                .unwrap_err(),
            error
        );
        assert_eq!(
            driver.renderer_emissions()[before..],
            ["input", "render", "release", "invalidate", "renderer"]
                .into_iter()
                .enumerate()
                .map(|(offset, channel)| (
                    6 + attempt * 6 + offset,
                    channel.into(),
                    if channel == "renderer" {
                        json!(false)
                    } else {
                        json!({"text":"before-host-error", "invalidations":1 + attempt + usize::from(channel == "invalidate")})
                    },
                ))
                .collect::<Vec<_>>()
        );
        driver.reject_bus_emission(None);
        lines(
            driver,
            id,
            7,
            "before-host-error",
            if error.is_empty() { 2 } else { 3 },
        )
        .await?;
    }
    Ok(())
}

/// Author failures leave their immediate effects visible on the same retained view.
async fn author_emission_errors(
    driver: &mut impl Driver,
    handler: u32,
    id: usize,
) -> Result<(), String> {
    let before = driver.renderer_emissions().len();
    assert_eq!(
        driver.component(id, Call::Input("fail")).await.unwrap_err(),
        "author Ω\nline"
    );
    emitted(
        driver,
        before,
        (18, "input", json!({"text":"fail", "invalidations":3})),
    );
    for (index, call, channel, invalidations) in [
        (19, Call::Render(0), "render", 3),
        (20, Call::Release, "release", 3),
        (21, Call::Invalidate, "invalidate", 4),
    ] {
        let before = driver.renderer_emissions().len();
        assert_eq!(
            driver.component(id, call).await.unwrap_err(),
            "author Ω\nline"
        );
        emitted(
            driver,
            before,
            (
                index,
                channel,
                json!({"text":"fail", "invalidations":invalidations}),
            ),
        );
    }
    driver.component(id, Call::Input("healthy")).await?;
    lines(driver, id, 1, "healthy", 4).await?;
    let mut failed: Value = serde_json::from_str(&message()).unwrap();
    failed["customType"] = json!("fail");
    let before = driver.renderer_emissions().len();
    assert_eq!(
        driver
            .renderer(handler, &failed.to_string(), false, "theme-author-error")
            .await
            .unwrap_err(),
        "author Ω\nline"
    );
    emitted(driver, before, (24, "renderer", json!(false)));
    let healthy = driver
        .renderer(handler, &message(), false, "theme-healthy")
        .await?
        .ok_or("missing recovered view")?;
    lines(driver, healthy, 7, "healthy", 4).await?;
    driver.drop_component(healthy).await?;
    driver.drop_component(id).await?;
    Ok(())
}

/// Missing, wrong-kind and released identities retain the existing diagnostic.
async fn invalid_renderer(driver: &mut impl Driver, handler: u32) -> Result<(), String> {
    let unknown = driver.unregistered()?;
    assert_eq!(
        driver
            .renderer(unknown, &message(), false, "theme-missing")
            .await
            .unwrap_err(),
        "no callback registered for identity 4242"
    );
    let wrong = driver.identity("command capture")?;
    assert_eq!(
        driver
            .renderer(wrong, &message(), false, "theme-wrong")
            .await
            .unwrap_err(),
        driver.callback_error(wrong)
    );
    let diagnostic = driver.callback_error(handler);
    assert!(driver.release("renderer emit").await);
    let released = driver.reborrow(driver.dropped().last().copied().unwrap())?;
    assert_eq!(
        driver
            .renderer(released, &message(), false, "theme-released")
            .await
            .unwrap_err(),
        diagnostic
    );
    Ok(())
}

/// Removing one registration preserves the shared callback until its final registration drops.
async fn release_renderer_aliases(
    driver: &mut impl Driver,
    two: u32,
    start: usize,
) -> Result<usize, String> {
    assert!(driver.release("renderer one").await);
    assert!(
        !driver.transcript()[start..]
            .iter()
            .any(|s| s == "entry released \"renderer\"")
    );
    let third = driver
        .renderer(two, &message(), true, "theme-three")
        .await?
        .ok_or("missing third")?;
    assert!(driver.release("renderer two").await);
    assert!(
        driver.transcript()[start..]
            .iter()
            .any(|s| s == "entry released \"renderer\"")
    );
    assert!(driver.transcript()[start..].contains(&"entry capture-reentered {\"Ok\":null}".into()));
    Ok(third)
}

/// Empty returned author failures stay distinct from absence and empty successful output.
async fn empty_author_error(driver: &mut impl Driver) -> Result<(), String> {
    register(driver, &["empty-author-error"], "error", Some(""), false).await?;
    let handler = driver.identity("renderer empty-author-error")?;
    let before = driver.renderer_emissions().len();
    assert_eq!(
        driver
            .renderer(handler, &message(), false, "empty-error-theme")
            .await
            .unwrap_err(),
        ""
    );
    assert!(driver.renderer_emissions()[before..].is_empty());
    register(driver, &["empty-author-view"], "emit", Some(""), false).await?;
    let handler = driver.identity("renderer empty-author-view")?;
    let id = driver
        .renderer(handler, &message(), false, "empty-view-theme")
        .await?
        .ok_or("view missing")?;
    let before = driver.renderer_emissions().len();
    assert_eq!(
        driver.component(id, Call::Input("fail")).await.unwrap_err(),
        ""
    );
    emitted(
        driver,
        before,
        (32, "input", json!({"text":"fail", "invalidations":0})),
    );
    for (index, call, channel, invalidations) in [
        (33, Call::Render(7), "render", 0),
        (34, Call::Release, "release", 0),
        (35, Call::Invalidate, "invalidate", 1),
    ] {
        let before = driver.renderer_emissions().len();
        assert_eq!(driver.component(id, call).await.unwrap_err(), "");
        emitted(
            driver,
            before,
            (
                index,
                channel,
                json!({"text":"fail", "invalidations":invalidations}),
            ),
        );
    }
    driver.component(id, Call::Input("healthy")).await?;
    lines(driver, id, 7, "healthy", 1).await?;
    driver.drop_component(id).await
}

/// Constructor-supplied and retained host theme owners release at their own boundaries.
async fn released_theme_owners(driver: &mut impl Driver, start: usize) -> Result<(), String> {
    assert_eq!(
        driver.transcript()[start..]
            .iter()
            .filter(|s| s.as_str() == "entry released \"local-theme\"")
            .count(),
        1
    );
    let capture = driver.identity("renderer capture-reentered")?;
    assert!(
        driver
            .renderer(capture, &message(), false, "capture-theme")
            .await?
            .is_none()
    );
    let fresh = driver.identity("renderer fresh-renderer")?;
    assert!(
        driver
            .renderer(fresh, &message(), false, "theme-fresh")
            .await?
            .is_none()
    );
    Ok(())
}
