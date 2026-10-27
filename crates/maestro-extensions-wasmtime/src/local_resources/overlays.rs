//! Native overlay resources retain the toolkit's existing shared options owner.
use super::{Capability, pump::State};
use crate::guest_calls::{
    Handle,
    widget_calls::{Answered, Event},
};
use crate::import_route::Value;
use crate::overlay_reads::{
    HandleEndpoint, HandleRequest, OptionsEndpoint, OptionsRequest, OverlayMessage, Visibility,
};
use maestro_extensions::{
    EventBus, Local,
    types::UIErrorReporter,
    widgets::{OverlayHandle, OverlayOptions},
};
use std::{
    cell::{OnceCell, RefCell},
    rc::Rc,
};

/// A shown overlay's native handle, shared with the host.
pub(crate) type SharedHandle = Rc<RefCell<Box<dyn OverlayHandle>>>;

/// The native placement record and the error reporter its first custom consumer binds.
#[derive(Clone)]
pub(crate) struct Placement {
    /// The record an overlay retains.
    pub(crate) options: Rc<RefCell<OverlayOptions>>,
    /// Receives failed visibility checks once a custom component uses the record.
    pub(crate) reporter: Rc<OnceCell<UIErrorReporter>>,
}

impl State {
    /// Serves placement without borrowing the capability table across native effects.
    pub(super) fn overlays(&self, message: OverlayMessage) {
        match message {
            OverlayMessage::Close(id, reply) => {
                let capability = self.registry.capability(id);
                let result = match capability.as_deref() {
                    Some(Capability::CustomClose(close)) => {
                        close().map_err(|error| error.to_string())
                    }
                    _ => Err("extension custom close resource ended".to_owned()),
                };
                let _ = reply.send(result);
            }
            OverlayMessage::NewOptions(encoded, reply) => {
                let result = crate::overlay_layout::decode(&encoded).and_then(|options| {
                    self.registry.lend_placement(Placement {
                        options: Rc::new(RefCell::new(options)),
                        reporter: Rc::default(),
                    })
                });
                let _ = reply.send(result);
            }
            OverlayMessage::Options(id, request) => {
                let capability = self.registry.capability(id);
                match capability.as_deref() {
                    Some(Capability::OverlayOptions(placement)) => self.answer(placement, request),
                    _ => request.reject("extension overlay options resource ended"),
                }
            }
            OverlayMessage::Visible(visibility) => {
                let reply = visibility.reply.clone();
                let _ = reply.send(self.visible(visibility));
            }
            OverlayMessage::Handle(id, request, reply) => {
                let capability = self.registry.capability(id);
                let result = match capability.as_deref() {
                    Some(Capability::OverlayHandle(handle)) => handled(handle, request),
                    _ => Err("extension overlay handle resource ended".to_owned()),
                };
                let _ = reply.send(result);
            }
        }
    }

    /// Reads, replaces or shares placement on the same native record an overlay retains.
    fn answer(&self, placement: &Placement, request: OptionsRequest) {
        let options = &placement.options;
        match request {
            OptionsRequest::Get(reply) => {
                let _ = reply.send(crate::overlay_layout::encode(&options.borrow()));
            }
            OptionsRequest::Set(encoded, reply) => {
                let result = crate::overlay_layout::decode(&encoded).map(|mut replacement| {
                    replacement.visible.clone_from(&options.borrow().visible);
                    let previous = options.replace(replacement);
                    drop(previous);
                });
                let _ = reply.send(result);
            }
            OptionsRequest::Share(reply) => {
                let _ = reply.send(self.registry.lend_placement(placement.clone()));
            }
        }
    }

