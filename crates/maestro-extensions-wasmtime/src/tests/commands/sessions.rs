//! Session-changing requests preserve outer records, raw operands and native failures.
use crate::{
    command_reads::Request,
    context_reads::{Endpoint, Owner},
    local_resources::Pump,
    session_calls::{NewSession, NewSessionData, Switch},
};
use maestro_extensions::ExtensionCommandContextActions;
use std::{cell::RefCell, rc::Rc};

#[test]
fn session_operations_keep_absent_empty_and_full_options() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(sessions());
}

/// Drives the sole receiver while the Store-side endpoint waits for native results.
async fn sessions() {
    let runner = crate::tests::runner("/sessions");
    let calls = Rc::new(RefCell::new(Vec::new()));
    runner.bind_command_context(Some(actions(&calls)));
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let endpoint = pump
        .registry()
        .context(Owner::Command(runner.create_command_context()))
        .unwrap();
    let worker = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(requests(endpoint));
    });
    drop(admission);
    pump.serve().await;
    worker.join().unwrap();
    assert_eq!(
        *calls.borrow(),
        [
            "new:absent",
            "new:empty",
            "new: /parent\u{feff} ",
            "fork:absent",
            "fork:empty",
            "fork:before",
            "fork:at",
            "switch:absent",
            "switch:empty"
        ]
    );
}

/// Supplies optional records independently of native operation defaults.
async fn requests(endpoint: Endpoint) {
    new_sessions(&endpoint).await;
    forks(&endpoint).await;
    for options in [None, Some(Switch { with_session: None })] {
        assert!(
            endpoint
                .command(|reply| Request::Switch(" /session\u{85} ".into(), options, reply))
                .await
                .unwrap()
                .cancelled
        );
    }
}

/// Preserves both levels of new-session presence without a placeholder callback.
async fn new_sessions(endpoint: &Endpoint) {
    for data in [
        None,
        Some(NewSessionData {
            parent_session: None,
        }),
        Some(NewSessionData {
            parent_session: Some(" /parent\u{feff} ".into()),
        }),
    ] {
        let options = NewSession {
            data,
            setup: None,
            with_session: None,
        };
        assert!(
            endpoint
                .command(|reply| Request::NewSession(options, reply))
                .await
                .unwrap()
                .cancelled
        );
    }
}

/// Sends every fork choice and rejects malformed data before invoking native work.
async fn forks(endpoint: &Endpoint) {
    for data in [
        None,
        Some("{}"),
        Some(r#"{"position":"before"}"#),
        Some(r#"{"position":"at"}"#),
    ] {
        assert_eq!(
            endpoint
                .command(|reply| Request::Fork(
                    "  e\u{feff}\u{85} ".into(),
                    data.map(str::to_owned),
                    None,
                    reply
                ))
                .await
                .unwrap_err(),
            "native fork failure"
        );
    }
    assert!(
        endpoint
            .command(|reply| Request::Fork("ignored".into(), Some("not JSON".into()), None, reply))
            .await
            .is_err()
    );
}

/// Checks native operands at consumption rather than an adapter-generated observation.
fn actions(calls: &Rc<RefCell<Vec<String>>>) -> ExtensionCommandContextActions {
    let new_calls = calls.clone();
    let fork_calls = calls.clone();
    let switch_calls = calls.clone();
    ExtensionCommandContextActions::new(maestro_extensions::CommandContextOperations {
        new_session: Rc::new(move |options| {
            let value = options.map_or_else(
                || "absent".to_owned(),
                |options| {
                    assert!(options.setup.is_none() && options.with_session.is_none());
                    options.parent_session.unwrap_or_else(|| "empty".into())
                },
            );
            new_calls.borrow_mut().push(format!("new:{value}"));
            Box::pin(async {
                Ok(maestro_extensions::types::SessionChangeResult { cancelled: true })
            })
        }),
        fork: Rc::new(move |id, options| {
            assert_eq!(id, "  e\u{feff}\u{85} ");
            let value = options.map_or("absent", |options| {
                assert!(options.with_session.is_none());
                match options.position {
                    None => "empty",
                    Some(maestro_extensions::ForkPosition::Before) => "before",
                    Some(maestro_extensions::ForkPosition::At) => "at",
                }
            });
            fork_calls.borrow_mut().push(format!("fork:{value}"));
            Box::pin(async { Err("native fork failure".into()) })
        }),
        switch_session: Rc::new(move |path, options| {
            assert_eq!(path, " /session\u{85} ");
            let value = options.map_or("absent", |options| {
                assert!(options.with_session.is_none());
                "empty"
            });
            switch_calls.borrow_mut().push(format!("switch:{value}"));
            Box::pin(async {
                Ok(maestro_extensions::types::SessionChangeResult { cancelled: true })
            })
        }),
        ..crate::context_actions::silent_command_operations()
    })
}
