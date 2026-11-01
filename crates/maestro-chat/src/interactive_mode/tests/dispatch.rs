//! The real mode through the shared startup interface.
use super::{Setup, isolated};
use maestro_app::presentation_data::mode_dispatch::InteractiveModeRunner;
use std::{
    future::Future,
    rc::Rc,
    task::{Context, Waker},
};
#[test]
fn interactive_contract_benchmark_uses_retained_mode() {
    isolated(
        "interactive_contract_benchmark_uses_retained_mode",
        async |directory| {
            let world = Setup::default().build(directory);
            let mode: Rc<dyn InteractiveModeRunner> = world.mode.clone();
            Rc::clone(&mode).init().await.unwrap();
            assert!(world.mode.is_initialized.get());
            mode.stop().unwrap();
            assert!(!world.mode.is_initialized.get());
            assert_eq!(world.terminal.events(), ["start", "stop"]);
            Rc::clone(&mode).init().await.unwrap();
            assert!(world.mode.is_initialized.get());
            mode.stop().unwrap();
            assert!(!world.mode.is_initialized.get());
            assert_eq!(world.terminal.events(), ["start", "stop", "start", "stop"]);
        },
    );
}
#[test]
fn interactive_contract_run_drives_existing_prompt_loop() {
    isolated(
        "interactive_contract_run_drives_existing_prompt_loop",
        async |directory| {
            let world = Setup {
                offline: Some(true),
                ..Setup::default()
            }
            .build(directory);
            let mode: Rc<dyn InteractiveModeRunner> = world.mode.clone();
            let mut run = Rc::clone(&mode).run();
            let result = Future::poll(run.as_mut(), &mut Context::from_waker(Waker::noop()));
            assert!(result.is_pending(), "{result:?}");
            super::prompts::until_waiting(&world.mode, std::pin::Pin::new(&mut run)).await;
            assert!(world.mode.is_initialized.get());
            assert!(world.mode.on_input_callback.borrow().is_some());
            drop(run);
            mode.stop().unwrap();
            assert!(!world.mode.is_initialized.get());
        },
    );
}
