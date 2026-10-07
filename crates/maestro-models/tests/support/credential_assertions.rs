pub fn assert_credential_eq<L: PartialEq<R> + ?Sized, R: ?Sized>(
    actual: &L,
    expected: &R,
    case: &str,
) {
    assert!(actual == expected, "credential mismatch for {case}");
}

#[test]
fn credential_assertion_panic_never_contains_values() {
    let failure = std::panic::catch_unwind(|| {
        assert_credential_eq(
            &Some("controlled-actual-secret"),
            &Some("controlled-expected-secret"),
            "controlled mismatch",
        );
    })
    .unwrap_err();
    let message = failure
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| failure.downcast_ref::<&str>().copied())
        .unwrap();
    assert!(message.contains("controlled mismatch"));
    assert!(
        !message.contains("controlled-actual-secret"),
        "actual credential leaked"
    );
    assert!(
        !message.contains("controlled-expected-secret"),
        "expected credential leaked"
    );
}
