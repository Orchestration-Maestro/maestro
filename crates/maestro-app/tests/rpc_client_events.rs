//! The RPC client's event callbacks, its deadlines and the work that a stop or the child's
//! end settles.
#![cfg(unix)]
use crate::rpc_peer::{Peer, SERVE, events, hold_clock, next_of};
use crate::support::rpc_connection::poll_once;
use crate::support::session_harness::{on_paused_task_set, on_task_set};
use maestro_app::{RpcClient, RpcClientError, RpcEventListener};
use maestro_process::child_process::process_exists;
use serde_json::{Value, json};
use std::{cell::RefCell, pin::pin, rc::Rc, task::Poll, time::Duration};

/// Emits each record of `PEER_RECORDS` once a command arrives, after answering it.
const BURST: &str = r"
for line in commands():
    respond(json.loads(line))
    for record in json.loads(os.environ['PEER_RECORDS']):
        emit(record)
";

/// A client of `peer` running [`BURST`] with `records`.
fn bursting(peer: &Peer, records: &Value) -> RpcClient {
    let mut options = peer.options(BURST);
    options
        .env
        .insert("PEER_RECORDS".into(), records.to_string());
    RpcClient::new(options)
}

/// A callback that records `label` and the event type, then reports `failure` if any.
fn recording(log: &Rc<RefCell<Vec<String>>>, label: &'static str) -> RpcEventListener {
    let log = Rc::clone(log);
    Rc::new(move |event: &Value| {
        log.borrow_mut()
            .push(format!("{label}:{}", event["type"].as_str().unwrap()));
        Ok(())
    })
}

/// Stops `client`, or makes its child exit.
async fn end_child(client: &RpcClient, ending: &str) {
    if ending == "stop" {
        client.stop().await.unwrap();
    } else {
        let _ = client.bash("exit").await;
    }
}

/// A callback that fails on `agent_end`.
fn failing_at_end() -> RpcEventListener {
    Rc::new(|event: &Value| {
        if event["type"] == "agent_end" {
            return Err(RpcClientError::Response("listener failed".into()));
        }
        Ok(())
    })
}

/// Removal operations a callback runs.
type Removers = Rc<RefCell<Vec<Box<dyn Fn()>>>>;

#[test]
fn event_registrations_keep_duplicates_and_stop_on_error() {
    let peer = Peer::new();
    let records = json!([{"type": "one"}, {"type": "agent_end"}, {"type": "two"}]);
    on_task_set(|_| async {
        let client = Rc::new(bursting(&peer, &records));
        let log = Rc::new(RefCell::new(Vec::new()));
        let twice = recording(&log, "twice");
        let remove_first = client.on_event(Rc::clone(&twice));
        let _remove_second = client.on_event(twice);
        let (weak, added_log, added) = (
            Rc::downgrade(&client),
            Rc::clone(&log),
            Rc::new(RefCell::new(None)),
        );
        let slot = Rc::clone(&added);
        let _adder = client.on_event(Rc::new(move |event: &Value| {
            if event["type"] == "one" && slot.borrow().is_none() {
                let remove = weak
                    .upgrade()
                    .unwrap()
                    .on_event(recording(&added_log, "added"));
                *slot.borrow_mut() = Some(remove);
            }
            Ok(())
        }));
        let _failing = client.on_event(failing_at_end());
        let _after = client.on_event(recording(&log, "after"));
        let mut events = events(&client);
        client.start().await.unwrap();
        let idle = client.wait_for_idle(Some(Duration::from_millis(300)));
        client.abort().await.unwrap();
        let idle = idle.await;
        assert!(
            matches!(idle, Err(RpcClientError::IdleTimeout { .. })),
            "the failure hid the terminal event"
        );
        let first_line = ["twice:one", "twice:one", "after:one", "added:one"];
        let ending = ["twice:agent_end", "twice:agent_end"];
        let last_line = ["twice:two", "twice:two", "after:two", "added:two"];
        assert_eq!(
            *log.borrow(),
            [&first_line[..], &ending, &last_line].concat()
        );
        remove_first();
        remove_first();
        log.borrow_mut().clear();
        while events.try_recv().is_ok() {}
        client.abort().await.unwrap();
        next_of(&mut events, "two").await;
        let expected = [
            "twice:one",
            "after:one",
            "added:one",
            "twice:agent_end",
            "twice:two",
            "after:two",
            "added:two",
        ];
        assert_eq!(*log.borrow(), expected);
        client.stop().await.unwrap();
    });
}

