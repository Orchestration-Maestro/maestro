//! Actor byte delivery and retained ownership.
use super::runner::{WriteStatus, Writes};
use super::*;
use super::{
    controls::Admission,
    unix::{Owner, ReadCallback, ReaderExitCallback},
};
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{self, error::TrySendError};

/// Starts a socket-backed actor with caller-owned callbacks.
fn callback_actor(
    quiesced: bool,
    on_read: ReadCallback,
    on_exit: Option<ReaderExitCallback>,
) -> (PtyIoActorHandle, tokio::net::UnixStream) {
    let (socket, peer) = std::os::unix::net::UnixStream::pair().unwrap();
    peer.set_nonblocking(true).unwrap();
    let handle = PtyIoActor::spawn(PtyIoActorConfig {
        pane_id: 1,
        master_fd: socket.into(),
        initially_quiesced: quiesced,
        on_read,
        on_reader_exit: on_exit,
    })
    .unwrap();
    (handle, tokio::net::UnixStream::from_std(peer).unwrap())
}

/// Starts the common byte-observation fixture.
fn fixture(
    quiesced: bool,
) -> (
    PtyIoActorHandle,
    tokio::net::UnixStream,
    tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
) {
    let (tx, rx) = mpsc::unbounded_channel();
    let (handle, peer) = callback_actor(
        quiesced,
        Box::new(move |bytes| {
            tx.send(bytes.to_vec()).unwrap();
            PtyReadResult::default()
        }),
        None,
    );
    (handle, peer, rx)
}

/// Stops and joins the one retained worker.
async fn finish(handle: PtyIoActorHandle) {
    handle.shutdown();
    let task = handle.owner.task.lock().unwrap().take().unwrap();
    task.await.unwrap();
}

#[test]
fn user_input_reaches_the_owned_descriptor() {
    crate::children::runtime().unwrap().block_on(async {
        if maestro_test_process::run_alone("user_input_reaches_the_owned_descriptor").unwrap() {
            return;
        }
        let (handle, mut peer, _reads) = fixture(false);
        handle.write_user_input(b"hello".to_vec()).await.unwrap();
        let mut bytes = [0; 5];
        peer.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"hello");
        finish(handle).await;
    });
}

#[test]
fn continuous_output_keeps_input_and_interrupt_writable() {
    crate::children::runtime().unwrap().block_on(async {
        if maestro_test_process::run_alone("continuous_output_keeps_input_and_interrupt_writable")
            .unwrap()
        {
            return;
        }
        let (socket, peer) = std::os::unix::net::UnixStream::pair().unwrap();
        peer.set_nonblocking(true).unwrap();
        rustix::io::write(&peer, &[42; 8192]).unwrap();
        let (done, completed) = tokio::sync::oneshot::channel();
        let mut done = Some(done);
        let mut observed = Vec::new();
        let mut rounds = 0;
        let handle = PtyIoActor::spawn(PtyIoActorConfig {
            pane_id: 1,
            master_fd: socket.into(),
            initially_quiesced: false,
            on_read: Box::new(move |bytes| {
                if rounds < 31 {
                    assert_eq!(rustix::io::write(&peer, bytes).unwrap(), bytes.len());
                }
                let mut input = [0; 32];
                match rustix::io::read(&peer, &mut input) {
                    Ok(length) => observed.extend_from_slice(&input[..length]),
                    Err(rustix::io::Errno::AGAIN) => {}
                    Err(error) => panic!("peer read: {error}"),
                }
                rounds += 1;
                if rounds == 32 {
                    assert_eq!(observed, b"input\x03reply");
                    done.take().unwrap().send(()).unwrap();
                }
                PtyReadResult {
                    terminal_responses: if rounds == 1 {
                        vec![b"reply".to_vec()]
                    } else {
                        vec![]
                    },
                }
            }),
            on_reader_exit: None,
        })
        .unwrap();
        handle.try_write_user_input(b"input\x03".to_vec()).unwrap();
        completed.await.unwrap();
        finish(handle).await;
    });
}

#[test]
fn continuous_output_keeps_async_input_producers_running() {
    crate::children::runtime().unwrap().block_on(async {
        if maestro_test_process::run_alone("continuous_output_keeps_async_input_producers_running")
            .unwrap()
        {
            return;
        }
        let (socket, peer) = std::os::unix::net::UnixStream::pair().unwrap();
        peer.set_nonblocking(true).unwrap();
        let reader = peer.try_clone().unwrap();
        rustix::io::write(&peer, &[42; 8192]).unwrap();
        let (started, reading) = tokio::sync::oneshot::channel();
        let mut started = Some(started);
        let handle = PtyIoActor::spawn(PtyIoActorConfig {
            pane_id: 2,
            master_fd: socket.into(),
            initially_quiesced: false,
            on_read: Box::new(move |bytes| {
                assert_eq!(rustix::io::write(&peer, bytes).unwrap(), bytes.len());
                if let Some(started) = started.take() {
                    started.send(()).unwrap();
                }
                PtyReadResult::default()
            }),
            on_reader_exit: None,
        })
        .unwrap();
        let producer = tokio::spawn(async move {
            reading.await.unwrap();
            handle
                .write_user_input(b"interrupt\x03".to_vec())
                .await
                .unwrap();
            let mut reader = tokio::net::UnixStream::from_std(reader).unwrap();
            let mut bytes = [0; 10];
            reader.read_exact(&mut bytes).await.unwrap();
            assert_eq!(&bytes, b"interrupt\x03");
            handle.release_after_commit().await.unwrap();
        });
        producer.await.unwrap();
    });
}

#[test]
fn appearance_reports_precede_queries_of_the_new_appearance() {
    crate::children::runtime().unwrap().block_on(async {
        if maestro_test_process::run_alone(
            "appearance_reports_precede_queries_of_the_new_appearance",
        )
        .unwrap()
        {
            return;
        }
        let (socket, peer) = std::os::unix::net::UnixStream::pair().unwrap();
        crate::terminal::fd::set_nonblocking(&socket).unwrap();
        let light = Arc::new(AtomicBool::new(false));
        let query_light = Arc::clone(&light);
        let (handle, data, commands) = parked_handle();
        let runner = runner::Runner::new(
            PtyIoActorConfig {
                pane_id: 7,
                master_fd: socket.into(),
                initially_quiesced: false,
                on_read: Box::new(move |_| PtyReadResult {
                    terminal_responses: vec![if query_light.load(Ordering::Acquire) {
                        b"query-light".to_vec()
                    } else {
                        b"query-dark".to_vec()
                    }],
                }),
                on_reader_exit: None,
            },
            data,
            commands,
            Arc::clone(&handle.owner.admission),
            Arc::clone(&handle.owner.controls),
        )
        .unwrap();
        let (changed, published) = std::sync::mpsc::channel();
        let (release, resume) = std::sync::mpsc::channel();
        let reentrant = handle.clone();
        let appearance = std::thread::spawn(move || {
            handle.write_terminal_response(|| {
                light.store(true, Ordering::Release);
                reentrant.try_write_user_input(b"input".to_vec()).unwrap();
                changed.send(()).unwrap();
                resume.recv().unwrap();
                Some(b"live-light".to_vec())
            });
        });
        published.recv().unwrap();
        assert_appearance_read(runner, peer, release).await;
        appearance.join().unwrap();
    });
}

