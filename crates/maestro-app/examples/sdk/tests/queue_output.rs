//! Real queue events qualify console error retention and listener removal.
use super::{
    super::*,
    writer::{Failure, RecordingWriter},
};

/// Queue output is immediate and removal stops both observations and writes.
#[test]
fn queue_output_is_immediate_and_unsubscribes() -> Result<(), Error> {
    support::execute(|sample| async move {
        let created = maestro_app::create_agent_session(sample.options()?).await?;
        let queues = Rc::new(RefCell::new(Vec::new()));
        let output = Rc::new(RefCell::new(RecordingWriter::default()));
        let finish = bind(created.session.clone(), queues.clone(), output.clone()).await?;
        created.session.steer("first", &[])?;
        assert_eq!(output.borrow().calls.concat(), "Queued: 1\n");
        assert_eq!(*queues.borrow(), [1]);
        finish()?;
        let before = output.borrow().calls.clone();
        created.session.steer("after removal", &[])?;
        assert_eq!(output.borrow().calls, before);
        assert_eq!(*queues.borrow(), [1]);
        assert_eq!(Rc::strong_count(&output), 1);
        created.session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

/// The first console failure is returned once, and later queue events do not write.
#[test]
fn queue_output_retains_first_error_and_suppresses_later_writes() -> Result<(), Error> {
    support::execute(|sample| async move {
        let created = maestro_app::create_agent_session(sample.options()?).await?;
        let queues = Rc::new(RefCell::new(Vec::new()));
        let output = Rc::new(RefCell::new(RecordingWriter {
            failure: Some(Failure::Write),
            ..RecordingWriter::default()
        }));
        let finish = bind(created.session.clone(), queues.clone(), output.clone()).await?;
        created.session.steer("first", &[])?;
        created.session.follow_up("second", &[])?;
        assert_eq!(*queues.borrow(), [1, 2]);
        assert_eq!(finish().unwrap_err().to_string(), "write failure 1");
        assert_eq!(output.borrow().calls.len(), 1);
        created.session.steer("after removal", &[])?;
        assert_eq!(*queues.borrow(), [1, 2]);
        assert_eq!(Rc::strong_count(&output), 1);
        assert!(finish().is_ok());
        created.session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

/// The shared recording sink can refuse a flush independently of byte writes.
#[test]
fn recording_writer_distinguishes_flush_failure() {
    use std::io::Write;
    let mut output = RecordingWriter {
        failure: Some(Failure::Flush),
        ..RecordingWriter::default()
    };
    output.write_all(b"text").unwrap();
    assert_eq!(output.flush().unwrap_err().to_string(), "flush failure 2");
}
