//! The shipped overlay command exercised through the author interface.
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/doom-overlay/main.rs"]
mod overlay;

/// The factory exported by the shipped component.
struct DoomOverlay;
impl Extension for DoomOverlay {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        overlay::factory(api)
    }
}

/// The command is opt-in, not an automatically opened game.
#[test]
fn doom_overlay_registers_an_opt_in_command() -> Result<(), String> {
    super::block_on(async {
        let (host, _) = super::example_support::start::<DoomOverlay>().await?;
        assert_eq!(host.observed().registrations.commands, [("doom-overlay".into(),
            Some("Play DOOM in an overlay. Q pauses and closes it.".into()), false)]);
        Ok(())
    })
}
