//! Inline expansions through the shipped component, native execution and a disposable cwd.
#![cfg(test)]
#![cfg(unix)]
use maestro_extensions::{
    ExtensionRunner, ExtensionUIContext, Local, create_event_bus_for, create_extension_runtime,
    load_extension_from_factory,
    types::{ExtensionUIContextPort, InputEventResult, InputSource, NotificationType},
};
use maestro_session::Presence;
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, sync::RwLock};

/// Records notifications without a terminal or a live provider.
#[derive(Default)]
struct Ui(RefCell<Vec<Value>>);
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn notify(
        &self,
        message: String,
        kind: Option<NotificationType>,
    ) -> Result<(), maestro_extensions::EventError> {
        self.0.borrow_mut().push(json!([message, kind]));
        Ok(())
    }
}

/// Loads the compiled shipped artifact on the real runner without model operations.
async fn loaded(cwd: &str) -> (ExtensionRunner<Local>, Rc<Ui>) {
    let path = std::env::var("MAESTRO_INLINE_BASH_COMPONENT").expect("nextest component setup");
    let runtime = create_extension_runtime::<Local>();
    let bus = create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no background work")),
        Rc::new(|error| panic!("{error}")),
    );
    let owner = load_extension_from_factory(
        crate::artifact(path.clone()),
        cwd.into(),
        bus.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session(cwd).0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    let _remove = runner.on_error(Rc::new(|error| panic!("{error:?}")));
    let ui = Rc::new(Ui::default());
    runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
    (runner, ui)
}

/// Runs local component callbacks on a caller-owned executor.
fn run(action: impl std::future::Future<Output = ()>) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, action);
}

/// Observes the runner's public input result and verifies attachment preservation.
async fn input(runner: &ExtensionRunner<Local>, text: &str) -> InputEventResult {
    let mut images: Presence<Vec<maestro_models::ImageContent>> =
        serde_json::from_value(json!([{"type":"image","data":"YQ==","mimeType":"image/png"}]))
            .unwrap();
    let original = images.clone();
    let result = runner
        .emit_input(text, &mut images, InputSource::Interactive)
        .await
        .unwrap();
    assert_eq!(images, original);
    result
}

/// Literal arguments, repeated matches and relative reads are executed in order at the session cwd.
#[test]
fn inline_bash_expands_quoted_commands_in_the_session_directory() {
    run(async {
        let root = Scratch::new().unwrap();
        std::fs::write(root.path().join("input.txt"), "  from cwd\n").unwrap();
        let (runner, ui) = loaded(root.path().to_str().unwrap()).await;
        let result = input(
            &runner,
            "A !{printf '%s' 'quoted space'} B !{cat input.txt} C !{printf same} / !{printf same}",
        )
        .await;
        let InputEventResult::Transform(transform) = result else {
            panic!("expected expanded input")
        };
        assert_eq!(transform.text, "A quoted space B from cwd C same / same");
        assert!(matches!(transform.images, Presence::Present(_)));
        assert_eq!(
            *ui.0.borrow(),
            [json!([
                "Expanded 4 inline command(s):\n!{printf '%s' 'quoted space'} -> \"quoted space\"\n!{cat input.txt} -> \"from cwd\"\n!{printf same} -> \"same\"\n!{printf same} -> \"same\"",
                "info"
            ])]
        );
        let result = input(
            &runner,
            "!{printf '\\041\\173printf same\\175'} / !{printf same}",
        )
        .await;
        let InputEventResult::Transform(transform) = result else {
            panic!("expected expanded input")
        };
        assert_eq!(transform.text, "!{printf same} / same");
    });
}

/// Whole-line shell input stays untouched; malformed expressions do not start a child.
#[test]
fn inline_bash_preserves_whole_line_commands_and_expands_without_ui() {
    run(async {
        let root = Scratch::new().unwrap();
        let (runner, ui) = loaded(root.path().to_str().unwrap()).await;
        for text in [
            "!echo !{touch sentinel}",
            "\u{feff}\t !echo !{touch sentinel}",
            "plain input",
            "",
            "!{}",
            "before !{printf unfinished",
        ] {
            assert!(
                matches!(input(&runner, text).await, InputEventResult::Continue),
                "{text}"
            );
        }
        assert!(!root.path().join("sentinel").exists());
        assert!(ui.0.borrow().is_empty());
        runner.set_ui_context(None);
        for source in [
            InputSource::Interactive,
            InputSource::Rpc,
            InputSource::Extension,
        ] {
            let result = runner
                .emit_input("!{printf headless}", &mut Presence::Missing, source)
                .await
                .unwrap();
            let InputEventResult::Transform(transform) = result else {
                panic!("expected expanded input")
            };
            assert_eq!(transform.text, "headless");
            assert_eq!(transform.images, Presence::Missing);
        }
        assert!(ui.0.borrow().is_empty());
    });
}