/// Drives a query concurrently with the held appearance publication, then checks both replies.
async fn assert_appearance_read(
    mut runner: runner::Runner,
    mut peer: std::os::unix::net::UnixStream,
    release: std::sync::mpsc::Sender<()>,
) {
    std::io::Write::write_all(&mut peer, b"query").unwrap();
    let (started, reading) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut bytes = [0; 5];
        let length = rustix::io::read(runner.fd.get_ref(), &mut bytes).unwrap();
        assert_eq!(&bytes[..length], b"query");
        started.send(()).unwrap();
        assert!(runner.read_result(&bytes, Ok(length)));
        runner
    });
    reading.recv().unwrap();
    release.send(()).unwrap();
    let mut runner = reader.join().unwrap();
    let mut bytes = Vec::new();
    assert!(matches!(
        runner.writes.flush(|chunk| {
            bytes.extend_from_slice(chunk);
            Ok(chunk.len())
        }),
        WriteStatus::Complete
    ));
    assert_eq!(bytes, b"live-lightquery-light");
    runner.begin_handoff().await.unwrap();
    let mut input = [0; 5];
    std::io::Read::read_exact(&mut peer, &mut input).unwrap();
    assert_eq!(&input, b"input");
}

#[test]
fn handoff_pauses_reads_and_input_until_rollback() {
    crate::children::runtime().unwrap().block_on(async {
    if maestro_test_process::run_alone("handoff_pauses_reads_and_input_until_rollback").unwrap() {
        return;
    }
    let (handle, mut peer, mut reads) = fixture(false);
    assert_eq!(
        handle
            .duplicate_for_handoff()
            .await
            .unwrap_err()
            .to_string(),
        "PTY actor must be quiesced before handoff duplication"
    );
    handle
        .try_write_user_input(b"queued-before-ack".to_vec())
        .unwrap();
    handle
        .begin_handoff(std::time::Duration::from_secs(1))
        .await
        .unwrap();
    let mut bytes = [0; 17];
    peer.read_exact(&mut bytes).await.unwrap();
    assert_eq!(&bytes, b"queued-before-ack");
    assert!(
        matches!(handle.try_write_user_input(b"blocked".to_vec()),Err(tokio::sync::mpsc::error::TrySendError::Closed(bytes)) if bytes==b"blocked")
    );
    let duplicate = handle.duplicate_for_handoff().await.unwrap();
    assert!(
        rustix::io::fcntl_getfd(&duplicate)
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    assert_eq!(handle.foreground_process_group_id().await, None);
    assert_quiesced_read_decision().await;
    peer.write_all(b"held").await.unwrap();
    handle.rollback_handoff().await.unwrap();
    assert_eq!(reads.recv().await.unwrap(), b"held");
    handle.write_user_input(b"after".to_vec()).await.unwrap();
    let mut bytes = [0; 5];
    peer.read_exact(&mut bytes).await.unwrap();
    assert_eq!(&bytes, b"after");
    handle.release_after_commit().await.unwrap();
    assert_eq!(
        handle.rollback_handoff().await.unwrap_err().to_string(),
        "pty actor closed"
    );
    assert!(matches!(
        handle.try_write_user_input(vec![1]),
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_))
    ));
    rustix::io::write(&duplicate, b"duplicate").unwrap();
    let mut bytes = [0; 9];
    peer.read_exact(&mut bytes).await.unwrap();
    assert_eq!(&bytes, b"duplicate");

    });
}

#[test]
fn actor_exit_and_errors_preserve_their_single_delivery() {
    let events = Events::default();
    maestro_test_tracing::with_default(events.clone(), || {
        crate::children::runtime().unwrap().block_on(async {
            if maestro_test_process::run_alone(
                "actor_exit_and_errors_preserve_their_single_delivery",
            )
            .unwrap()
            {
                return;
            }
            assert_read_outcomes(&events);
            assert_write_outcomes(&events);
            assert_closed_actor_replies(&events).await;
            assert_reader_exit_releases_callbacks(&events).await;
            let (socket, peer) = std::os::unix::net::UnixStream::pair().unwrap();
            fill(&socket);
            peer.shutdown(std::net::Shutdown::Write).unwrap();
            let mut runner = controlled_runner(socket.into(), Arc::default());
            runner.enqueue(b"blocked".to_vec());
            drop(runner.fd.readable().await.unwrap());
            assert_eq!(
                runner.begin_handoff().await.unwrap_err().to_string(),
                "PTY closed while draining writes before handoff"
            );
        });
    });
}

/// A handle with controllable queue and replies, without a worker or native resource.
fn parked_handle() -> (
    PtyIoActorHandle,
    mpsc::Receiver<Vec<u8>>,
    mpsc::UnboundedReceiver<controls::Control>,
) {
    let (data, rx) = mpsc::channel(1);
    let (control, commands) = mpsc::unbounded_channel();
    let handle = PtyIoActorHandle {
        owner: Arc::new(Owner {
            admission: Arc::new(Admission::new(true)),
            data,
            control,
            controls: Arc::default(),
            task: Mutex::new(None),
        }),
    };
    (handle, rx, commands)
}

/// Polls once without installing a timer or spawning work.
fn poll_once<F: std::future::Future>(future: std::pin::Pin<&mut F>) -> std::task::Poll<F::Output> {
    future.poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
}

