//! Field admission and numeric domains.

#[test]
fn wire_field_domains_match_payloads() {
    super::corpus("wire_field_domains_match_payloads", 1592);
}

#[test]
fn wire_integer_boundaries_preserve_numeric_values() {
    for (spelling, output) in [
        ("-0", Some("0")),
        ("1.0", Some("1")),
        ("9007199254740991", Some("9007199254740991")),
        ("9007199254740992", None),
        ("-9007199254740991", Some("-9007199254740991")),
        ("-9007199254740992", None),
        ("1.5", None),
        ("1e300", None),
    ] {
        let number: serde_json::Value = serde_json::from_str(spelling).unwrap();
        let input =
            serde_json::json!({"model":"number-boundary", "messages":[], "maxTokens":number});
        let expected = output.map_or(super::Expected::Error { error: true }, |number| super::Expected::Json {
            json: format!("{{\"model\":\"number-boundary\",\"max_tokens\":{number},\"stream\":true,\"messages\":[]}}"),
        });
        super::check(&input, &expected);
    }
}
