//! Replacement-session consumers through the installed navigation commands.
use super::super::{
    Setup, World, actions::shows, isolated, models::register, start, typed::submit,
};
use super::{append_user, fork_panel, tree::finish};
use maestro_app::{ForkOptions, ForkPosition};
use maestro_tui::Component;
use std::{
    rc::Rc,
    sync::{Arc, RwLock},
};

/// The rendered footer after changing only the current session model.
async fn changed_footer(world: &World, model: Arc<RwLock<maestro_app::Model>>) {
    let new = world.host.session();
    new.set_model(model).await.unwrap();
    let rows = maestro_app::strip_ansi(&world.mode.footer.render(120).join("\n")).unwrap();
    assert!(
        rows.contains("changed-model"),
        "footer retained a retired session: {rows}"
    );
    assert!(
        !rows.contains("original-model"),
        "retired model remains: {rows}"
    );
}

#[test]
fn navigation_footer_follows_the_replaced_session_model() {
    isolated(
        "navigation_footer_follows_the_replaced_session_model",
        async |directory| {
            let world = Setup::default().build(directory);
            start(&world).await;
            let models = register(&world, "navigation", &["original-model", "changed-model"]);
            for command in ["/clone", "/fork"] {
                world
                    .host
                    .session()
                    .set_model(Arc::clone(&models[0]))
                    .await
                    .unwrap();
                append_user(&world, "first");
                let last = append_user(&world, "second");
                let retired = world.host.session();
                submit(&world.mode, command);
                if command == "/fork" {
                    fork_panel(&world)
                        .unwrap()
                        .get_message_list()
                        .on_select()
                        .unwrap()(&last);
                }
                finish(&world).await;
                assert!(!Rc::ptr_eq(&retired, &world.host.session()));
                changed_footer(&world, Arc::clone(&models[1])).await;
                assert!(shows(
                    &world.mode.chat_container,
                    if command == "/clone" {
                        "Cloned to new session"
                    } else {
                        "Forked to new session"
                    }
                ));
            }
            let weak = Rc::downgrade(&world.mode);
            drop(world.mode);
            assert!(weak.upgrade().is_none());
        },
    );
}

#[test]
fn runtime_replacement_rebinds_before_its_continuation() {
    isolated(
        "runtime_replacement_rebinds_before_its_continuation",
        async |directory| {
            let world = Setup::default().build(directory);
            start(&world).await;
            let models = register(&world, "navigation", &["original-model", "changed-model"]);
            world
                .host
                .session()
                .set_model(Arc::clone(&models[0]))
                .await
                .unwrap();
            let leaf = append_user(&world, "kept");
            let footer = Rc::clone(&world.mode.footer);
            let changed = Arc::clone(&models[1]);
            let runtime = Rc::clone(&world.host);
            let options = ForkOptions {
                position: Some(ForkPosition::At),
                with_session: Some(Rc::new(move |context| {
                    let session = runtime.session();
                    assert!(Arc::ptr_eq(
                        &context.model().unwrap().unwrap(),
                        &session.model()
                    ));
                    let changed = Arc::clone(&changed);
                    let footer = Rc::clone(&footer);
                    Box::pin(async move {
                        session.set_model(changed).await?;
                        let rows = maestro_app::strip_ansi(&footer.render(120).join("\n")).unwrap();
                        assert!(
                            rows.contains("changed-model"),
                            "root rebind must precede continuation: {rows}"
                        );
                        Ok(())
                    })
                })),
            };
            assert!(!world.host.fork(&leaf, options).await.unwrap().cancelled);
        },
    );
}