#[test]
fn empty_input_does_not_reach_the_terminal() {
    crate::children::runtime().unwrap().block_on(async {
        if maestro_test_process::run_alone("empty_input_does_not_reach_the_terminal").unwrap() {
            return;
        }
        let mut writes = super::runner::Writes::default();
        writes.enqueue(vec![]);
        assert!(writes.is_empty());
        let (handle, mut peer, _) = fixture(false);
        handle.try_write_user_input(vec![]).unwrap();
        handle.try_write_user_input(b"sentinel".to_vec()).unwrap();
        let mut bytes = [0; 8];
        peer.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"sentinel");
        finish(handle).await;
    });
}

#[test]
fn idle_actor_processes_new_input_without_polling() {
    crate::children::runtime().unwrap().block_on(async {
        if maestro_test_process::run_alone("idle_actor_processes_new_input_without_polling")
            .unwrap()
        {
            return;
        }
        let (handle, mut peer, _) = fixture(false);
        handle.try_write_user_input(b"first".to_vec()).unwrap();
        let mut bytes = [0; 5];
        peer.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"first");
        handle.try_write_user_input(b"later".to_vec()).unwrap();
        peer.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"later");
        finish(handle).await;
    });
}

#[test]
fn terminal_reads_reach_the_supplied_callback() {
    crate::children::runtime().unwrap().block_on(async {
        if maestro_test_process::run_alone("terminal_reads_reach_the_supplied_callback").unwrap() {
            return;
        }
        let (tx, mut reads) = mpsc::unbounded_channel();
        let (handle, mut peer) = callback_actor(
            false,
            Box::new(move |bytes| {
                tx.send(bytes.to_vec()).unwrap();
                PtyReadResult {
                    terminal_responses: vec![
                        b"reply-a".to_vec(),
                        vec![],
                        b"reply-b".to_vec(),
                        b"reply-a".to_vec(),
                    ],
                }
            }),
            None,
        );
        peer.write_all(b"from-peer").await.unwrap();
        assert_eq!(reads.recv().await.unwrap(), b"from-peer");
        let mut replies = [0; 21];
        peer.read_exact(&mut replies).await.unwrap();
        assert_eq!(&replies, b"reply-areply-breply-a");
        finish(handle).await;
    });
}

#[test]
fn idle_actor_accepts_handoff_without_polling() {
    crate::children::runtime().unwrap().block_on(async {
        if maestro_test_process::run_alone("idle_actor_accepts_handoff_without_polling").unwrap() {
            return;
        }
        let (handle, _peer, _) = fixture(false);
        handle
            .begin_handoff(std::time::Duration::from_secs(1))
            .await
            .unwrap();
        assert!(handle.duplicate_for_handoff().await.is_ok());
        handle.release_after_commit().await.unwrap();
    });
}

#[test]
fn quiesced_hangup_is_not_read_or_busy_polled() {
    crate::children::runtime().unwrap().block_on(async {
        if maestro_test_process::run_alone("quiesced_hangup_is_not_read_or_busy_polled").unwrap() {
            return;
        }
        let (handle, peer, mut reads) = fixture(true);
        assert!(matches!(handle.try_write_user_input(vec![42]),Err(TrySendError::Closed(bytes)) if bytes==[42]));
        drop(peer);
        assert!(handle.duplicate_for_handoff().await.is_ok());
        handle.release_after_commit().await.unwrap();
        assert!(reads.recv().await.is_none());
    });
}

#[test]
fn handoff_duplicates_only_a_quiesced_master() {
    crate::children::runtime().unwrap().block_on(async {
        if maestro_test_process::run_alone("handoff_duplicates_only_a_quiesced_master").unwrap() {
            return;
        }
        let (handle, mut peer, _) = fixture(false);
        assert!(handle.duplicate_for_handoff().await.is_err());
        handle
            .begin_handoff(std::time::Duration::from_secs(1))
            .await
            .unwrap();
        let fd = handle.duplicate_for_handoff().await.unwrap();
        assert!(
            rustix::io::fcntl_getfd(&fd)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        handle.release_after_commit().await.unwrap();
        rustix::io::write(&fd, b"retained").unwrap();
        let mut bytes = [0; 8];
        peer.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"retained");
    });
}

#[test]
fn full_input_queue_keeps_the_latest_resize_and_nudge() {
    crate::children::runtime().unwrap().block_on(async {
    if maestro_test_process::run_alone("full_input_queue_keeps_the_latest_resize_and_nudge").unwrap(){return;}
    let (handle, _data, _control) = parked_handle();
    handle.try_write_user_input(vec![1]).unwrap();
    let size = |rows, cols| crate::terminal::PtySize {
        rows,
        cols,
        cell_width_px: 8,
        cell_height_px: 16,
    };
    handle.resize(size(20, 80), vec![b"old".to_vec()]);
    handle.resize(size(40, 120), vec![b"new".to_vec()]);
    handle.nudge_child_redraw_after_handoff(size(21, 81));
    handle.nudge_child_redraw_after_handoff(size(41, 121));
    handle.write_terminal_response(|| Some(b"first".to_vec()));
    handle.write_terminal_response(|| Some(b"second".to_vec()));
    assert!(
        matches!(handle.try_write_user_input(vec![2]),Err(TrySendError::Full(bytes)) if bytes==[2])
    );
    {
    let pending = handle.owner.controls.pending.lock().unwrap();
    assert_eq!(pending.resize, Some((size(40, 120), vec![b"new".to_vec()])));
    assert_eq!(pending.nudge, Some(size(41, 121)));
    assert_eq!(pending.responses, [b"first".to_vec(), b"second".to_vec()]);
    }

    let (live,mut peer,_)=fixture(false);
    for index in 0u16..1024 {live.try_write_user_input(index.to_le_bytes().to_vec()).unwrap();}
    assert!(matches!(live.try_write_user_input(b"overflow".to_vec()),Err(TrySendError::Full(bytes)) if bytes==b"overflow"));
    live.release_after_commit().await.unwrap();
    let mut bytes=Vec::new();peer.read_to_end(&mut bytes).await.unwrap();assert!(bytes.is_empty());


    });
}

#[test]
fn async_input_resumes_only_after_capacity_returns() {
    crate::children::runtime().unwrap().block_on(async {
        let (handle, mut data, _control) = parked_handle();
        handle.try_write_user_input(b"fill".to_vec()).unwrap();
        let write = handle.write_user_input(b"waiting".to_vec());
        tokio::pin!(write);
        assert!(poll_once(write.as_mut()).is_pending());
        assert_eq!(data.recv().await.unwrap(), b"fill");
        write.await.unwrap();
        assert_eq!(data.recv().await.unwrap(), b"waiting");
    });
}

