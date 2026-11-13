#![cfg(unix)]
//! Terminal progress while the session answers, through the terminal and the scripted
//! provider.
use crate::interactive_mode_support::{Effects, World, isolation::isolated, started};

/// The terminal request that shows progress.
const PROGRESS_ACTIVE: &str = "\u{1b}]9;4;3\u{7}";

/// The terminal request that clears progress.
const PROGRESS_CLEAR: &str = "\u{1b}]9;4;0;\u{7}";

/// The progress requests `effects` holds, in order: shown or cleared.
fn progress(effects: &Effects) -> Vec<bool> {
    let effects = effects.borrow();
    let requests = effects.iter().filter_map(|effect| match effect.as_str() {
        PROGRESS_ACTIVE => Some(true),
        PROGRESS_CLEAR => Some(false),
        _ => None,
    });
    requests.collect()
}

/// Turns the terminal-progress setting of `world` on or off.
fn set_progress_setting(world: &World, shown: bool) {
    let session = world.host.session();
    let mut settings = session.settings_manager().borrow_mut();
    settings.set_show_terminal_progress(shown);
}

#[test]
fn response_progress_pairs_each_start_with_an_end() {
    const TEST: &str = "response_progress_pairs_each_start_with_an_end";
    isolated(TEST, async |directory| {
        let world = started(directory).await;
        world.type_text("draft");
        set_progress_setting(&world, true);
        let effects = world.effects();
        world
            .held("run", async |_| assert_eq!(progress(&effects), [true]))
            .await;
        assert_eq!(progress(&effects), [true, false]);
        // A later response pairs its own start with its own end.
        world.held("run", async |_| ()).await;
        assert_eq!(progress(&effects), [true, false, true, false]);
        // The setting alone decides whether the terminal is written at all.
        set_progress_setting(&world, false);
        let silent = world.effects();
        world.held("run", async |_| ()).await;
        assert_eq!(progress(&silent), Vec::<bool>::new());
    });
}
