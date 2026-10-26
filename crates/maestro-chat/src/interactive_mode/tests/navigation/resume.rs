//! Saved-session discovery, selection, rename and resume through the installed command.
use super::super::{
    Setup, World, actions::shows, isolated, isolated_output, lifecycle::Effects, models::editor,
    models::shows_only, start, typed::submit,
};
use super::{append_user, fork_panel, tree::finish, tree::panel};
use crate::components::session_selector::{SessionList, SessionSelectorComponent};
use maestro_app::{ReadonlySessionManager, SessionManager};
use maestro_tui::{Component, tui::InputHandler};
use serde_json::json;
use std::{path::Path, rc::Rc};

/// Writes a saved session holding one user message and returns its path.
pub(in crate::interactive_mode::tests) fn saved(
    directory: &Path,
    id: &str,
    cwd: &serde_json::Value,
    text: &str,
) -> String {
    std::fs::create_dir_all(directory).unwrap();
    let header = json!({
        "type": "session", "version": 3, "id": id,
        "timestamp": "2026-01-02T03:04:05.006Z", "cwd": cwd,
    });
    let message = json!({
        "type": "message", "id": "a1b2c3d4", "parentId": null,
        "timestamp": "2026-01-02T03:04:05.006Z",
        "message": {"role": "user", "content": text, "timestamp": 5},
    });
    let file = directory.join(format!("{id}.jsonl"));
    std::fs::write(&file, format!("{header}\n{message}\n")).unwrap();
    file.to_str().unwrap().to_owned()
}

/// A started mode whose session history lives in `here`, with one other saved session
/// there and one in another project of the same agent directory.
async fn resumable(directory: &Path, effects: &Rc<Effects>) -> (World, String) {
    let here = directory.join("here");
    std::fs::create_dir_all(&here).unwrap();
    let world = Setup {
        lifecycle: effects.clone(),
        session_history: Some(here.clone()),
        ..Setup::default()
    }
    .build(directory);
    start(&world).await;
    append_user(&world, "current question");
    let cwd = json!(directory.to_str().unwrap());
    let other = saved(&here, "other", &cwd, "resumed question");
    saved(
        &directory.join("sessions").join("--elsewhere--"),
        "far",
        &cwd,
        "far question",
    );
    (world, other)
}

/// Opens the session selector and settles its current-scope discovery.
async fn opened(world: &World) -> (Rc<SessionSelectorComponent>, Rc<SessionList>) {
    submit(&world.mode, "/resume");
    let selector = panel::<SessionSelectorComponent>(world).expect("the session selector");
    finish(world).await;
    let list = selector.get_session_list();
    (selector, list)
}

/// The selector's plain rendered rows.
fn rows(selector: &SessionSelectorComponent) -> String {
    maestro_app::strip_ansi(&selector.render(120).join("\n")).unwrap()
}

#[test]
fn resume_selector_forwards_discovery_rename_close_and_exit() {
    isolated(
        "resume_selector_forwards_discovery_rename_close_and_exit",
        async |directory| {
            let effects = Rc::new(Effects::default());
            let (world, other) = resumable(directory, &effects).await;
            let (selector, list) = opened(&world).await;
            let listed = rows(&selector);
            assert!(listed.contains("resumed question"), "{listed}");
            assert!(!listed.contains("far question"), "{listed}");
            selector.handle_input("\t");
            finish(&world).await;
            assert!(
                rows(&selector).contains("far question"),
                "all projects are listed"
            );
            list.on_select().unwrap()(&other);
            assert!(
                panel::<SessionSelectorComponent>(&world).is_none(),
                "closes first"
            );
            finish(&world).await;
            assert_eq!(world.host.session().session_file(), Some(other.clone()));
            assert!(shows(&world.mode.chat_container, "resumed question"));
            assert!(shows(&world.mode.chat_container, "Resumed session"));

            let (_, list) = opened(&world).await;
            list.on_cancel().unwrap()();
            assert!(shows_only(&world, &editor(&world)));
            assert_eq!(world.host.session().session_file(), Some(other));
            assert!(world.runtime.drain_futures().is_empty());

            let (_, list) = opened(&world).await;
            list.on_exit()();
            finish(&world).await;
            assert!(
                effects
                    .events
                    .borrow()
                    .iter()
                    .any(|event| event == "exit:0")
            );
        },
    );
}

/// The stored name of the session file at `path`.
fn stored_name(path: &str) -> Option<String> {
    let manager = SessionManager::open(path, None, None).unwrap();
    manager.get_session_name().unwrap().map(str::to_owned)
}

