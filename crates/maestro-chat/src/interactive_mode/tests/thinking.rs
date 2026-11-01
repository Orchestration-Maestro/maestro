//! Thinking commands through submitted editor input and the existing picker.
use super::actions::shows;
use super::models::register_configured;
use super::typed::{poll, recalled, submit, waiting};
use super::{World, isolated, started};
use maestro_app::{ReadonlySessionManager, ThinkingLevel, strip_ansi};
use maestro_tui::Component;

/// Makes a controlled reasoning model current without invoking a provider.
async fn reasoning(world: &World) {
    let models = register_configured(world, "reasoning", &["deep"], |config| {
        config.models.as_mut().unwrap()[0].reasoning = true;
        config.api_key = Some("fixture".into());
    });
    let session = world.host.session();
    session.set_model(models[0].clone()).await.unwrap();
    session.set_thinking_level(ThinkingLevel::Off).unwrap();
}

/// Checks the selected level, saved default and live terminal decorations.
fn applied(world: &World) {
    let session = world.host.session();
    assert_eq!(session.thinking_level(), ThinkingLevel::High);
    assert_eq!(
        session.settings_manager().borrow().get_global_settings().0["defaultThinkingLevel"],
        "high"
    );
    assert!(shows(&world.mode.chat_container, "Thinking level: high"));
    let theme = world.themes.theme().get().unwrap();
    let painted = theme.get_thinking_border_color("high")("─");
    assert!(world.mode.editor.render(80)[0].starts_with(&painted[..painted.find('─').unwrap()]));
    assert!(world.mode.footer.render(120).join("\n").contains("high"));
}

#[test]
fn thinking_command_sets_case_insensitive_level_and_persists() {
    isolated(
        "thinking_command_sets_case_insensitive_level_and_persists",
        async |directory| {
            let world = started(directory).await;
            reasoning(&world).await;
            let mut input = waiting(&world.mode);
            submit(&world.mode, "/thinking \u{feff}HiGh\u{a0}");
            assert!(poll(&mut input).is_none());
            applied(&world);
            assert!(recalled(&world.mode.editor).is_empty());
            assert!(world.mode.editor.get_text().is_empty());
        },
    );
}

#[test]
fn thinking_command_opens_picker_applies_choice_and_restores_editor() {
    isolated(
        "thinking_command_opens_picker_applies_choice_and_restores_editor",
        async |directory| {
            let world = started(directory).await;
            reasoning(&world).await;
            for command in ["/thinking", "/thinking \u{feff}"] {
                submit(&world.mode, command);
                let children = world.mode.editor_container.children().borrow().clone();
                let picker = (children[0].clone() as std::rc::Rc<dyn std::any::Any>)
                    .downcast::<crate::ThinkingSelectorComponent>()
                    .expect("the existing thinking picker is shown");
                assert!(picker.render(80).join("\n").contains("high"));
                world.terminal.send_input("\x1b");
                assert!(super::models::shows_only(
                    &world,
                    &super::models::editor(&world)
                ));
                assert!(super::models::editor_focused(&world));
            }
            assert_eq!(world.host.session().thinking_level(), ThinkingLevel::Off);
            submit(&world.mode, "/thinking");
            for _ in 0..4 {
                world.terminal.send_input("\x1b[B");
            }
            world.terminal.send_input("\r");
            applied(&world);
            assert!(super::models::shows_only(
                &world,
                &super::models::editor(&world)
            ));
            assert!(super::models::editor_focused(&world));
        },
    );
}

#[test]
fn thinking_command_rejects_levels_unavailable_for_current_model() {
    isolated(
        "thinking_command_rejects_levels_unavailable_for_current_model",
        async |directory| {
            let world = started(directory).await;
            reasoning(&world).await;
            let session = world.host.session();
            let before = session
                .session_manager()
                .borrow()
                .get_entries()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let settings = session.settings_manager().borrow().get_global_settings();
            for value in ["nonsense", "xhigh"] {
                submit(&world.mode, &format!("/thinking {value}"));
                assert!(strip_ansi(&world.mode.chat_container.render(140).concat()).unwrap().contains(&format!(
                "Unknown thinking level \"{value}\". Available levels: off, minimal, low, medium, high."
            )));
                assert_eq!(session.thinking_level(), ThinkingLevel::Off);
                assert_eq!(
                    session
                        .session_manager()
                        .borrow()
                        .get_entries()
                        .into_iter()
                        .cloned()
                        .collect::<Vec<_>>(),
                    before
                );
                assert_eq!(
                    session.settings_manager().borrow().get_global_settings().0,
                    settings.0
                );
            }
            let model = session.model();
            model.write().unwrap().reasoning = false;
            submit(&world.mode, "/thinking high");
            assert!(shows(
                &world.mode.chat_container,
                "Unknown thinking level \"high\". Available levels: off."
            ));
            assert_eq!(session.thinking_level(), ThinkingLevel::Off);
            assert!(recalled(&world.mode.editor).is_empty());
        },
    );
}
