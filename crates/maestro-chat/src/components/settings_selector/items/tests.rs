//! The setting descriptions that name configured keys.

use super::follow_up_description;

/// The description names the key it is given, not a fixed key.
#[test]
fn follow_up_description_names_the_configured_key() {
    for key in ["Alt+Enter", "Ctrl+J", "Option+Enter/Shift+Enter"] {
        assert_eq!(
            follow_up_description(key),
            format!(
                "{key} queues follow-up messages until the agent stops. 'one-at-a-time': one message at a time, each waiting for its response. 'all': everything at once."
            )
        );
    }
}