#[test]
fn handoff_rejects_input_already_waiting_for_capacity() {
    crate::children::runtime().unwrap().block_on(async {
        let (handle, mut data, mut commands) = parked_handle();
        handle.try_write_user_input(b"occupied".to_vec()).unwrap();
        let write = handle.write_user_input(b"must-return".to_vec());
        tokio::pin!(write);
        assert!(poll_once(write.as_mut()).is_pending());
        let handoff = handle.begin_handoff(std::time::Duration::from_secs(1));
        tokio::pin!(handoff);
        assert!(poll_once(handoff.as_mut()).is_pending());
        let controls::Control::Begin(reply) = commands.try_recv().unwrap() else {
            panic!("begin control");
        };
        assert!(reply.send(Ok(())).is_ok());
        handoff.await.unwrap();
        assert_eq!(data.recv().await.unwrap(), b"occupied");
        assert_eq!(write.await.unwrap_err().0, b"must-return");
        assert!(matches!(
            data.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    });
}

#[test]
fn full_input_queue_does_not_block_handoff_control() {
    crate::children::runtime().unwrap().block_on(async {
        let (handle, _data, mut commands) = parked_handle();
        handle.try_write_user_input(vec![7]).unwrap();
        let handoff = handle.begin_handoff(std::time::Duration::from_secs(1));
        tokio::pin!(handoff);
        assert!(poll_once(handoff.as_mut()).is_pending());
        let controls::Control::Begin(reply) = commands.try_recv().unwrap() else {
            panic!("independent control");
        };
        assert!(reply.send(Ok(())).is_ok());
        handoff.await.unwrap();
    });
}

#[test]
fn handoff_drains_input_queued_before_admission_closed() {
    crate::children::runtime().unwrap().block_on(async {
        if maestro_test_process::run_alone("handoff_drains_input_queued_before_admission_closed")
            .unwrap()
        {
            return;
        }
        let (handle, mut peer, _) = fixture(false);
        handle
            .try_write_user_input(b"pre-quiesce".to_vec())
            .unwrap();
        handle
            .begin_handoff(std::time::Duration::from_secs(1))
            .await
            .unwrap();
        let mut bytes = [0; 11];
        peer.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"pre-quiesce");
        handle.release_after_commit().await.unwrap();
    });
}

#[test]
fn committed_release_stops_terminal_io() {
    crate::children::runtime().unwrap().block_on(async {
    if maestro_test_process::run_alone("committed_release_stops_terminal_io").unwrap() {
        return;
    }
    let (handle, mut peer, mut reads) = fixture(false);
    handle.release_after_commit().await.unwrap();
    assert!(
        matches!(handle.try_write_user_input(b"late".to_vec()),Err(TrySendError::Closed(bytes)) if bytes==b"late")
    );
    let mut called = false;
    handle.write_terminal_response(|| {
        called = true;
        Some(b"late-reply".to_vec())
    });
    assert!(called);
    let size = crate::terminal::PtySize {
        rows: 30,
        cols: 90,
        cell_width_px: 8,
        cell_height_px: 16,
    };
    handle.resize(size, vec![b"late-resize".to_vec()]);
    handle.nudge_child_redraw_after_handoff(size);
    let mut bytes = Vec::new();
    peer.read_to_end(&mut bytes).await.unwrap();
    assert!(bytes.is_empty());
    assert!(reads.recv().await.is_none());

    });
}

/// Opens a real terminal whose slave can observe geometry and terminal bytes.
fn terminal_pair() -> (std::os::fd::OwnedFd, std::fs::File) {
    use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
    let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY | OpenptFlags::CLOEXEC).unwrap();
    grantpt(&master).unwrap();
    unlockpt(&master).unwrap();
    let name = ptsname(&master, Vec::new()).unwrap();
    let slave = rustix::fs::open(
        name.as_c_str(),
        rustix::fs::OFlags::RDWR | rustix::fs::OFlags::NOCTTY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .unwrap();
    (master, std::fs::File::from(slave))
}

/// Builds the actual private readiness owner without launching its loop.
fn controlled_runner(
    fd: std::os::fd::OwnedFd,
    controls: Arc<controls::Controls>,
) -> runner::Runner {
    controlled_read_runner(fd, controls, Box::new(|_| PtyReadResult::default()))
}

/// Builds the readiness owner with an observable byte consumer.
fn controlled_read_runner(
    fd: std::os::fd::OwnedFd,
    controls: Arc<controls::Controls>,
    on_read: ReadCallback,
) -> runner::Runner {
    crate::terminal::fd::set_nonblocking(&fd).unwrap();
    let (_, data) = mpsc::channel(1);
    let (_, commands) = mpsc::unbounded_channel();
    runner::Runner::new(
        PtyIoActorConfig {
            pane_id: 7,
            master_fd: fd,
            initially_quiesced: false,
            on_read,
            on_reader_exit: None,
        },
        data,
        commands,
        Arc::new(Admission::new(true)),
        controls,
    )
    .unwrap()
}

/// Closes the quiesced read decision after proving the same socket can deliver bytes.
async fn assert_quiesced_read_decision() {
    let (socket, peer) = std::os::unix::net::UnixStream::pair().unwrap();
    let (tx, mut reads) = mpsc::unbounded_channel();
    let mut runner = controlled_read_runner(
        socket.into(),
        Arc::default(),
        Box::new(move |bytes| {
            tx.send(bytes.to_vec()).unwrap();
            PtyReadResult::default()
        }),
    );
    rustix::io::write(&peer, b"running").unwrap();
    drop(runner.fd.readable().await.unwrap());
    assert!(runner.interest().is_readable());
    assert!(runner.service_ready(tokio::io::Ready::READABLE));
    assert_eq!(reads.try_recv().unwrap(), b"running");
    runner.begin_handoff().await.unwrap();
    assert!(!runner.interest().is_readable());
    rustix::io::write(&peer, b"held").unwrap();
    drop(runner.fd.readable().await.unwrap());
    assert!(runner.service_ready(tokio::io::Ready::READABLE));
    assert!(matches!(
        reads.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));
}

#[test]
fn resize_is_applied_before_its_terminal_responses() {
    crate::children::runtime().unwrap().block_on(async {
        if maestro_test_process::run_alone("resize_is_applied_before_its_terminal_responses")
            .unwrap()
        {
            return;
        }
        let (master, slave) = terminal_pair();
        let (handle, _data, _commands) = parked_handle();
        let mut runner = controlled_runner(master, Arc::clone(&handle.owner.controls));
        let size = crate::terminal::PtySize {
            rows: 40,
            cols: 100,
            cell_width_px: 9,
            cell_height_px: 18,
        };
        handle.resize(size, vec![b"resized".to_vec()]);
        runner.apply_controls().await;
        let mut emitted = Vec::new();
        assert!(matches!(
            runner.writes.flush(|bytes| {
                let size = rustix::termios::tcgetwinsize(&slave).unwrap();
                assert_eq!(
                    (size.ws_row, size.ws_col, size.ws_xpixel, size.ws_ypixel),
                    (40, 100, 900, 720)
                );
                emitted.extend_from_slice(bytes);
                Ok(bytes.len())
            }),
            runner::WriteStatus::Complete
        ));
        assert_eq!(emitted, b"resized");
    });
}

#[test]
fn actor_redraw_nudge_restores_the_requested_size() {
    crate::tests::runtime::paused_runtime().block_on(async {
        if maestro_test_process::run_alone("actor_redraw_nudge_restores_the_requested_size")
            .unwrap()
        {
            return;
        }
        let (master, slave) = terminal_pair();
        let (handle, _data, _commands) = parked_handle();
        let mut runner = controlled_runner(master, Arc::clone(&handle.owner.controls));
        assert_nudge_sizes(&mut runner, &slave, &handle).await;
        let first = crate::terminal::PtySize {
            rows: 20,
            cols: 60,
            cell_width_px: 0,
            cell_height_px: 0,
        };
        let second = crate::terminal::PtySize {
            rows: 30,
            cols: 90,
            ..first
        };
        handle.nudge_child_redraw_after_handoff(first);
        {
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(30);
            let apply = runner.apply_controls();
            tokio::pin!(apply);
            assert!(poll_once(apply.as_mut()).is_pending());
            handle.nudge_child_redraw_after_handoff(second);
            tokio::time::advance(std::time::Duration::from_millis(30)).await;
            finish_at(apply, deadline).await;
        }
        {
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(30);
            let apply = runner.apply_controls();
            tokio::pin!(apply);
            assert!(poll_once(apply.as_mut()).is_pending());
            assert_eq!(rustix::termios::tcgetwinsize(&slave).unwrap().ws_row, 29);
            tokio::time::advance(std::time::Duration::from_millis(30)).await;
            finish_at(apply, deadline).await;
        }
        assert_eq!(rustix::termios::tcgetwinsize(&slave).unwrap().ws_row, 30);
        let (master2, slave2) = terminal_pair();
        let (handle2, _, _) = parked_handle();
        let mut runner2 = controlled_runner(master2, Arc::clone(&handle2.owner.controls));
        handle2.nudge_child_redraw_after_handoff(first);
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(30);
        let apply = runner2.apply_controls();
        tokio::pin!(apply);
        assert!(poll_once(apply.as_mut()).is_pending());
        assert_eq!(rustix::termios::tcgetwinsize(&slave2).unwrap().ws_row, 19);
        assert_eq!(rustix::termios::tcgetwinsize(&slave).unwrap().ws_row, 30);
        tokio::time::advance(std::time::Duration::from_millis(30)).await;
        finish_at(apply, deadline).await;
        assert_eq!(rustix::termios::tcgetwinsize(&slave2).unwrap().ws_row, 20);
    });
}

/// Fills a socket's native output buffer to a proven `WouldBlock` boundary.
fn fill(socket: &std::os::unix::net::UnixStream) -> usize {
    socket.set_nonblocking(true).unwrap();
    let mut count = 0;
    loop {
        match rustix::io::write(socket, &[0xaa; 8192]) {
            Ok(length) => count += length,
            Err(rustix::io::Errno::AGAIN) => break,
            Err(error) => panic!("fill: {error}"),
        }
    }
    assert!(count > 0);
    count
}

#[test]
fn input_backpressure_does_not_stop_terminal_output() {
    crate::children::runtime().unwrap().block_on(async {
        if maestro_test_process::run_alone("input_backpressure_does_not_stop_terminal_output")
            .unwrap()
        {
            return;
        }
        let (socket, peer) = std::os::unix::net::UnixStream::pair().unwrap();
        let count = fill(&socket);
        peer.set_nonblocking(true).unwrap();
        let (tx, mut reads) = mpsc::unbounded_channel();
        let handle = PtyIoActor::spawn(PtyIoActorConfig {
            pane_id: 2,
            master_fd: socket.into(),
            initially_quiesced: false,
            on_read: Box::new(move |bytes| {
                tx.send(bytes.to_vec()).unwrap();
                PtyReadResult::default()
            }),
            on_reader_exit: None,
        })
        .unwrap();
        let mut peer = tokio::net::UnixStream::from_std(peer).unwrap();
        handle.try_write_user_input(b"pending".to_vec()).unwrap();
        peer.write_all(b"output-while-blocked").await.unwrap();
        assert_eq!(reads.recv().await.unwrap(), b"output-while-blocked");
        let mut bytes = vec![0; count + 7];
        peer.read_exact(&mut bytes).await.unwrap();
        assert!(bytes[..count].iter().all(|byte| *byte == 0xaa));
        assert_eq!(&bytes[count..], b"pending");
        finish(handle).await;
    });
}

/// Checks the exact one-second boundary of a held control reply.
async fn reply_deadline<T>(
    future: impl std::future::Future<Output = std::io::Result<T>>,
    text: &str,
) {
    let Err(error) = reply_boundary(future).await else {
        panic!("reply unexpectedly succeeded");
    };
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    assert_eq!(error.to_string(), text);
}

/// Polls the same held reply immediately before and at its one-second boundary.
async fn reply_boundary<F: std::future::Future>(future: F) -> F::Output {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
    tokio::pin!(future);
    assert!(poll_once(future.as_mut()).is_pending());
    tokio::time::advance(std::time::Duration::from_millis(999)).await;
    assert!(poll_once(future.as_mut()).is_pending());
    tokio::time::advance(std::time::Duration::from_millis(1)).await;
    finish_at(future, deadline).await
}

#[test]
fn actor_deadlines_restore_admission_without_losing_pending_bytes() {
    crate::tests::runtime::paused_runtime().block_on(async {
        if maestro_test_process::run_alone(
            "actor_deadlines_restore_admission_without_losing_pending_bytes",
        )
        .unwrap()
        {
            return;
        }
        assert_control_reply_budgets().await;
        let (socket, peer) = std::os::unix::net::UnixStream::pair().unwrap();
        let count = fill(&socket);
        peer.set_nonblocking(true).unwrap();
        let mut runner = controlled_runner(socket.into(), Arc::default());
        runner.enqueue(b"kept".to_vec());
        {
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
            let begin = runner.begin_handoff();
            tokio::pin!(begin);
            assert!(poll_once(begin.as_mut()).is_pending());
            tokio::time::advance(std::time::Duration::from_millis(1999)).await;
            assert!(poll_once(begin.as_mut()).is_pending());
            tokio::time::advance(std::time::Duration::from_millis(1)).await;
            assert_eq!(
                finish_at(begin, deadline).await.unwrap_err().to_string(),
                "timed out draining PTY writes before handoff"
            );
        }
        let mut peer = tokio::net::UnixStream::from_std(peer).unwrap();
        let mut prefix = vec![0; count];
        peer.read_exact(&mut prefix).await.unwrap();
        runner.drain().await.unwrap();
        let mut bytes = [0; 4];
        peer.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"kept");
    });
}

