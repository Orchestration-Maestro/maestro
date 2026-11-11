//! Stream session text to the sample's console without waiting for the completed answer.
use maestro_agent::AgentEvent;
use maestro_app::{AgentSession, AgentSessionEvent};
use maestro_models::AssistantMessageEvent;
use std::{cell::RefCell, io::Write, rc::Rc};

/// Subscribe until the caller finishes its prompt, retaining any console error.
pub fn subscribe(session: &AgentSession) -> impl FnOnce() -> Result<(), crate::support::Error> {
    let error = Rc::new(RefCell::new(None));
    let failed = Rc::clone(&error);
    let unsubscribe = session.subscribe(Rc::new(move |event| {
        if let AgentSessionEvent::Agent(event) = event
            && let Ok(event) = event.read()
            && let AgentEvent::MessageUpdate {
                assistant_message_event: AssistantMessageEvent::TextDelta { delta, .. },
                ..
            } = &*event
            && failed.borrow().is_none()
        {
            let mut output = std::io::stdout();
            *failed.borrow_mut() = output
                .write_all(delta.as_bytes())
                .and_then(|()| output.flush())
                .err();
        }
    }));
    move || {
        unsubscribe();
        if let Some(error) = error.borrow_mut().take() {
            return Err(error.into());
        }
        Ok(())
    }
}
