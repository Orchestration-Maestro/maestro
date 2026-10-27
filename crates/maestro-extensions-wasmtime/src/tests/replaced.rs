//! Replacement messages keep their original session after guarded reads retire.
use crate::read_imports::ReadEndpoint;
use crate::{
    context_reads::Request as Read, local_resources::Pump, replaced_reads::Request, tests::runner,
};
use maestro_extensions::ReplacedSessionContext;
use std::{cell::RefCell, rc::Rc};

#[test]
fn replacement_messages_keep_original_session_while_command_view_becomes_stale() {
    tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {
        let runner = runner("/replacement");
        let calls = Rc::new(RefCell::new(Vec::new()));
        let custom = calls.clone();
        let users = calls.clone();
        let replacement = ReplacedSessionContext::new(runner.create_command_context(), Rc::new(move |message, options| {
            custom.borrow_mut().push(serde_json::json!([message, options]));
            Box::pin(async { Ok(()) })
        }), Rc::new(move |content, options| {
            users.borrow_mut().push(serde_json::json!([content, options]));
            Box::pin(async { Err("replacement user failure".into()) })
        }));
        let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
        let endpoint = pump.registry().replaced(replacement).unwrap();
        let first = endpoint.clone();
        let worker = std::thread::spawn(move || first.command().unwrap());
        assert!(pump.answer_next().await);
        let command = worker.join().unwrap();
        let worker = std::thread::spawn(move || { assert_eq!(command.read(Read::Cwd).unwrap(), "/replacement"); command });
        assert!(pump.answer_next().await);
        let command = worker.join().unwrap();
        runner.invalidate(Some("retired replacement".into()));
        let worker = std::thread::spawn(move || tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {
            assert_eq!(command.read(Read::Cwd).unwrap_err(), "retired replacement");
            drop(command);
            endpoint.send(|reply| Request::Message(r#"{"customType":" original ","content":"","display":false}"#.into(), Some("{}".into()), reply)).await.unwrap();
            assert_eq!(endpoint.send(|reply| Request::UserMessage(r#"" original ""#.into(), None, reply)).await.unwrap_err(), "replacement user failure");
        }));
        drop(admission);
        pump.serve().await;
        worker.join().unwrap();
        assert_eq!(*calls.borrow(), [serde_json::json!([{"customType":" original ","content":"","display":false}, {}]), serde_json::json!([" original ", null])]);
    });
}
