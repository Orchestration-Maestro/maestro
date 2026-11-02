//! The tree selector's navigation hint.
use super::help;
use maestro_tui::KeybindingKeys;

/// The fold hint names the keys bound to folding and to unfolding, whatever they are.
#[test]
fn help_names_the_configured_fold_and_unfold_keys() {
    maestro_tui::set_keybindings(
        (*maestro_app::KeybindingsManager::new(Vec::default(), None)).clone(),
    );
    maestro_tui::get_keybindings().set_user_bindings(vec![
        (
            "app.tree.foldOrUp".to_owned(),
            Some(KeybindingKeys::Single("ctrl+j".to_owned())),
        ),
        (
            "app.tree.unfoldOrDown".to_owned(),
            Some(KeybindingKeys::Multiple(vec![
                "ctrl+k".to_owned(),
                "shift+k".to_owned(),
            ])),
        ),
    ]);
    assert!(
        help().contains(" ctrl+j/ctrl+k/shift+k: fold/branch. "),
        "{}",
        help()
    );
}