/// A caller waker that holds one admitted channel send at a known boundary.
struct BlockingWake {
    /// Producer reached the caller's wake implementation.
    entered: std::sync::Barrier,
    /// Caller permits that wake to finish.
    release: std::sync::Barrier,
}
impl std::task::Wake for BlockingWake {
    fn wake(self: Arc<Self>) {
        self.entered.wait();
        self.release.wait();
    }
}

#[test]
fn admission_keeps_channel_wakes_through_handoff() {
    crate::children::runtime().unwrap().block_on(async {
        let (handle, mut data, mut commands) = parked_handle();
        let wake = Arc::new(BlockingWake {
            entered: std::sync::Barrier::new(2),
            release: std::sync::Barrier::new(2),
        });
        let waker = std::task::Waker::from(Arc::clone(&wake));
        let receive = data.recv();
        tokio::pin!(receive);
        assert!(
            receive
                .as_mut()
                .poll(&mut std::task::Context::from_waker(&waker))
                .is_pending()
        );
        let producer = handle.clone();
        let thread =
            std::thread::spawn(move || producer.try_write_user_input(b"admitted".to_vec()));
        wake.entered.wait();
        let begin = handle.begin_handoff(std::time::Duration::from_secs(1));
        tokio::pin!(begin);
        assert!(poll_once(begin.as_mut()).is_pending());
        let drained = handle.owner.admission.wait_drained();
        tokio::pin!(drained);
        assert!(poll_once(drained.as_mut()).is_pending());
        wake.release.wait();
        thread.join().unwrap().unwrap();
        drained.await;
        assert_eq!(receive.await.unwrap(), b"admitted");
        let controls::Control::Begin(reply) = commands.try_recv().unwrap() else {
            panic!("handoff control");
        };
        assert!(reply.send(Ok(())).is_ok());
        begin.await.unwrap();
    });
}

