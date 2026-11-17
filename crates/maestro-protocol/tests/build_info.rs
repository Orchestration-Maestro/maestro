//! Build identity.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
#![cfg(test)]
#[test]
/// Build identity exposes a nonempty version string.
fn build_version_exposes_nonempty_text() {
    assert!(!maestro_protocol::build_info::version().is_empty());
}

#[test]
fn selected_build_identity_matches_configuration() {
    let channel = option_env!("MAESTRO_BUILD_CHANNEL")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("stable");
    let build_id = option_env!("MAESTRO_BUILD_ID")
        .map(str::trim)
        .filter(|value| !value.is_empty());
    assert_eq!(maestro_protocol::build_info::channel(), channel);
    assert_eq!(
        maestro_protocol::build_info::is_preview(),
        channel == "preview"
    );
    assert_eq!(maestro_protocol::build_info::build_id(), build_id);
    let base = env!("CARGO_PKG_VERSION");
    let expected = match (channel, build_id) {
        ("stable", _) => base.to_owned(),
        (channel, Some(id)) => format!("{base}-{channel}.{id}"),
        (channel, None) => format!("{base}-{channel}"),
    };
    assert_eq!(maestro_protocol::build_info::version(), expected);
}
