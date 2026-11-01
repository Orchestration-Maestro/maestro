//! Authored styling and highlighting functions a host widget calls while it renders.
use super::{Call, Handle};
use crate::lifecycle::replies::Reply;
use crate::local_resources::Registry;
use maestro_extensions::widgets::BorderColor;
use std::{cell::RefCell, rc::Rc};

/// One call of an authored styling or highlighting function.
pub(crate) enum PaintCall<H = Handle> {
    /// Styles text.
    Style(H, String, Reply<String>),
    /// Highlights code with its optional language.
    Highlight(H, String, Option<String>, Reply<Vec<String>>),
}

impl<H> PaintCall<H> {
    /// The authored function's identity.
    pub(crate) fn handler(&self) -> &H {
        match self {
            Self::Style(handler, ..) | Self::Highlight(handler, ..) => handler,
        }
    }
    /// Moves the call to another identity owner.
    pub(crate) fn map<G>(self, handler: impl FnOnce(H) -> G) -> PaintCall<G> {
        match self {
            Self::Style(owner, text, reply) => PaintCall::Style(handler(owner), text, reply),
            Self::Highlight(owner, code, lang, reply) => {
                PaintCall::Highlight(handler(owner), code, lang, reply)
            }
        }
    }
}

/// The first failure of the authored functions one widget calls; the widget's next rendering
/// reports it.
pub(crate) type Failure = Rc<RefCell<Option<String>>>;

/// Code highlighting the toolkit calls.
pub(crate) type HighlightFn = dyn Fn(&str, Option<&str>) -> Vec<String>;

impl Handle {
    /// The authored style as a toolkit style; a failure keeps the text and is kept in `failure`.
    pub(crate) fn style(self, registry: Registry, failure: Failure) -> BorderColor {
        Rc::new(move |text: &str| {
            let styled = self.0.endpoint.synchronous(&registry, |reply| {
                Ok(Call::Paint(PaintCall::Style(
                    self.clone(),
                    text.to_owned(),
                    reply,
                )))
            });
            styled.unwrap_or_else(|error| {
                failure.borrow_mut().get_or_insert(error);
                text.to_owned()
            })
        })
    }

    /// The authored highlighting as toolkit highlighting; a failure keeps the code rows and is
    /// kept in `failure`.
    pub(crate) fn highlight(self, registry: Registry, failure: Failure) -> Rc<HighlightFn> {
        Rc::new(move |code: &str, lang: Option<&str>| {
            let rows = self.0.endpoint.synchronous(&registry, |reply| {
                let call =
                    PaintCall::Highlight(self.clone(), code.into(), lang.map(Into::into), reply);
                Ok(Call::Paint(call))
            });
            rows.unwrap_or_else(|error| {
                failure.borrow_mut().get_or_insert(error);
                code.lines().map(str::to_owned).collect()
            })
        })
    }
}
