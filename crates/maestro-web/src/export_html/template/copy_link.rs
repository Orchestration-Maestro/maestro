//! Literal message-link preparation with weak asynchronous feedback delivery.
use crate::browser::{
    attachment_content::{self, AttachmentHost, AttachmentUiError},
    timer::Timer,
};
use std::{cell::RefCell, rc::Rc};
mod draw;
pub use draw::{CopyLinkView, render_copy_link_button};

/// One retained message control with an owned restoring timer.
pub struct CopyLinkState {
    /// Replaceable effects owner.
    host: Rc<dyn AttachmentHost>,
    /// Stable toolkit identity, unrelated to literal message identity.
    id: egui::Id,
    /// Pending clipboard work cannot retain this UI owner.
    feedback: Rc<RefCell<Option<Timer>>>,
    /// Primary gesture admitted at its first endpoint.
    pressed: Option<()>,
}
impl CopyLinkState {
    /// Retain the supplied browser effects and a stable control identity.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            host,
            id: attachment_content::identity(),
            feedback: Rc::default(),
            pressed: None,
        }
    }
    /// Begin the native effect before scheduling its owned completion.
    fn copy(&self, text: &str, context: &egui::Context) {
        let work = self.host.copy_to_clipboard(text);
        let feedback = Rc::downgrade(&self.feedback);
        let host = Rc::downgrade(&self.host);
        let context = context.clone();
        self.host.spawn_local(Box::pin(async move {
            if !work.await {
                return;
            }
            let (Some(feedback), Some(host)) = (feedback.upgrade(), host.upgrade()) else {
                return;
            };
            let timer = host.timer(
                1500,
                false,
                restore(Rc::downgrade(&feedback), context.clone()),
            );
            let old = feedback.replace(timer.ok());
            drop(old);
            context.request_repaint();
        }));
    }
}
/// Restore idle feedback without retaining its UI owner.
fn restore(weak: std::rc::Weak<RefCell<Option<Timer>>>, context: egui::Context) -> Rc<dyn Fn()> {
    Rc::new(move || {
        let Some(feedback) = weak.upgrade() else {
            return;
        };
        let old = feedback.borrow_mut().take();
        drop(old);
        context.request_repaint();
    })
}
/// Escapes query syntax while preserving healthy opaque-key punctuation.
const KEY_ESCAPES: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
    .add(b'%')
    .add(b'&')
    .add(b'=')
    .add(b'+');
/// Prepare an activation-time URL without converting identities into markup.
fn build_share_url(entry: &str, view: &CopyLinkView<'_>) -> Result<String, AttachmentUiError> {
    let mut url = url::Url::parse(view.location_href)
        .map_err(|error| AttachmentUiError::Presentation(error.to_string()))?;
    let selected = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("leafId", view.current_leaf_id)
        .append_pair("targetId", entry)
        .finish();
    if let Some(base) = view.share_base_url {
        let (base, fragment) = base
            .split_once('#')
            .map_or((base, None), |(base, tail)| (base, Some(tail)));
        return Ok(format!(
            "{base}&{selected}{}",
            fragment.map_or_else(String::new, |tail| format!("#{tail}"))
        ));
    }
    let mut seen = std::collections::HashSet::new();
    let key = url.query_pairs().find_map(|(key, value)| {
        (seen.insert(key.to_string()) && value.is_empty() && key != "leafId" && key != "targetId")
            .then_some(key.into_owned())
    });
    let query = match key.filter(|key| !key.is_empty()) {
        Some(key) => {
            let escaped = percent_encoding::utf8_percent_encode(&key, KEY_ESCAPES);
            format!("{escaped}&{selected}")
        }
        None => selected,
    };
    url.set_query(Some(&query));
    Ok(url.into())
}
