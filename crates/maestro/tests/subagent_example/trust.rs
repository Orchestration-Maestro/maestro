//! Interactive confirmation applies only to requested project definitions.
#![cfg(unix)]
use super::*;
use maestro_app::{
    EventError, ExtensionBindings, ExtensionUIContext, ExtensionUIContextPort,
    ExtensionUIDialogOptions, ExtensionUIFuture,
};
use std::cell::{Cell, RefCell};

/// A controlled answer on the real application's UI binding.
#[derive(Default)]
struct Ui {
    /// Supplied trust answer.
    approve: Cell<bool>,
    /// Exact presented questions.
    calls: RefCell<Vec<(String, String)>>,
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        crate::support::theme()
    }
    fn confirm(
        &self,
        title: String,
        message: String,
        options: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<bool> {
        assert!(options.is_none_or(|options| options.timeout.is_none()));
        self.calls.borrow_mut().push((title, message));
        let answer = self.approve.get();
        Box::pin(async move { Ok::<_, EventError>(answer) })
    }
}

#[test]
fn subagent_project_agents_require_only_the_requested_interactive_trust() {
    if child_fixture("subagent_example::trust::subagent_project_agents_require_only_the_requested_interactive_trust", None).is_some() { return; }
    run(|local| {
        Box::pin(async move {
            let root = Root::new();
            controlled_role(&root, "agent/agents");
            controlled_role(&root, "project/.maestro/agents");
            let session = loaded(&root, local).await;
            let args = json!({"agent":"role","task":"inspect","agentScope":"both"});
            let headless = execute(&session, args.clone()).await;
            assert_eq!(headless["details"]["results"][0]["agentSource"], "project");
            let ui = Rc::new(Ui::default());
            session
                .bind_extensions(ExtensionBindings {
                    ui_context: Some(ExtensionUIContext::new(ui.clone()).into()),
                    ..ExtensionBindings::default()
                })
                .await
                .unwrap();
            let denied = execute(&session, args.clone()).await;
            assert_eq!(
                denied["content"][0]["text"],
                "Canceled: project-local agents not approved."
            );
            assert_eq!(denied["details"]["results"], json!([]));
            let mut explicit = args.clone();
            explicit["confirmProjectAgents"] = json!(false);
            assert_eq!(
                execute(&session, explicit).await["details"]["results"][0]["exitCode"],
                0
            );
            assert_eq!(ui.calls.borrow().len(), 1);
            ui.approve.set(true);
            assert_eq!(
                execute(&session, args).await["details"]["results"][0]["exitCode"],
                0
            );
            assert_eq!(ui.calls.borrow().len(), 2);
            assert!(
                ui.calls.borrow()[0]
                    .1
                    .contains(root.path("project/.maestro/agents").to_str().unwrap())
            );
            let user = execute(&session, json!({"agent":"role","task":"inspect"})).await;
            assert_eq!(user["details"]["results"][0]["agentSource"], "user");
            assert_eq!(ui.calls.borrow().len(), 2);
        })
    });
}