    /// Replaces the record's visibility check with one calling the authored check on the
    /// extension's bus.
    fn visible(&self, visibility: Visibility) -> Result<(), String> {
        let registry = &self.registry;
        let events = registry.events_of(visibility.api)?;
        let placement = match registry.capability(visibility.options.0.id).as_deref() {
            Some(Capability::OverlayOptions(placement)) => placement.clone(),
            _ => return Err("extension overlay options resource ended".to_owned()),
        };
        let check = visibility.visible.map(|handle| {
            let registry = registry.clone();
            let reporter = Rc::clone(&placement.reporter);
            Rc::new(move |columns: usize, rows: usize| {
                visible(&handle, (&registry, &events), (columns, rows), &reporter)
            }) as Rc<dyn Fn(usize, usize) -> bool>
        });
        let previous = std::mem::replace(&mut placement.options.borrow_mut().visible, check);
        drop(previous);
        Ok(())
    }
}

/// Runs the authored check; a failure is reported once bound and counts as hidden.
fn visible(
    handle: &Handle,
    (registry, events): (&super::Registry, &EventBus<Local>),
    (columns, rows): (usize, usize),
    reporter: &OnceCell<UIErrorReporter>,
) -> bool {
    let answer = match (u32::try_from(columns), u32::try_from(rows)) {
        (Ok(columns), Ok(rows)) => handle.widget(registry, events, Event::Visible(columns, rows)),
        _ => Err("the terminal size exceeds the visibility check".to_owned()),
    };
    let error = match answer {
        Ok(Answered::Visible(visible)) => return visible,
        Ok(_) => "overlay visibility answer kind mismatch".to_owned(),
        Err(error) => error,
    };
    if let Some(report) = reporter.get() {
        let _ = report("overlay.visible", &error.into());
    }
    false
}

/// Runs one request on a shown overlay's native handle.
fn handled(
    handle: &RefCell<Box<dyn OverlayHandle>>,
    request: HandleRequest,
) -> Result<Value, String> {
    let mut handle = handle
        .try_borrow_mut()
        .map_err(|_| "the overlay handle is already in use".to_owned())?;
    Ok(match request {
        HandleRequest::Hide => {
            handle.hide().map_err(|error| error.to_string())?;
            Value::Unit
        }
        HandleRequest::SetHidden(hidden) => {
            handle.set_hidden(hidden);
            Value::Unit
        }
        HandleRequest::IsHidden => Value::Boolean(handle.is_hidden()),
        HandleRequest::Focus => {
            handle.focus();
            Value::Unit
        }
        HandleRequest::Unfocus => {
            handle.unfocus();
            Value::Unit
        }
        HandleRequest::IsFocused => Value::Boolean(handle.is_focused()),
    })
}

impl super::Registry {
    /// Retains the original native close operation.
    pub(crate) fn custom_close(
        &self,
        close: maestro_extensions::types::CustomClose,
    ) -> Result<crate::custom_reads::CloseEndpoint, String> {
        self.insert(Capability::CustomClose(close))
            .map(crate::custom_reads::CloseEndpoint)
    }
    /// The bus of the extension whose API has this identity.
    pub(super) fn events_of(&self, api: u64) -> Result<EventBus<Local>, String> {
        match self.capability(api).as_deref() {
            Some(Capability::Api(api)) => Ok(api.events.clone()),
            _ => Err("extension API resource ended".to_owned()),
        }
    }
    /// Lends one owner of a placement record.
    fn lend_placement(&self, placement: Placement) -> Result<OptionsEndpoint, String> {
        self.insert(Capability::OverlayOptions(placement))
            .map(OptionsEndpoint)
    }
    /// Lends a shown overlay's native handle.
    pub(crate) fn overlay_handle(&self, handle: SharedHandle) -> Result<HandleEndpoint, String> {
        self.insert(Capability::OverlayHandle(handle))
            .map(HandleEndpoint)
    }
    /// The original shared native options owner, binding `reporter` when none is bound.
    pub(crate) fn overlay_options(
        &self,
        options: &OptionsEndpoint,
        reporter: Option<&UIErrorReporter>,
    ) -> Result<Rc<RefCell<OverlayOptions>>, String> {
        match self.capability(options.0.id).as_deref() {
            Some(Capability::OverlayOptions(placement)) => {
                if let Some(reporter) = reporter {
                    let _ = placement.reporter.set(Rc::clone(reporter));
                }
                Ok(Rc::clone(&placement.options))
            }
            _ => Err("extension overlay options resource ended".to_owned()),
        }
    }
}
