//! Generates the session title and shows the header name, title button or title editor.
use super::{ExampleMessages, NAME, State, element, js, report};
use maestro_app::{AgentMessage, UserBlock, UserContent};
use maestro_web::components::input::{Input, InputProps, InputSize};
use std::{rc::Rc, sync::PoisonError};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::{Event, HtmlElement};

/// The text of the first user message: its text, or its text blocks joined by spaces.
fn first_user_text(messages: &[AgentMessage<ExampleMessages>]) -> Option<String> {
    let text = |content: &UserContent| match content {
        UserContent::Text(text) => text.clone(),
        UserContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|block| match block {
                UserBlock::Text(text) => Some(text.text.as_str()),
                UserBlock::Image(_) => None,
            })
            .collect::<Vec<_>>()
            .join(" "),
    };
    messages.iter().find_map(|message| match message {
        AgentMessage::User(user) => Some(text(
            &user.read().unwrap_or_else(PoisonError::into_inner).content,
        )),
        AgentMessage::Custom(custom) => {
            match &*custom.read().unwrap_or_else(PoisonError::into_inner) {
                ExampleMessages::UserWithAttachments(user) => Some(text(&user.content)),
                _ => None,
            }
        }
        _ => None,
    })
}
/// A title from the first user message: its first sentence when that ends within 50
/// characters, else the text itself up to 50 characters, else its first 47 and an ellipsis.
#[must_use]
pub fn generate_title(messages: &[AgentMessage<ExampleMessages>]) -> String {
    let Some(text) = first_user_text(messages) else {
        return String::new();
    };
    let text = text.trim();
    let end = text.chars().position(|c| matches!(c, '.' | '!' | '?'));
    if let Some(end) = end.filter(|end| (1..=50).contains(end)) {
        return text.chars().take(end + 1).collect();
    }
    if text.chars().count() <= 50 {
        return text.into();
    }
    let mut title: String = text.chars().take(47).collect();
    title.push_str("...");
    title
}
/// Whether the history holds a user message and an assistant response.
#[must_use]
pub fn should_save_session(messages: &[AgentMessage<ExampleMessages>]) -> bool {
    let user = messages.iter().any(|message| match message {
        AgentMessage::User(_) => true,
        AgentMessage::Custom(custom) => matches!(
            &*custom.read().unwrap_or_else(PoisonError::into_inner),
            ExampleMessages::UserWithAttachments(_)
        ),
        _ => false,
    });
    user && messages
        .iter()
        .any(|message| matches!(message, AgentMessage::Assistant(_)))
}
impl State {
    /// Show the name without a title, the title as an edit button, or the title editor.
    pub(super) fn render_title(self: &Rc<Self>) -> Result<(), JsValue> {
        let (title, editing) = {
            let current = self.current.borrow();
            (current.title.clone(), current.editing_title)
        };
        if title.is_empty() {
            let name = element(&self.view, "span", "example-name")?;
            name.set_text_content(Some(NAME));
            self.title.replace_children_with_node_1(&name);
            self.title_input.replace(None);
            return Ok(());
        }
        if editing {
            return self.edit_title(&title);
        }
        self.title_input.replace(None);
        let button = element(&self.view, "button", "example-title-button")?;
        button.set_attribute("type", "button")?;
        button.set_attribute("title", "Click to edit title")?;
        button.set_text_content(Some(&title));
        let listener = Closure::<dyn Fn(Event)>::new(self.weakly(|state, _: Event| {
            state.current.borrow_mut().editing_title = true;
            report(state.render_title());
        }));
        button.add_event_listener_with_callback("click", listener.as_ref().unchecked_ref())?;
        self.title_listener.replace(Some(listener));
        self.title.replace_children_with_node_1(&button);
        Ok(())
    }
    /// Show the title editor with its text selected: a change or Enter keeps a new nonblank
    /// title, and Escape keeps the old one.
    fn edit_title(self: &Rc<Self>, title: &str) -> Result<(), JsValue> {
        let commit = Rc::new(self.weakly(|state, event: Event| {
            let value = event
                .target()
                .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
                .map(|field| field.value());
            state.finish_title(value);
        }));
        let on_change = Rc::clone(&commit);
        let cancel = self.weakly(|state, ()| state.finish_title(None));
        let input = Input::new(
            &self.view,
            InputProps {
                value: title.into(),
                size: InputSize::Sm,
                class_name: "example-title-input".into(),
                on_change: Some(Rc::new(move |event| on_change(event))),
                on_key_down: Some(Rc::new(move |event: web_sys::KeyboardEvent| {
                    match event.key().as_str() {
                        "Enter" => commit(event.into()),
                        "Escape" => cancel(()),
                        _ => {}
                    }
                })),
                ..InputProps::default()
            },
        )
        .map_err(js)?;
        self.title.replace_children_with_node_1(input.root());
        let field: &HtmlElement = input.input().unchecked_ref();
        field.focus()?;
        input.input().select();
        self.title_input.replace(Some(input));
        Ok(())
    }
    /// End title editing on the page's task queue, storing a new title `value` first.
    fn finish_title(self: &Rc<Self>, value: Option<String>) {
        self.spawn(|state| async move { state.commit_title(value).await });
    }
    /// End title editing, storing `value` first when it is a new nonblank title of a stored
    /// session.
    async fn commit_title(self: Rc<Self>, value: Option<String>) -> Result<(), JsValue> {
        let title = value.map(|value| value.trim().to_owned());
        let id = self.current.borrow().session_id.clone();
        let changed =
            title.filter(|title| !title.is_empty() && *title != self.current.borrow().title);
        if let (Some(title), Some(id)) = (changed, id) {
            self.storage
                .sessions
                .update_title(&id, &title)
                .await
                .map_err(js)?;
            self.current.borrow_mut().title = title;
        }
        let editing = std::mem::replace(&mut self.current.borrow_mut().editing_title, false);
        if editing {
            self.render_title()?;
        }
        Ok(())
    }
}
