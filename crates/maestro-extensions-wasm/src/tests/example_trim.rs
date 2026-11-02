//! Shared example trimming observes host boundaries and propagates host failures.
use serde_json::json;

use super::controlled::Controlled;
use crate::component_adapter::Exports;
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/support/trim.rs"]
mod trim;

/// Exercises borrowed slices, including the host's non-native whitespace boundaries.
struct TrimCases;
impl Extension for TrimCases {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        Box::pin(async move {
            for (text, expected) in [
                ("", ""),
                (" \t\u{feff}\n", ""),
                (" \u{feff}hé \tllo\n", "hé \tllo"),
                (" \u{85}text\u{85} ", "\u{85}text\u{85}"),
            ] {
                assert_eq!(trim::trim(&api, text)?, expected);
            }
            Ok(())
        })
    }
}

/// A rejected classification cannot silently become native trimming.
struct TrimFailure;
impl Extension for TrimFailure {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        Box::pin(async move {
            assert_eq!(trim::trim(&api, " text "), Err("classifier refused".into()));
            Ok(())
        })
    }
}

/// Uses the existing controlled adapter, without a second whitespace classifier.
#[test]
fn example_trim_uses_host_boundaries_and_propagates_errors() -> Result<(), String> {
    super::block_on(async {
        let host = Controlled::default();
        let mut replies = json!({"is_whitespace_char":false});
        for ch in [' ', '\t', '\n', '\u{feff}'] {
            replies[format!("is_whitespace_char {}", json!([ch.to_string()]))] = json!(true);
        }
        host.observe().actions.value = replies;
        let exports = Exports::new(host.clone());
        exports.start::<TrimCases>().await?;
        host.observe().actions.error = Some("classifier refused".into());
        exports.start::<TrimFailure>().await
    })
}
