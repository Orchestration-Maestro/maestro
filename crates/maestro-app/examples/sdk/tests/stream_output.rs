//! Real-session qualification of live console delivery and listener cleanup.
use super::{
    super::*,
    writer::{Failure, RecordingWriter},
};
use maestro_agent::AgentEvent;
use maestro_app::AgentSessionEvent;
use maestro_models::AssistantMessageEvent;
use std::cell::RefCell;

/// Queue enough text to force several live deltas without timed pacing.
fn queue_answer(sample: &Sample) {
    sample.provider.set_responses(vec![FauxResponseStep::Message(Box::new(
        faux_assistant_message("A controlled answer long enough to span several text deltas and exercise later writes.", FauxAssistantMessageOptions::default()),
    ))]);
}

/// Each delta is written and flushed before the next session listener sees it.
#[test]
fn stream_writes_and_flushes_each_live_delta() -> Result<(), Error> {
    support::execute(|sample| async move {
        queue_answer(&sample);
        let created = create_agent_session(sample.options()?).await?;
        let output = Rc::new(RefCell::new(RecordingWriter::default()));
        let finish = stream::subscribe(&created.session, output.clone());
        let observed = Rc::clone(&output);
        let deltas = Rc::new(RefCell::new(String::new()));
        let received = Rc::clone(&deltas);
        let immediate = Rc::new(RefCell::new(Vec::new()));
        let observations = Rc::clone(&immediate);
        let remove_observer = created.session.subscribe(Rc::new(move |event| {
            if let AgentSessionEvent::Agent(event) = event
                && let Ok(event) = event.read()
                && let AgentEvent::MessageUpdate {
                    assistant_message_event: AssistantMessageEvent::TextDelta { delta, .. },
                    ..
                } = &*event
            {
                received.borrow_mut().push_str(delta);
                let calls = &observed.borrow().calls;
                observations
                    .borrow_mut()
                    .push(calls.ends_with(&[delta.clone(), "flush".into()]));
            }
        }));
        created
            .session
            .prompt("Answer once", PromptOptions::default())
            .await?;
        remove_observer();
        assert_eq!(finish()?, *deltas.borrow());
        assert!(immediate.borrow().iter().all(|observed| *observed));
        assert!(output.borrow().calls.len() >= 4);
        assert_eq!(Rc::strong_count(&output), 1);
        created.session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

/// A write or flush failure survives later deltas and releases the subscription.
#[test]
fn stream_retains_first_error_and_suppresses_later_writes() -> Result<(), Error> {
    for (failure, expected, attempts) in [
        (Failure::Write, "write failure 1", 1),
        (Failure::Flush, "flush failure 2", 2),
    ] {
        support::execute(|sample| async move {
            let created = create_agent_session(sample.options()?).await?;
            let output = Rc::new(RefCell::new(RecordingWriter {
                failure: Some(failure),
                ..RecordingWriter::default()
            }));
            let finish = stream::subscribe(&created.session, output.clone());
            for _ in 0..2 {
                queue_answer(&sample);
                created
                    .session
                    .prompt("Answer again", PromptOptions::default())
                    .await?;
            }
            assert_eq!(finish().unwrap_err().to_string(), expected);
            assert_eq!(output.borrow().calls.len(), attempts);
            assert_eq!(Rc::strong_count(&output), 1);
            created.session.dispose();
            Ok(Value::Null)
        })?;
    }
    Ok(())
}