/// Checks retryable, closed and permanent read outcomes at the callback boundary.
fn assert_read_outcomes(events: &Events) {
    let start = events.mark();
    let (socket, _peer) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut probe = controlled_runner(socket.into(), Arc::default());
    assert!(probe.read_result(&[], Err(std::io::ErrorKind::Interrupted.into())));
    assert!(probe.read_result(&[], Err(std::io::ErrorKind::WouldBlock.into())));
    assert!(!probe.read_result(&[], Err(std::io::ErrorKind::BrokenPipe.into())));
    assert!(!probe.read_result(&[], Ok(0)));
    assert!(probe.read_result(b"x", Ok(1)));
    let error = std::io::Error::from(std::io::ErrorKind::BrokenPipe).to_string();
    events.assert_since(start, &[("PTY actor read failed", 7, Some(&error))]);
}

/// Checks exact partial-write offsets, retries, zero writes and permanent failures.
fn assert_write_outcomes(events: &Events) {
    let start = events.mark();
    let (socket, _peer) = std::os::unix::net::UnixStream::pair().unwrap();
    let probe = controlled_runner(socket.into(), Arc::default());
    let mut writes = Writes::default();
    writes.enqueue(vec![]);
    assert!(writes.is_empty());
    writes.enqueue(b"abc".to_vec());
    writes.enqueue(b"def".to_vec());
    let mut emitted = Vec::new();
    let mut first = true;
    assert!(matches!(
        writes.flush(|bytes| {
            if first {
                first = false;
                emitted.extend_from_slice(&bytes[..2]);
                Ok(2)
            } else {
                Err(std::io::ErrorKind::WouldBlock.into())
            }
        }),
        WriteStatus::Blocked
    ));
    assert!(matches!(
        writes.flush(|_| Err(std::io::ErrorKind::Interrupted.into())),
        WriteStatus::Blocked
    ));
    let status = writes.flush(|_| Ok(0));
    assert!(matches!(status, WriteStatus::Zero));
    probe.report_write_status(status);
    assert!(matches!(
        writes.flush(|bytes| {
            emitted.extend_from_slice(bytes);
            Ok(bytes.len())
        }),
        WriteStatus::Complete
    ));
    assert_eq!(emitted, b"abcdef");
    writes.enqueue(b"discard".to_vec());
    let status = writes.flush(|_| Err(std::io::ErrorKind::BrokenPipe.into()));
    assert!(
        matches!(&status,WriteStatus::Failed(error) if error.kind()==std::io::ErrorKind::BrokenPipe)
    );
    probe.report_write_status(status);
    let error = std::io::Error::from(std::io::ErrorKind::BrokenPipe).to_string();
    events.assert_since(
        start,
        &[
            ("PTY actor write returned zero bytes", 7, None),
            ("PTY actor write failed", 7, Some(&error)),
        ],
    );
    assert!(writes.is_empty());
}

