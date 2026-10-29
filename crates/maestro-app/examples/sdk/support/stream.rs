//! Stream session text to the sample's console without waiting for the completed answer.
use maestro_agent::AgentEvent;
use maestro_app::{AgentSession, AgentSessionEvent};
use maestro_models::AssistantMessageEvent;
use std::{cell::RefCell, io::Write, rc::Rc};

/// Subscribe until the caller finishes its prompt, retaining text and any console error.
pub fn subscribe(
    session: &AgentSession,
    output: Rc<RefCell<dyn Write>>,
) -> impl FnOnce() -> Result<String, crate::support::Error> {
    let text = Rc::new(RefCell::new(String::new()));
    let streamed = Rc::clone(&text);
    let error = Rc::new(RefCell::new(None));
    let failed = Rc::clone(&error);
    let unsubscribe = session.subscribe(Rc::new(move |event| {
        if let AgentSessionEvent::Agent(event) = event
            && let Ok(event) = event.read()
            && let AgentEvent::MessageUpdate {
                assistant_message_event: AssistantMessageEvent::TextDelta { delta, .. },
                ..
            } = &*event
        {
            streamed.borrow_mut().push_str(delta);
            if failed.borrow().is_none() {
                let mut output = output.borrow_mut();
                *failed.borrow_mut() = output
                    .write_all(delta.as_bytes())
                    .and_then(|()| output.flush())
                    .err();
            }
        }
    }));
    move || {
        unsubscribe();
        if let Some(error) = error.borrow_mut().take() {
            return Err(error.into());
        }
        Ok(text.borrow().clone())
    }
}