#[test]
fn listener_removal_keeps_the_next_registration() {
    let peer = Peer::new();
    let records = json!([{"type": "one"}, {"type": "two"}]);
    on_task_set(|_| async {
        let client = bursting(&peer, &records);
        let log = Rc::new(RefCell::new(Vec::new()));
        let removers: Removers = Rc::default();
        let (first_log, first_removers) = (Rc::clone(&log), Rc::clone(&removers));
        let first = client.on_event(Rc::new(move |event: &Value| {
            first_log
                .borrow_mut()
                .push(format!("first:{}", event["type"].as_str().unwrap()));
            // Removes itself, then the third registration.
            for remove in first_removers.borrow().iter() {
                remove();
            }
            Ok(())
        }));
        let second = client.on_event(recording(&log, "second"));
        let third = client.on_event(recording(&log, "third"));
        removers.borrow_mut().push(Box::new(first));
        removers.borrow_mut().push(Box::new(third));
        let mut events = events(&client);
        client.start().await.unwrap();
        client.abort().await.unwrap();
        next_of(&mut events, "two").await;
        assert_eq!(*log.borrow(), ["first:one", "second:one", "second:two"]);
        second();
        client.stop().await.unwrap();
    });
}

#[test]
fn request_deadline_keeps_exact_boundary_and_stderr() {
    let peer = Peer::new();
    let script = r"
os.write(2, b'diag\n')
emit({'type': 'peer_ready'})
lines = commands()
state = json.loads(next(lines))
emit({'type': 'received'})
abort = json.loads(next(lines))
respond(abort)
respond(state, {'thinkingLevel': 'off', 'isStreaming': False, 'isCompacting': False, 'steeringMode': 'all', 'followUpMode': 'all', 'sessionId': 's', 'autoCompactionEnabled': False, 'messageCount': 0, 'pendingMessageCount': 0})
sys.stdin.read()
";
    on_paused_task_set(|_| async {
        let client = RpcClient::new(peer.options(script));
        let mut events = events(&client);
        client.start().await.unwrap();
        next_of(&mut events, "peer_ready").await;
        let (release, hold) = hold_clock();
        while client.get_stderr().is_empty() {
            tokio::task::yield_now().await;
        }
        let mut state = pin!(client.get_state());
        assert!(poll_once(state.as_mut()).is_pending());
        next_of(&mut events, "received").await;
        tokio::time::advance(Duration::from_millis(29_999)).await;
        // Each turn lets a deadline that has come run before the request is looked at.
        tokio::task::yield_now().await;
        assert!(poll_once(state.as_mut()).is_pending());
        tokio::time::advance(Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        let Poll::Ready(Err(error)) = poll_once(state.as_mut()) else {
            panic!("the request times out at 30 seconds");
        };
        assert!(
            matches!(&error, RpcClientError::RequestTimeout { command, .. } if command == "get_state")
        );
        assert_eq!(
            error.to_string(),
            "Timeout waiting for response to get_state. Stderr: diag\n"
        );
        client.abort().await.unwrap();
        let late = next_of(&mut events, "response").await;
        assert_eq!(
            (late["id"].as_str(), late["command"].as_str()),
            (Some("req_1"), Some("get_state"))
        );
        release.send(()).unwrap();
        hold.await.unwrap();
        client.stop().await.unwrap();
    });
}

#[test]
fn request_deadlines_hold_through_a_delayed_first_poll() {
    let peer = Peer::new();
    let script = r"
emit({'type': 'peer_ready'})
lines = commands()
next(lines)
sys.stdin.read()
";
    on_paused_task_set(|_| async {
        let client = RpcClient::new(peer.options(script));
        let mut events = events(&client);
        client.start().await.unwrap();
        next_of(&mut events, "peer_ready").await;
        let (release, hold) = hold_clock();
        let mut request = pin!(client.abort());
        assert!(poll_once(request.as_mut()).is_pending());
        // The deadline's task first runs only after the caller's own work has taken the
        // executor past 29,999 of its 30,000 milliseconds.
        tokio::time::advance(DEADLINE.saturating_sub(Duration::from_millis(1))).await;
        tokio::time::advance(Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert!(
            matches!(
                poll_once(request.as_mut()),
                Poll::Ready(Err(RpcClientError::RequestTimeout { .. }))
            ),
            "the request expires at the deadline fixed when it was sent"
        );
        client.stop().await.unwrap();
        release.send(()).unwrap();
        hold.await.unwrap();
    });
}

#[test]
fn idle_and_collection_deadlines_keep_their_defaults_and_overrides() {
    on_paused_task_set(|_| async {
        let client = RpcClient::new(maestro_app::RpcClientOptions::default());
        for (timeout, boundary) in [
            (None, 60_000),
            (Some(Duration::from_millis(17)), 17),
            (Some(Duration::ZERO), 0),
        ] {
            let mut idle = pin!(client.wait_for_idle(timeout));
            let mut collect = pin!(client.collect_events(timeout));
            if boundary > 0 {
                tokio::time::advance(Duration::from_millis(boundary - 1)).await;
                assert!(poll_once(idle.as_mut()).is_pending());
                assert!(poll_once(collect.as_mut()).is_pending());
                tokio::time::advance(Duration::from_millis(1)).await;
            }
            let idle = idle.await.unwrap_err();
            assert!(matches!(idle, RpcClientError::IdleTimeout { .. }));
            assert_eq!(
                idle.to_string(),
                "Timeout waiting for agent to become idle. Stderr: "
            );
            let collect = collect.await.unwrap_err();
            assert!(matches!(collect, RpcClientError::CollectionTimeout { .. }));
            assert_eq!(collect.to_string(), "Timeout collecting events. Stderr: ");
        }
    });
}

#[test]
fn waits_hold_their_deadlines_through_a_delayed_first_poll() {
    on_paused_task_set(|_| async {
        let client = RpcClient::new(maestro_app::RpcClientOptions::default());
        let (release, hold) = hold_clock();
        let timeout = Some(Duration::from_millis(17));
        let mut idle = pin!(client.wait_for_idle(timeout));
        let mut collected = pin!(client.collect_events(timeout));
        // Both deadline tasks first run only after 16 of their 17 milliseconds have
        // passed, as the caller's own work makes them.
        tokio::time::advance(Duration::from_millis(16)).await;
        assert!(poll_once(idle.as_mut()).is_pending());
        assert!(poll_once(collected.as_mut()).is_pending());
        tokio::time::advance(Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert!(
            matches!(
                poll_once(idle.as_mut()),
                Poll::Ready(Err(RpcClientError::IdleTimeout { .. }))
            ),
            "the idle wait expires at the deadline fixed when it was registered"
        );
        assert!(
            matches!(
                poll_once(collected.as_mut()),
                Poll::Ready(Err(RpcClientError::CollectionTimeout { .. }))
            ),
            "the collection expires at the deadline fixed when it was registered"
        );
        release.send(()).unwrap();
        hold.await.unwrap();
    });
}

#[test]
fn waits_ignore_events_after_their_deadline() {
    let peer = Peer::new();
    let script = r"
emit({'type': 'peer_ready'})
lines = commands()
prompt = json.loads(next(lines))
emit({'type': 'received'})
abort = json.loads(next(lines))
emit({'type': 'agent_end'})
respond(prompt)
respond(abort)
sys.stdin.read()
";
    on_paused_task_set(|_| async {
        let client = RpcClient::new(peer.options(script));
        let mut events = events(&client);
        client.start().await.unwrap();
        next_of(&mut events, "peer_ready").await;
        let (release, hold) = hold_clock();
        let timeout = Some(Duration::from_secs(1));
        let mut idle = pin!(client.wait_for_idle(timeout));
        let mut collected = pin!(client.collect_events(timeout));
        let mut prompted = pin!(client.prompt_and_wait("hello", None, timeout));
        assert!(poll_once(prompted.as_mut()).is_pending());
        next_of(&mut events, "received").await;
        tokio::time::advance(Duration::from_secs(1)).await;
        client.abort().await.unwrap();
        assert!(matches!(
            poll_once(idle.as_mut()),
            Poll::Ready(Err(RpcClientError::IdleTimeout { .. }))
        ));
        assert!(matches!(
            poll_once(collected.as_mut()),
            Poll::Ready(Err(RpcClientError::CollectionTimeout { .. }))
        ));
        assert!(
            matches!(
                poll_once(prompted.as_mut()),
                Poll::Ready(Err(RpcClientError::CollectionTimeout { .. }))
            ),
            "a prompt admitted after the collection's deadline times out"
        );
        release.send(()).unwrap();
        hold.await.unwrap();
        client.stop().await.unwrap();
    });
}

#[test]
fn answers_after_their_deadline_reach_listeners() {
    let peer = Peer::new();
    let script = r"
emit({'type': 'peer_ready'})
lines = commands()
first = json.loads(next(lines))
emit({'type': 'received'})
second = json.loads(next(lines))
respond(first)
respond(second)
sys.stdin.read()
";
    on_paused_task_set(|_| async {
        let client = RpcClient::new(peer.options(script));
        let mut events = events(&client);
        client.start().await.unwrap();
        next_of(&mut events, "peer_ready").await;
        let (release, hold) = hold_clock();
        let mut first = pin!(client.abort());
        assert!(poll_once(first.as_mut()).is_pending());
        next_of(&mut events, "received").await;
        tokio::time::advance(Duration::from_secs(30)).await;
        client.abort().await.unwrap();
        assert!(matches!(
            poll_once(first.as_mut()),
            Poll::Ready(Err(RpcClientError::RequestTimeout { .. }))
        ));
        assert_eq!(next_of(&mut events, "response").await["id"], "req_1");
        release.send(()).unwrap();
        hold.await.unwrap();
        client.stop().await.unwrap();
    });
}

/// The deadline that every request and wait of the timing cases runs against.
const DEADLINE: Duration = Duration::from_secs(30);

#[test]
fn work_expired_before_the_child_ends_keeps_its_timeout() {
    let peer = Peer::new();
    let script = r"
emit({'type': 'peer_ready'})
lines = commands()
next(lines)
emit({'type': 'received'})
next(lines)
os.write(2, b'late\n')
os._exit(0)
";
    on_paused_task_set(|_| async {
        let client = RpcClient::new(peer.options(script));
        let mut events = events(&client);
        client.start().await.unwrap();
        next_of(&mut events, "peer_ready").await;
        let (release, hold) = hold_clock();
        let mut idle = pin!(client.wait_for_idle(Some(DEADLINE)));
        let mut state = pin!(client.get_state());
        assert!(poll_once(state.as_mut()).is_pending());
        next_of(&mut events, "received").await;
        tokio::time::advance(DEADLINE).await;
        assert!(
            matches!(client.abort().await, Err(RpcClientError::Stopped)),
            "the child ends after the deadline"
        );
        while !client.get_stderr().contains("late") {
            tokio::task::yield_now().await;
        }
        let Poll::Ready(Err(error)) = poll_once(state.as_mut()) else {
            panic!("the expired request has settled");
        };
        assert_eq!(
            error.to_string(),
            "Timeout waiting for response to get_state. Stderr: "
        );
        let Poll::Ready(Err(error)) = poll_once(idle.as_mut()) else {
            panic!("the expired wait has settled");
        };
        assert_eq!(
            error.to_string(),
            "Timeout waiting for agent to become idle. Stderr: "
        );
        release.send(()).unwrap();
        hold.await.unwrap();
        client.stop().await.unwrap();
    });
}

#[test]
fn answers_before_the_deadline_count_when_consumed_after_it() {
    let peer = Peer::new();
    let script = r"
emit({'type': 'peer_ready'})
abort = json.loads(next(commands()))
emit({'type': 'agent_end'})
respond(abort)
emit({'type': 'answered'})
sys.stdin.read()
";
    on_paused_task_set(|_| async {
        let client = RpcClient::new(peer.options(script));
        let mut events = events(&client);
        client.start().await.unwrap();
        next_of(&mut events, "peer_ready").await;
        let (release, hold) = hold_clock();
        let mut idle = pin!(client.wait_for_idle(Some(DEADLINE)));
        let mut answered = pin!(client.abort());
        assert!(poll_once(answered.as_mut()).is_pending());
        next_of(&mut events, "answered").await;
        tokio::time::advance(DEADLINE).await;
        assert!(matches!(poll_once(answered.as_mut()), Poll::Ready(Ok(()))));
        assert!(matches!(poll_once(idle.as_mut()), Poll::Ready(Ok(()))));
        release.send(()).unwrap();
        hold.await.unwrap();
        client.stop().await.unwrap();
    });
}

#[test]
fn stops_before_the_deadline_count_when_consumed_after_it() {
    let peer = Peer::new();
    let script = r"
emit({'type': 'peer_ready'})
next(commands())
emit({'type': 'received'})
while True: time.sleep(1)
";
    on_paused_task_set(|_| async {
        let client = RpcClient::new(peer.options(script));
        let mut events = events(&client);
        client.start().await.unwrap();
        next_of(&mut events, "peer_ready").await;
        let (release, hold) = hold_clock();
        let mut idle = pin!(client.wait_for_idle(Some(DEADLINE)));
        let mut state = pin!(client.get_state());
        assert!(poll_once(state.as_mut()).is_pending());
        next_of(&mut events, "received").await;
        client.stop().await.unwrap();
        tokio::time::advance(DEADLINE).await;
        assert!(stopped(state.as_mut()), "the request settled at the stop");
        assert!(stopped(idle.as_mut()), "the wait settled at the stop");
        release.send(()).unwrap();
        hold.await.unwrap();
    });
}

#[test]
fn collections_include_the_future_terminal_event() {
    let peer = Peer::new();
    let script = r"
emit({'type': 'agent_end', 'past': True})
for line in commands():
    command = json.loads(line)
    for record in [{'type': 'message_update'}, {'type': 'turn_end'}, {'type': 'turn_end'}, {'type': 'agent_end'}, {'type': 'late'}]:
        emit(record)
    respond(command)
";
    on_task_set(|_| async {
        let client = RpcClient::new(peer.options(script));
        let mut events = events(&client);
        client.start().await.unwrap();
        next_of(&mut events, "agent_end").await;
        let idle = client.wait_for_idle(None);
        let collected = client.collect_events(None);
        client.abort().await.unwrap();
        idle.await.unwrap();
        assert_eq!(
            collected.await.unwrap(),
            [
                json!({"type": "message_update"}),
                json!({"type": "turn_end"}),
                json!({"type": "turn_end"}),
                json!({"type": "agent_end"})
            ]
        );
        client.stop().await.unwrap();
    });
}

#[test]
fn prompt_collection_is_subscribed_before_the_prompt() {
    let peer = Peer::new();
    let script = r"
for line in commands():
    command = json.loads(line)
    if command['message'] == 'quiet':
        respond(command); continue
    emit({'type': 'agent_start'})
    emit({'type': 'agent_end', 'messages': []})
    respond(command)
";
    on_task_set(|_| async {
        let client = RpcClient::new(peer.options(script));
        client.start().await.unwrap();
        let events = client.prompt_and_wait("hello", None, None).await.unwrap();
        assert_eq!(
            events,
            [
                json!({"type": "agent_start"}),
                json!({"type": "agent_end", "messages": []})
            ]
        );
        let error = client
            .prompt_and_wait("quiet", None, Some(Duration::from_millis(17)))
            .await
            .unwrap_err();
        assert!(matches!(error, RpcClientError::CollectionTimeout { .. }));
        client.stop().await.unwrap();
    });
}

#[test]
fn prompt_failure_releases_its_prepared_collector() {
    let peer = Peer::new();
    let refusing = "emit({'type': 'peer_ready'})\nfor line in commands():\n    respond(json.loads(line), error='Rejected prompt')\n    emit({'type': 'agent_end'})";
    let closed = "os.close(0)\nemit({'type': 'peer_ready'})\nwhile True: time.sleep(1)";
    on_paused_task_set(|_| async {
        let started = tokio::time::Instant::now();
        let unstarted = RpcClient::new(peer.options(SERVE));
        let error = unstarted
            .prompt_and_wait("hello", None, None)
            .await
            .unwrap_err();
        assert!(matches!(error, RpcClientError::NotStarted));
        assert_eq!(
            tokio::time::Instant::now(),
            started,
            "the collection ended with the prompt"
        );
    });
    on_task_set(|_| async {
        for (script, expected) in [
            (refusing, "Rejected prompt"),
            (closed, "Broken pipe (os error 32)"),
        ] {
            let client = RpcClient::new(peer.options(script));
            let mut events = events(&client);
            client.start().await.unwrap();
            next_of(&mut events, "peer_ready").await;
            let started = std::time::Instant::now();
            let error = client
                .prompt_and_wait("hello", None, None)
                .await
                .unwrap_err();
            assert_eq!(error.to_string(), expected);
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "the collection ended with the prompt"
            );
            client.stop().await.unwrap();
        }
    });
}

#[test]
fn stop_and_exit_settle_only_owned_calls_and_waits() {
    let peer = Peer::new();
    let script = r"
emit({'type': 'peer_ready'})
for line in commands():
    command = json.loads(line)
    emit({'type': 'received', 'command': command['type']})
    if command['type'] == 'bash': os._exit(3)
    if command['type'] == 'abort': respond(command)
";
    on_task_set(|_| async {
        let client = RpcClient::new(peer.options(script));
        let mut events = events(&client);
        for ending in ["stop", "exit"] {
            client.start().await.unwrap();
            next_of(&mut events, "peer_ready").await;
            let idle = client.wait_for_idle(None);
            let collected = client.collect_events(None);
            let state = client.get_state();
            let ended = async {
                next_of(&mut events, "received").await;
                end_child(&client, ending).await;
            };
            let (idle, collected, state, ()) = tokio::join!(idle, collected, state, ended);
            assert!(matches!(idle, Err(RpcClientError::Stopped)), "{ending}");
            assert!(
                matches!(collected, Err(RpcClientError::Stopped)),
                "{ending}"
            );
            let Err(error) = state else {
                panic!("the pending request settles at {ending}");
            };
            assert!(matches!(error, RpcClientError::Stopped), "{ending}");
            assert_eq!(error.to_string(), "Client stopped");
            client.stop().await.unwrap();
            while events.try_recv().is_ok() {}
            client.start().await.unwrap();
            client.abort().await.unwrap();
            assert_eq!(
                next_of(&mut events, "received").await["command"],
                "abort",
                "the subscription outlives {ending}"
            );
            client.stop().await.unwrap();
        }
    });
}

/// Whether `future` has settled with [`RpcClientError::Stopped`] by now.
fn stopped<T>(
    future: std::pin::Pin<&mut impl std::future::Future<Output = Result<T, RpcClientError>>>,
) -> bool {
    matches!(poll_once(future), Poll::Ready(Err(RpcClientError::Stopped)))
}

#[test]
fn child_exit_settles_work_while_a_descendant_keeps_the_output_open() {
    let peer = Peer::new();
    let script = r"
import subprocess
emit({'type': 'peer_ready', 'pid': os.getpid()})
next(commands())
subprocess.Popen(['sh', '-c', 'read _'])
os._exit(0)
";
    on_paused_task_set(|_| async {
        let client = RpcClient::new(peer.options(script));
        let mut events = events(&client);
        client.start().await.unwrap();
        let ready = next_of(&mut events, "peer_ready").await;
        let pid = u32::try_from(ready["pid"].as_u64().unwrap()).unwrap();
        let (release, hold) = hold_clock();
        let mut idle = pin!(client.wait_for_idle(None));
        let mut state = pin!(client.get_state());
        assert!(poll_once(state.as_mut()).is_pending());
        while process_exists(pid) {
            tokio::task::yield_now().await;
        }
        assert!(stopped(state.as_mut()), "the request settles at the exit");
        assert!(stopped(idle.as_mut()), "the wait settles at the exit");
        release.send(()).unwrap();
        hold.await.unwrap();
        client.stop().await.unwrap();
    });
}

#[test]
fn work_registered_during_termination_settles_when_it_ends() {
    let peer = Peer::new();
    let script = r"
signal.signal(signal.SIGTERM, lambda *_: os.write(2, b'terminating\n'))
emit({'type': 'peer_ready'})
while True: time.sleep(1)
";
    on_paused_task_set(|_| async {
        let client = RpcClient::new(peer.options(script));
        let mut events = events(&client);
        client.start().await.unwrap();
        next_of(&mut events, "peer_ready").await;
        let (release, hold) = hold_clock();
        let mut stop = pin!(client.stop());
        assert!(poll_once(stop.as_mut()).is_pending());
        while !client.get_stderr().contains("terminating") {
            tokio::task::yield_now().await;
        }
        let mut idle = pin!(client.wait_for_idle(None));
        let mut state = pin!(client.get_state());
        assert!(poll_once(state.as_mut()).is_pending());
        tokio::time::advance(Duration::from_millis(1000)).await;
        stop.await.unwrap();
        assert!(
            stopped(state.as_mut()),
            "a request sent while stopping settles"
        );
        assert!(
            stopped(idle.as_mut()),
            "a wait registered while stopping settles"
        );
        release.send(()).unwrap();
        hold.await.unwrap();
    });
}

#[test]
fn cancelled_and_failed_operations_release_only_their_own_resources() {
    let peer = Peer::new();
    let script = r"
emit({'type': 'peer_ready'})
for line in commands():
    command = json.loads(line)
    if command['type'] == 'abort': emit({'type': 'agent_end'})
    respond(command)
";
    let token = Rc::new(());
    on_task_set(|_| async {
        let client = RpcClient::new(peer.options(script));
        let kept = Rc::clone(&token);
        let mut events = events(&client);
        let _keep = client.on_event(Rc::new(move |_: &Value| {
            let _ = &kept;
            Ok(())
        }));
        client.start().await.unwrap();
        next_of(&mut events, "peer_ready").await;
        let mut dropped = Box::pin(client.get_state());
        assert!(poll_once(dropped.as_mut()).is_pending());
        drop(dropped);
        let late = next_of(&mut events, "response").await;
        assert_eq!(
            late["id"], "req_1",
            "a dropped request's answer is an ordinary event"
        );
        drop(client.collect_events(None));
        let collected = client.collect_events(None);
        client.abort().await.unwrap();
        assert_eq!(collected.await.unwrap(), [json!({"type": "agent_end"})]);
        client.stop().await.unwrap();
        drop(client);
        assert_eq!(
            Rc::strong_count(&token),
            1,
            "a stopped client keeps no callback alive"
        );
    });
}

#[test]
fn dispatch_and_framing_work_stays_linear() {
    let peer = Peer::new();
    let script = "for line in commands():\n    count = json.loads(line)['command']\n    for index in range(int(count)): emit({'type': 'tick', 'index': index})\n    emit({'type': 'done'})\n    respond(json.loads(line))";
    on_task_set(|_| async {
        let client = RpcClient::new(peer.options(script));
        let visits = Rc::new(RefCell::new(0_usize));
        let removers: Vec<_> = (0..4)
            .map(|_| {
                let visits = Rc::clone(&visits);
                client.on_event(Rc::new(move |_: &Value| {
                    *visits.borrow_mut() += 1;
                    Ok(())
                }))
            })
            .collect();
        let mut events = events(&client);
        client.start().await.unwrap();
        for frames in [64, 128, 256] {
            *visits.borrow_mut() = 0;
            let _ = client.bash(&frames.to_string()).await;
            next_of(&mut events, "done").await;
            assert_eq!(
                *visits.borrow(),
                4 * (frames + 1),
                "each frame visits each registration once"
            );
        }
        for remove in removers {
            remove();
        }
        client.stop().await.unwrap();
    });
}