/// Checks empty responses, dropped descriptor replies and closed-reader delivery.
async fn assert_closed_actor_replies(events: &Events) {
    let start = events.mark();
    let (handle, mut peer, mut reads) = fixture(true);
    let mut called = 0;
    handle.write_terminal_response(|| {
        called += 1;
        None
    });
    handle.write_terminal_response(|| {
        called += 1;
        Some(vec![])
    });
    assert_eq!(called, 2);
    handle.resize(
        crate::terminal::PtySize {
            rows: 1,
            cols: 1,
            cell_width_px: 0,
            cell_height_px: 0,
        },
        vec![b"r".to_vec()],
    );
    let mut marker = [0; 1];
    peer.read_exact(&mut marker).await.unwrap();
    assert_eq!(&marker, b"r");
    let error = std::io::Error::from_raw_os_error(25).to_string();
    events.assert_since(start, &[("PTY resize failed", 1, Some(&error))]);
    let start = events.mark();
    let before = std::fs::read_dir("/proc/self/fd").unwrap().count();
    let (reply, receiver) = tokio::sync::oneshot::channel();
    drop(receiver);
    assert!(
        handle
            .owner
            .control
            .send(super::controls::Control::Duplicate(reply))
            .is_ok()
    );
    assert_eq!(handle.foreground_process_group_id().await, None);
    assert_eq!(std::fs::read_dir("/proc/self/fd").unwrap().count(), before);
    handle.release_after_commit().await.unwrap();
    let mut bytes = Vec::new();
    peer.read_to_end(&mut bytes).await.unwrap();
    assert!(bytes.is_empty());
    assert!(reads.recv().await.is_none());
    events.assert_since(start, &[("PTY actor exiting", 1, None)]);
}

/// Joins EOF shutdown before checking the one-shot callback and captured ownership.
async fn assert_reader_exit_releases_callbacks(events: &Events) {
    let start = events.mark();
    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let exited = Arc::clone(&counter);
    let retained = Arc::clone(&counter);
    let (handle, peer) = callback_actor(
        false,
        Box::new(move |_| {
            let _ = &retained;
            PtyReadResult::default()
        }),
        Some(Box::new(move || {
            Box::pin(async move {
                exited.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            })
        })),
    );
    drop(peer);
    let task = handle.owner.task.lock().unwrap().take().unwrap();
    task.await.unwrap();
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(Arc::strong_count(&counter), 1);
    events.assert_since(start, &[("PTY actor exiting", 1, None)]);
}

/// Observes temporary and restored kernel geometry on both sides of the redraw boundary.
async fn assert_nudge_sizes(
    runner: &mut runner::Runner,
    slave: &std::fs::File,
    handle: &PtyIoActorHandle,
) {
    for (rows, cols, temporary_rows, temporary_cols) in
        [(10, 80, 9, 80), (2, 80, 2, 79), (1, 1, 1, 4), (0, 4, 0, 4)]
    {
        let size = crate::terminal::PtySize {
            rows,
            cols,
            cell_width_px: 7,
            cell_height_px: 11,
        };
        handle.resize(size, vec![]);
        runner.apply_controls().await;
        handle.nudge_child_redraw_after_handoff(size);
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(30);
        let apply = runner.apply_controls();
        tokio::pin!(apply);
        let first = poll_once(apply.as_mut());
        let observed = rustix::termios::tcgetwinsize(slave).unwrap();
        assert_eq!(
            (observed.ws_row, observed.ws_col),
            (temporary_rows, temporary_cols)
        );
        if (rows, cols) == (temporary_rows, temporary_cols) {
            assert!(first.is_ready());
            continue;
        }
        assert!(first.is_pending());
        tokio::time::advance(std::time::Duration::from_millis(29)).await;
        assert!(poll_once(apply.as_mut()).is_pending());
        tokio::time::advance(std::time::Duration::from_millis(1)).await;
        finish_at(apply, deadline).await;
        let observed = rustix::termios::tcgetwinsize(slave).unwrap();
        assert_eq!(
            (
                observed.ws_row,
                observed.ws_col,
                observed.ws_xpixel,
                observed.ws_ypixel
            ),
            (rows, cols, cols * 7, rows * 11)
        );
    }
}

/// Pins the one-second reply budgets and shorter caller-budget rollback independently.
async fn assert_control_reply_budgets() {
    let (handle, _data, mut commands) = parked_handle();
    reply_deadline(
        handle.duplicate_for_handoff(),
        "timed out waiting for PTY handoff duplicate",
    )
    .await;
    reply_deadline(
        handle.rollback_handoff(),
        "timed out waiting for PTY handoff rollback",
    )
    .await;
    reply_deadline(
        handle.release_after_commit(),
        "timed out waiting for PTY actor release",
    )
    .await;
    assert!(
        reply_boundary(handle.foreground_process_group_id())
            .await
            .is_none()
    );
    while commands.try_recv().is_ok() {}
    assert_shorter_caller_budget(&handle, &mut commands).await;
}

/// Checks that the caller deadline triggers rollback without confusing its reply budget.
async fn assert_shorter_caller_budget(
    handle: &PtyIoActorHandle,
    commands: &mut mpsc::UnboundedReceiver<controls::Control>,
) {
    let start = tokio::time::Instant::now();
    let writer = handle.clone();
    let begin = tokio::spawn(async move {
        writer
            .begin_handoff(std::time::Duration::from_millis(500))
            .await
    });
    assert!(matches!(
        commands.recv().await.unwrap(),
        controls::Control::Begin(_)
    ));
    tokio::pin!(begin);
    tokio::time::advance(std::time::Duration::from_millis(499)).await;
    assert!(poll_once(begin.as_mut()).is_pending());
    tokio::time::advance(std::time::Duration::from_millis(1)).await;
    let controls::Control::Rollback(reply) = commands.recv().await.unwrap() else {
        panic!("rollback after caller budget");
    };
    assert_eq!(
        tokio::time::Instant::now(),
        start + std::time::Duration::from_millis(500)
    );
    assert!(reply.send(Ok(())).is_ok());
    assert_eq!(
        begin.await.unwrap().unwrap_err().to_string(),
        "timed out waiting for PTY actor to quiesce"
    );
    assert!(!handle.owner.admission.closed());
}

/// Complete structured event fields, kept in emission order without deduplication.
#[derive(Clone, Default)]
struct Events(Arc<Mutex<Vec<EventFields>>>);
/// One tracing event's named fields in recording order.
#[derive(Clone, Default)]
struct EventFields(Vec<(&'static str, String)>);
impl tracing::field::Visit for EventFields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.push((field.name(), format!("{value:?}")));
    }
}
impl tracing::Subscriber for Events {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        let mut fields = EventFields::default();
        event.record(&mut fields);
        self.0.lock().unwrap().push(fields);
    }
}
impl Events {
    /// Marks a boundary before the operation whose emissions are asserted.
    fn mark(&self) -> usize {
        self.0.lock().unwrap().len()
    }

