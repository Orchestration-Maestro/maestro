#![cfg(unix)]
//! Queued input restored when an extension stops the turn and the key the queue hint
//! names, through the session's queue, the extension context and the mode's view.
use crate::interactive_mode_support::{World, corpus::cases, isolation::isolated, started};
use crate::platform_keys;
use maestro_chat::Region;
use maestro_tools::output::render_utils::strip_ansi;
use maestro_tui::KeybindingKeys;
use serde::Deserialize;
use std::{
    future::Future,
    task::{Context, Waker},
};

#[test]
fn extension_abort_restores_queued_input_exactly_as_typed() {
    const TEST: &str = "extension_abort_restores_queued_input_exactly_as_typed";
    isolated(TEST, async |directory| {
        let world = started(directory).await;
        world
            .held("run", async |held| {
                let session = world.host.session();
                session.steer("  queued  steer ", &[]).unwrap();
                session.extension_runner().create_context().abort().unwrap();
                assert_eq!(world.mode.view().editor_text(), "  queued  steer ");
                assert_eq!(session.pending_message_count(), 0);
                assert_eq!(held.aborts(), 1);
            })
            .await;
    });
}

#[test]
fn extension_abort_during_a_confirmation_restores_queued_input() {
    const TEST: &str = "extension_abort_during_a_confirmation_restores_queued_input";
    isolated(TEST, async |directory| {
        let world = started(directory).await;
        world
            .held("run", async |held| {
                let session = world.host.session();
                session.steer("queued steer", &[]).unwrap();
                let context = session.extension_runner().create_context();
                let ui = context.ui().unwrap();
                let mut confirmation =
                    std::pin::pin!(ui.confirm("Run the tool?".into(), "bash".into(), None));
                let mut observer = Context::from_waker(Waker::noop());
                assert!(
                    confirmation.as_mut().poll(&mut observer).is_pending(),
                    "the confirmation stays open"
                );
                context.abort().unwrap();
                assert_eq!(world.mode.view().editor_text(), "queued steer");
                assert_eq!(held.aborts(), 1);
            })
            .await;
    });
}

/// Every recorded queue case.
const FIXTURES: &str = include_str!("fixtures/interactive_queues.json");

/// The recorded key names.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum KeyName {
    /// One key list to capitalize.
    CapitalizeKey {
        /// The keys as bound.
        key: String,
    },
}

/// A recorded displayed key name.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Displayed {
    /// The displayed keys.
    result: String,
}

/// Queues one more steering message, which redraws the pending rows, and returns the key
/// their hint names for editing the queue.
fn hinted_key(world: &World) -> String {
    world.host.session().steer("queued", &[]).unwrap();
    let rows = world.region(Region::PendingMessages);
    let hint = rows.iter().find_map(|row| {
        let row = strip_ansi(row).unwrap();
        let key = row.trim().strip_prefix("↳ ")?;
        Some(key.strip_suffix(" to edit all queued messages")?.to_owned())
    });
    hint.unwrap_or_else(|| panic!("no queue hint in {rows:?}"))
}

#[test]
fn key_display_capitalizes_each_live_binding_without_reordering() {
    const TEST: &str = "key_display_capitalizes_each_live_binding_without_reordering";
    for (KeyName::CapitalizeKey { key }, expected) in cases::<KeyName, Displayed>(FIXTURES, TEST) {
        assert_eq!(
            maestro_tools::format_key_text(&key, true),
            platform_keys::on_this_platform(&expected.result)
        );
    }
    isolated(TEST, async |directory| {
        // The mode publishes the application's key registry, which the hint reads live.
        let world = started(directory).await;
        let keys = maestro_tui::get_keybindings();
        let bind = |keys_bound: Option<&[&str]>| {
            keys.set_user_bindings(vec![(
                "app.message.dequeue".to_owned(),
                keys_bound.map(|bound| {
                    KeybindingKeys::Multiple(bound.iter().map(|key| (*key).to_owned()).collect())
                }),
            )]);
            hinted_key(&world)
        };
        assert_eq!(bind(None), platform_keys::on_this_platform("Alt+Up"));
        assert_eq!(bind(Some(&["shift+x", "ctrl+q"])), "Shift+X/Ctrl+Q");
        assert_eq!(bind(Some(&["ctrl+q", "shift+x"])), "Ctrl+Q/Shift+X");
        assert_eq!(bind(None), platform_keys::on_this_platform("Alt+Up"));
    });
}
