//! Delivery of captured input after successful tree navigation.
use super::super::{World, actions::shows, isolated, prompts::Scripted, start, typed::submit};
use super::{
    append_user,
    tree::{finish, panel},
};
use crate::TreeSelectorComponent;
use maestro_app::messages::UserMessageDelivery;
use std::rc::Rc;

/// Selects an entry through the installed tree command.
fn select(world: &World, target: &str) {
    submit(&world.mode, "/tree");
    panel::<TreeSelectorComponent>(world)
        .unwrap()
        .get_tree_list()
        .on_select()
        .unwrap()(target);
}

/// Asserts the transcript has replayed and reported success before the next prompt starts.
fn before_prompt(world: &World) -> impl Fn() + use<> {
    let mode = Rc::downgrade(&world.mode);
    world.host.session().subscribe(Rc::new(move |event| {
        if let maestro_app::AgentSessionEvent::Agent(event) = event
            && matches!(*event.read().unwrap(), maestro_app::AgentEvent::AgentStart)
        {
            let mode = mode.upgrade().unwrap();
            assert!(shows(&mode.chat_container, "Navigated to selected point"));
            assert!(!shows(&mode.chat_container, "tail"));
        }
    }))
}

/// Successful navigation delivers the batch once after replay and status.
async fn successful_flush(scripted: &Scripted, first: &str) {
    let world = &scripted.world;
    let unsubscribe = before_prompt(world);
    scripted
        .provider
        .set_responses(vec![maestro_models::FauxResponseStep::Message(Box::new(
            maestro_models::faux_assistant_message(
                "answer",
                maestro_models::FauxAssistantMessageOptions::default(),
            ),
        ))]);
    select(world, first);
    finish(world).await;
    assert!(
        world.mode.compaction_queued_messages.borrow().is_empty(),
        "successful navigation delivers the captured batch"
    );
    assert_eq!(scripted.provider.state.call_count(), 1);
    assert!(shows(
        &world.mode.chat_container,
        "Navigated to selected point"
    ));
    assert!(shows(&world.mode.chat_container, "captured input"));
    select(world, first);
    finish(world).await;
    assert_eq!(
        scripted.provider.state.call_count(),
        1,
        "the empty second batch is inert"
    );
    unsubscribe();
}

#[test]
fn navigation_calls_compaction_flush_after_success_only() {
    isolated(
        "navigation_calls_compaction_flush_after_success_only",
        async |directory| {
            let scripted = Scripted::new(directory, crate::InteractiveModeOptions::default());
            let world = &scripted.world;
            start(world).await;
            world
                .host
                .session()
                .settings_manager()
                .borrow_mut()
                .set_global_key("branchSummary", "skipPrompt", true);
            let first = append_user(world, "first");
            let last = append_user(world, "tail");
            world.mode.compaction_queued_messages.borrow_mut().push(
                super::super::super::queues::CompactionQueuedMessage {
                    text: "captured input".to_owned(),
                    mode: UserMessageDelivery::FollowUp,
                },
            );
            select(world, &last);
            finish(world).await;
            assert_eq!(world.mode.compaction_queued_messages.borrow().len(), 1);
            assert_eq!(scripted.provider.state.call_count(), 0);
            successful_flush(&scripted, &first).await;
            scripted.finish();
        },
    );
}