#[test]
fn resume_rename_reopens_the_selected_file() {
    isolated(
        "resume_rename_reopens_the_selected_file",
        async |directory| {
            for (typed, name) in [
                ("", None),
                ("   ", None),
                ("\u{feff}", None),
                ("\u{85}", Some("\u{85}")),
                ("  Ünïcode name\u{feff} ", Some("Ünïcode name")),
            ] {
                let effects = Rc::new(Effects::default());
                let (world, other) = resumable(directory, &effects).await;
                let current = world.host.session().session_file().unwrap();
                let (selector, list) = opened(&world).await;
                list.on_rename_session().unwrap()(&other);
                selector.handle_input(&format!("\x1b[200~{typed}\x1b[201~"));
                selector.handle_input("\r");
                finish(&world).await;
                assert_eq!(stored_name(&other).as_deref(), name, "{typed:?}");
                assert_eq!(stored_name(&current), None, "the current file is untouched");
                assert_eq!(world.host.session().session_file(), Some(current));
            }
        },
    );
}

#[test]
fn navigation_async_callbacks_keep_the_original_error_route() {
    let test = "navigation_async_callbacks_keep_the_original_error_route";
    let output = isolated_output(test, "fatal", &[], "trap '' XFSZ", async |directory| {
        let effects = Rc::new(Effects::default());
        let (world, _) = resumable(directory, &effects).await;
        let broken = saved(&directory.join("here"), "broken", &json!(5), "broken");
        let (_, list) = opened(&world).await;
        list.on_select().unwrap()(&broken);
        println!("\nPROBE resume started");
        finish(&world).await;
        println!("\nPROBE resume survived");
    });
    if let Some(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("PROBE resume started"), "{stdout}");
        assert!(!stdout.contains("PROBE resume survived"), "{stdout}");
        assert_eq!(output.status.code(), Some(1), "{output:?}");
    }
}

/// Runs the replacing `command`, choosing `last` to fork before or `other` to resume.
async fn replace(world: &World, command: &str, last: &str, other: &str) {
    submit(&world.mode, command);
    match command {
        "/fork" => fork_panel(world)
            .unwrap()
            .get_message_list()
            .on_select()
            .unwrap()(last),
        "/resume" => {
            let selector = panel::<SessionSelectorComponent>(world).unwrap();
            finish(world).await;
            selector.get_session_list().on_select().unwrap()(other);
        }
        _ => {}
    }
    finish(world).await;
}

/// Checks that nothing of the replaced session's captured input reaches the editor,
/// the pending rows or the next prompt the replacement sends.
async fn discarded(scripted: &super::super::prompts::Scripted, command: &str) {
    let world = &scripted.world;
    assert!(
        world.mode.compaction_queued_messages.borrow().is_empty(),
        "{command}"
    );
    world.mode.handle_dequeue().unwrap();
    assert!(shows(
        &world.mode.chat_container,
        "No queued messages to restore"
    ));
    assert!(!world.mode.editor.get_text().contains("stale"), "{command}");
    assert!(world.mode.pending_messages_container.render(80).is_empty());
    scripted.adopt(&world.host.session());
    scripted
        .provider
        .set_responses(vec![maestro_models::FauxResponseStep::Message(Box::new(
            maestro_models::faux_assistant_message(
                "answer",
                maestro_models::FauxAssistantMessageOptions::default(),
            ),
        ))]);
    let session = world.host.session();
    session
        .prompt("fresh prompt", maestro_app::PromptOptions::default())
        .await
        .unwrap();
    let sent = serde_json::to_string(&*session.messages().read().unwrap()).unwrap();
    assert!(sent.contains("fresh prompt"), "{command}: {sent}");
    assert!(
        !sent.contains("stale compaction input"),
        "{command}: {sent}"
    );
}

#[test]
fn navigation_replacement_discards_old_compaction_input() {
    isolated(
        "navigation_replacement_discards_old_compaction_input",
        async |directory| {
            let scripted = super::super::prompts::Scripted::new(
                directory,
                crate::InteractiveModeOptions::default(),
            );
            let world = &scripted.world;
            start(world).await;
            let cwd = json!(directory.to_str().unwrap());
            let other = saved(&directory.join("saved"), "other", &cwd, "saved question");
            for command in ["/clone", "/fork", "/resume"] {
                let last = append_user(world, &format!("before {command}"));
                world.mode.compaction_queued_messages.borrow_mut().push(
                    super::super::super::queues::CompactionQueuedMessage {
                        text: "stale compaction input".to_owned(),
                        mode: maestro_app::messages::UserMessageDelivery::Steer,
                    },
                );
                replace(world, command, &last, &other).await;
                discarded(&scripted, command).await;
            }
            assert_eq!(
                world.host.session().session_file().as_deref(),
                Some(other.as_str())
            );
            scripted.finish();
        },
    );
}
