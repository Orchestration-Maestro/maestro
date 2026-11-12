//! Tests of the streaming parse's skip of attempts that cannot succeed.

use super::{parse_json_with_repair, parse_streaming_json, partial, repair_json};

/// The streaming parse as it read before incomplete text skipped the strict attempt.
fn every_attempt(input: &str) -> serde_json::Value {
    parse_json_with_repair(input)
        .ok()
        .or_else(|| partial(input))
        .or_else(|| partial(&repair_json(input)))
        .unwrap_or_else(|| serde_json::json!({}))
}

#[test]
fn skipping_impossible_attempts_gives_the_result_of_every_attempt() {
    let documents = [
        r#"{"path":"a\"b.txt","content":"line\nüñí é 😀","n":[1,-2.5e3,true,false,null,{"k":[]}]}"#,
        "{\"raw\":\"tab\there\nnewline\",\"ok\":1}",
        r#"{"a":1} trailing"#,
        r#"[{"a":1,"b":oops}]"#,
        r#"{"a":"x\"#,
        "1e5",
        r#""lone""#,
        "null",
        "true",
        "false",
    ];
    for document in documents {
        for end in (1..=document.len()).filter(|end| document.is_char_boundary(*end)) {
            let prefix = &document[..end];
            assert_eq!(
                parse_streaming_json(Some(prefix)),
                every_attempt(prefix),
                "{prefix:?}"
            );
        }
    }
}
