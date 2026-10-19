//! Build identity behavior.

#[test]
fn selected_build_identity_matches_configuration() {
    let channel = option_env!("MAESTRO_BUILD_CHANNEL")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("stable");
    let build_id = option_env!("MAESTRO_BUILD_ID")
        .map(str::trim)
        .filter(|value| !value.is_empty());
    assert_eq!(crate::build_info::channel(), channel);
    assert_eq!(crate::build_info::is_preview(), channel == "preview");
    assert_eq!(crate::build_info::build_id(), build_id);
    let base = env!("CARGO_PKG_VERSION");
    let expected = match (channel, build_id) {
        ("stable", _) => base.to_owned(),
        (channel, Some(id)) => format!("{base}-{channel}.{id}"),
        (channel, None) => format!("{base}-{channel}"),
    };
    assert_eq!(crate::build_info::version(), expected);
}
