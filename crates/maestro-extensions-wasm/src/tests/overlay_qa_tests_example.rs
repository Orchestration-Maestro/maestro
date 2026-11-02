//! Stream timestamps through the shipped example's feed operation and controlled host.
use super::controlled::Controlled;
use crate::{Extension, ExtensionAPI, ExtensionFuture};
use serde_json::json;

#[path = "../../../../examples/extensions/overlay_qa_tests.rs"]
mod overlay_qa_tests;

/// Runs the shipped feed operation with a fixed instant.
struct FeedTick;
impl Extension for FeedTick {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        Box::pin(async move {
            overlay_qa_tests::factory(api.clone()).await?;
            Err(overlay_qa_tests::panels::feed_line(
                &api,
                1_793_090_707_000,
                3,
            )?)
        })
    }
}

/// A date-time without a comma must not put the date into the stream timestamp.
#[test]
fn overlay_stream_timestamps_use_time_only_without_a_locale_delimiter() {
    let host = Controlled::default();
    host.observe().actions.value = json!({
        "time.local_date_time_string": "2026-10-27 09 h 25 min 07 s"
    });
    host.observe().actions.helpers.insert(
        "time.local_time_string [1793090707000]".into(),
        json!("9:25:07 AM"),
    );
    let exports = crate::component_adapter::Exports::new(host.clone());

    let result = super::block_on(exports.start::<FeedTick>());

    assert_eq!(result, Err("[9:25:07 AM] Line 3".into()));
    let names: Vec<_> = host
        .observed()
        .registrations
        .commands
        .iter()
        .map(|(name, _, _)| format!("command {name}"))
        .collect();
    super::example_support::release_all(&host, &names);
}
