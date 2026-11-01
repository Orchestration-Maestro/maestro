//! Input transforms: text and image chaining, explicit intent, handled input and failures.
use super::Harness;
use super::cases::{answer, edit, encoded, fail, labels, outcome, trace, typed};
use maestro_extensions::{InputSource, Presence};
use maestro_models::ImageContent;
use serde_json::{Value, json};

/// Index of the recorded case whose handler returns the caller's own image list.
const SAME_LIST: usize = 42;

/// An image with the given data.
fn image(data: &str) -> Value {
    json!({ "type": "image", "data": data, "mimeType": "image/png" })
}

/// A transform result, with images only when supplied.
fn transform(text: &str, images: Option<Value>) -> Value {
    let mut result = json!({ "action": "transform", "text": text });
    if let Some(images) = images {
        result["images"] = images;
    }
    result
}

/// Dispatches one recorded input case.
pub(super) async fn input(
    harness: &Harness,
    index: usize,
    input: &Value,
    recorded: Value,
) -> (Value, Value) {
    let change = input["edit"].as_str().unwrap();
    match change {
        "equal-empty-new-identity" => {
            let steps = json!([answer(transform("original", Some(json!([]))))]);
            let run = harness.prepare("input", &[vec![steps]]).await;
            let mut images = Presence::Present(Vec::new());
            let result = run
                .runner
                .emit_input("original", &mut images, InputSource::Extension, None)
                .await
                .unwrap();
            (
                outcome(json!({ "result": encoded(&result) }), &run),
                recorded,
            )
        }
        "mutate-same-list-return-continue" => {
            let steps = json!([
                edit("images", json!([image("old"), image("mutated")])),
                answer(json!({ "action": "continue" }))
            ]);
            let run = harness.prepare("input", &[vec![steps]]).await;
            let mut images: Presence<Vec<ImageContent>> = typed(json!([image("old")]));
            let result = run
                .runner
                .emit_input("original", &mut images, InputSource::Rpc, None)
                .await
                .unwrap();
            let observed =
                json!({ "result": encoded(&result), "originalImages": encoded(&images) });
            (outcome(observed, &run), recorded)
        }
        "empty text" => {
            let run = harness
                .prepare("input", &[vec![json!([answer(transform("", None))])]])
                .await;
            let mut images = Presence::Missing;
            let result = run
                .runner
                .emit_input("nonempty", &mut images, InputSource::Interactive, None)
                .await
                .unwrap();
            (encoded(&result), recorded)
        }
        _ => chained(harness, index, change, recorded).await,
    }
}

/// The first handler's ending for one recorded edit.
fn ending(edit: &str) -> Value {
    match edit {
        "handled" => answer(json!({ "action": "handled" })),
        "error" => fail("input fault"),
        "omitted" => answer(transform("original", None)),
        "text-reverted" => answer(transform("temporary", None)),
        "null" => answer(transform("original", Some(Value::Null))),
        "empty" => answer(transform("original", Some(json!([])))),
        "different" => answer(transform("original", Some(json!([image("new")])))),
        _ => answer(transform("original", Some(json!([image("old")])))),
    }
}

/// Two chained handlers over the caller's image slot.
async fn chained(
    harness: &Harness,
    index: usize,
    edit: &str,
    mut recorded: Value,
) -> (Value, Value) {
    let mut second = vec![trace("second")];
    if edit == "text-reverted" {
        second.push(answer(transform("original", None)));
    }
    let handlers = vec![json!([trace("first"), ending(edit)]), Value::Array(second)];
    let run = harness.prepare("input", &[handlers]).await;
    let mut images: Presence<Vec<ImageContent>> = typed(json!([image("old")]));
    let result = run
        .runner
        .emit_input("original", &mut images, InputSource::Interactive, None)
        .await
        .unwrap();
    let observed = json!({ "result": encoded(&result), "trace": labels(harness), "originalImages": encoded(&images) });
    let output = &mut recorded["output"];
    if index == SAME_LIST {
        // Returning the caller's own list is an identity the component boundary does not carry;
        // an explicitly supplied equal list is a transform.
        output["result"] = transform("original", Some(json!([image("old")])));
    }
    if let Some(selected) = output["result"].get("images").cloned() {
        // The caller passes its image slot by mutable reference and receives the selected
        // images there; the recording's untouched caller list is a reference observation.
        output["originalImages"] = selected;
    }
    (outcome(observed, &run), recorded)
}

#[test]
fn component_input_chains_text_preserves_images_and_forwards_sources() {
    super::local(async {
        let harness = Harness::load().await;
        let run = harness
            .prepare(
                "input",
                &[
                    vec![json!([{"observe":"input"}, {"suffix":"[1]"}])],
                    vec![json!([{"observe":"input"}, {"suffix":"[2]"}])],
                ],
            )
            .await;
        for (source, name) in [
            (InputSource::Interactive, "interactive"),
            (InputSource::Rpc, "rpc"),
            (InputSource::Extension, "extension"),
        ] {
            let mut images: Presence<Vec<ImageContent>> = typed(json!([image("old")]));
            let result = run
                .runner
                .emit_input("X", &mut images, source, None)
                .await
                .unwrap();
            assert_eq!(
                encoded(&result),
                transform("X[1][2]", Some(json!([image("old")])))
            );
            assert_eq!(encoded(&images), json!([image("old")]));
            assert_eq!(
                harness.bus.take(),
                [
                    json!({"text":"X", "images":[image("old")], "source":name}),
                    json!({"text":"X[1]", "images":[image("old")], "source":name}),
                ]
            );
            assert_eq!(run.errors(), json!([]));
        }
    });
}
