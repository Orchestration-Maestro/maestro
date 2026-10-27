//! Controlled authorization and streaming for the shipped message provider.
#[path = "../../../../examples/extensions/custom-provider-anthropic/pkce.rs"]
mod pkce;

#[test]
fn custom_message_provider_pkce_matches_rfc7636() {
    let bytes = [
        116, 24, 223, 180, 151, 153, 224, 37, 79, 250, 96, 125, 216, 173, 187, 186, 22, 212, 37,
        77, 105, 214, 191, 240, 91, 88, 5, 88, 83, 132, 141, 121,
    ];
    assert_eq!(
        pkce::generate(bytes),
        (
            "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into(),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".into()
        )
    );
}