    /// Asserts the complete ordered event slice and its pane/error identities.
    fn assert_since(&self, start: usize, expected: &[(&str, u32, Option<&str>)]) {
        let events = self.0.lock().unwrap().clone();
        let actual: Vec<_> = events[start..]
            .iter()
            .map(|fields| {
                let get = |name| {
                    fields
                        .0
                        .iter()
                        .find(|(key, _)| *key == name)
                        .map(|(_, value)| value.as_str())
                };
                (
                    get("message").unwrap(),
                    get("pane").unwrap().parse::<u32>().unwrap(),
                    get("err"),
                )
            })
            .collect();
        assert_eq!(actual, expected);
    }
}

/// Allows timer delivery without permitting paused-clock auto-advance to hide a late deadline.
async fn finish_at<F: std::future::Future>(future: F, deadline: tokio::time::Instant) -> F::Output {
    let value = future.await;
    assert_eq!(tokio::time::Instant::now(), deadline);
    value
}

#[test]
fn reader_exit_keeps_blocking_runtime_notifications() {
    crate::children::runtime().unwrap().block_on(async {
        if maestro_test_process::run_alone("reader_exit_keeps_blocking_runtime_notifications")
            .unwrap()
        {
            return;
        }
        let (sender, mut receiver) = mpsc::channel(1);
        let runtime = tokio::runtime::Handle::current();
        let control = std::thread::spawn(move || runtime.block_on(sender.send(())));
        assert_eq!(receiver.recv().await, Some(()));
        control.join().unwrap().unwrap();
        let (sender, mut receiver) = mpsc::channel(1);
        let (handle, peer) = callback_actor(
            false,
            Box::new(|_| PtyReadResult::default()),
            Some(Box::new(move || {
                Box::pin(async move {
                    sender.send(()).await.unwrap();
                })
            })),
        );
        drop(peer);
        let task = handle.owner.task.lock().unwrap().take().unwrap();
        let joined = task.await;
        let delivered = receiver.recv().await;
        assert_eq!((joined.is_ok(), delivered), (true, Some(())));
    });
}

#[test]
fn exit_hook_completion_precedes_master_release() {
    crate::children::runtime().unwrap().block_on(async {
        if maestro_test_process::run_alone("exit_hook_completion_precedes_master_release").unwrap()
        {
            return;
        }
        let (master, _slave) = terminal_pair();
        let path = format!("/proc/self/fd/{}", master.as_raw_fd());
        let (sender, mut receiver) = mpsc::channel(1);
        sender.try_send(1).unwrap();
        let (started, ready) = tokio::sync::oneshot::channel();
        let handle = PtyIoActor::spawn(PtyIoActorConfig {
            pane_id: 3,
            master_fd: master,
            initially_quiesced: true,
            on_read: Box::new(|_| PtyReadResult::default()),
            on_reader_exit: Some(Box::new(move || {
                Box::pin(async move {
                    started.send(()).unwrap();
                    sender.send(2).await.unwrap();
                })
            })),
        })
        .unwrap();
        let release = handle.release_after_commit();
        tokio::pin!(release);
        assert!(poll_once(release.as_mut()).is_pending());
        ready.await.unwrap();
        assert!(std::fs::read_link(&path).is_ok());
        assert!(matches!(
            handle.try_write_user_input(vec![1]),
            Err(TrySendError::Closed(_))
        ));
        assert_eq!(receiver.recv().await, Some(1));
        release.await.unwrap();
        assert_eq!(receiver.recv().await, Some(2));
        assert_eq!(
            std::fs::read_link(path).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
    });
}

#[test]
fn exit_hook_cancellation_releases_master() {
    if maestro_test_process::run_alone("exit_hook_cancellation_releases_master").unwrap() {
        return;
    }
    let runtime = crate::children::runtime().unwrap();
    let (master, _slave) = terminal_pair();
    let path = format!("/proc/self/fd/{}", master.as_raw_fd());
    let (sender, mut receiver) = mpsc::channel(1);
    sender.try_send(1).unwrap();
    let (started, ready) = tokio::sync::oneshot::channel();
    let handle = runtime.block_on(async {
        let handle = PtyIoActor::spawn(PtyIoActorConfig {
            pane_id: 4,
            master_fd: master,
            initially_quiesced: true,
            on_read: Box::new(|_| PtyReadResult::default()),
            on_reader_exit: Some(Box::new(move || {
                Box::pin(async move {
                    started.send(()).unwrap();
                    sender.send(2).await.unwrap();
                })
            })),
        })
        .unwrap();
        handle.shutdown();
        ready.await.unwrap();
        handle
    });
    assert!(std::fs::read_link(&path).is_ok());
    drop(runtime);
    assert_eq!(
        std::fs::read_link(path).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    assert_eq!(receiver.blocking_recv(), Some(1));
    assert_eq!(receiver.blocking_recv(), None);
    let mut task = handle.owner.task.lock().unwrap().take().unwrap();
    assert!(
        matches!(poll_once(std::pin::Pin::new(&mut task)),std::task::Poll::Ready(Err(error)) if error.is_cancelled())
    );
}

#[test]
fn restored_master_is_not_inherited_by_children() {
    crate::children::runtime().unwrap().block_on(async {
        if maestro_test_process::run_alone("restored_master_is_not_inherited_by_children").unwrap(){return;}
        let (master,_slave)=terminal_pair();
        rustix::io::fcntl_setfd(&master,rustix::io::FdFlags::empty()).unwrap();
        assert!(!rustix::io::fcntl_getfd(&master).unwrap().contains(rustix::io::FdFlags::CLOEXEC));
        let number=master.as_raw_fd();
        let handle=PtyIoActor::spawn(PtyIoActorConfig {pane_id:5,master_fd:master,initially_quiesced:true,
            on_read:Box::new(|_|PtyReadResult::default()),on_reader_exit:None}).unwrap();
        let info=std::fs::read_to_string(format!("/proc/self/fdinfo/{number}")).unwrap();
        let flags=info.lines().find_map(|line|line.strip_prefix("flags:")).unwrap().trim();
        let cloexec=u64::from_str_radix(flags,8).unwrap() & u64::from(rustix::fs::OFlags::CLOEXEC.bits()) != 0;
        let output=std::process::Command::new("/bin/sh").args(["-c",r#"if [ -e "/proc/self/fd/$MAESTRO_TEST_MASTER_FD" ]; then printf leaked; else printf closed; fi"#])
            .env("MAESTRO_TEST_MASTER_FD",number.to_string()).output().unwrap();
        assert!(output.status.success());handle.release_after_commit().await.unwrap();
        assert_eq!((cloexec,output.stdout),(true,b"closed".to_vec()));
    });
}
