use maestro_extensions_wasm::{
    ToolCallEvent, ToolResultEvent, is_bash_tool_result, is_edit_tool_result, is_find_tool_result,
    is_grep_tool_result, is_ls_tool_result, is_read_tool_result, is_tool_call_event_type,
    is_write_tool_result,
};
use serde_json::json;

#[test]
fn tool_name_helpers_compare_only_the_name() {
    let checks: [fn(&ToolResultEvent) -> bool; 7] = [
        is_bash_tool_result,
        is_read_tool_result,
        is_edit_tool_result,
        is_write_tool_result,
        is_grep_tool_result,
        is_find_tool_result,
        is_ls_tool_result,
    ];
    let names = ["bash", "read", "edit", "write", "grep", "find", "ls"];
    let cases = [
        ("bash", [true, false, false, false, false, false, false]),
        ("read", [false, true, false, false, false, false, false]),
        ("edit", [false, false, true, false, false, false, false]),
        ("write", [false, false, false, true, false, false, false]),
        ("grep", [false, false, false, false, true, false, false]),
        ("find", [false, false, false, false, false, true, false]),
        ("ls", [false, false, false, false, false, false, true]),
        ("custom", [false; 7]),
        ("", [false; 7]),
        ("Bash", [false; 7]),
        ("bash ", [false; 7]),
    ];
    for (name, expected) in cases {
        let event = ToolCallEvent {
            tool_call_id: "call".into(),
            tool_name: name.into(),
            input: json!({"unusual": false}),
        };
        for (index, requested) in names.iter().enumerate() {
            assert_eq!(is_tool_call_event_type(requested, &event), expected[index]);
        }
        assert!(is_tool_call_event_type(name, &event));
        assert!(!is_tool_call_event_type("different", &event));
        let result = ToolResultEvent {
            tool_call_id: event.tool_call_id,
            tool_name: event.tool_name,
            input: event.input,
            content: vec![],
            is_error: true,
            details: Some(json!(null)),
        };
        for (index, check) in checks.iter().enumerate() {
            assert_eq!(check(&result), expected[index]);
        }
    }
}

#[path = "support/capabilities.rs"]
mod capabilities;

#[test]
fn bus_prefix_tail_and_unsubscribe_keep_ownership() {
    use capabilities::ControlledBus;
    use maestro_extensions_wasm::{EventBus, EventBusHandler};
    use std::{
        cell::RefCell,
        rc::Rc,
        task::{Context, Poll, Waker},
    };

    let bus = Rc::new(ControlledBus::default());
    let log = Rc::new(RefCell::new(Vec::new()));
    let inner_log = Rc::clone(&log);
    let _inner = bus
        .on(
            "inner",
            Rc::new(move |value| {
                assert_eq!(value, json!({"message": "nested"}));
                inner_log.borrow_mut().push("inner");
                Ok(None)
            }),
        )
        .unwrap();
    let callback_log = Rc::clone(&log);
    let callback_bus = Rc::clone(&bus);
    let handler: EventBusHandler = Rc::new(move |value| {
        assert_eq!(value, json!({"message": "hello", "from": "author"}));
        callback_log.borrow_mut().push("prefix");
        callback_bus.emit("inner", json!({"message": "nested"}))?;
        callback_log.borrow_mut().push("after nested");
        let tail_log = Rc::clone(&callback_log);
        Ok(Some(Box::pin(async move {
            tail_log.borrow_mut().push("tail");
            Ok(())
        })))
    });
    let unsubscribe = bus.on("message", handler).unwrap();
    bus.emit("message", json!({"message": "hello", "from": "author"}))
        .unwrap();
    log.borrow_mut().push("after emit");
    assert_eq!(
        *log.borrow(),
        ["prefix", "inner", "after nested", "after emit"]
    );
    assert_eq!(bus.tails.borrow().len(), 1);
    unsubscribe().unwrap();
    bus.emit("message", json!({"message": "ignored"})).unwrap();
    assert_eq!(bus.tails.borrow().len(), 1);
    let mut tail = bus.tails.borrow_mut().pop().unwrap();
    let mut cx = Context::from_waker(Waker::noop());
    assert!(matches!(tail.as_mut().poll(&mut cx), Poll::Ready(Ok(()))));
    assert!(bus.tails.borrow().is_empty());
    assert_eq!(
        *log.borrow(),
        ["prefix", "inner", "after nested", "after emit", "tail"]
    );

    let immediate_log = Rc::clone(&log);
    let removal = bus
        .on(
            "sync",
            Rc::new(move |_| {
                immediate_log.borrow_mut().push("sync");
                Ok(None)
            }),
        )
        .unwrap();
    drop(removal);
    bus.emit("sync", json!(null)).unwrap();
    assert_eq!(log.borrow().last(), Some(&"sync"));
    assert!(bus.tails.borrow().is_empty());
}
