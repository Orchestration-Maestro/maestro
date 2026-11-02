//! Extension custom views shown in place of the editor or as overlays.
use super::super::InteractiveMode;
use crate::components::extension_widgets::keybindings;
use crate::whole_cell_width::whole_cell_width;
use maestro_app::{
    CustomClose, CustomComponentFactory, CustomFactoryOutcome, CustomUIOptions, EventError,
    ExtensionComponent, ExtensionUIFuture, Listeners, OverlayOptionsSource,
};
use maestro_tui::tui::ComponentHandle;
use maestro_tui::{OverlayHandle, OverlayOptions, SizeValue};
use std::{
    cell::RefCell,
    error::Error,
    fmt,
    rc::{Rc, Weak},
    sync::Arc,
};
use tokio::sync::oneshot;

/// The custom views the mode keeps until each settles.
pub(in super::super) type CustomViews = Rc<RefCell<Listeners<Rc<CustomView>>>>;

/// A shown overlay's handle, shared with the extension.
type SharedHandle = Rc<RefCell<Box<dyn OverlayHandle>>>;

/// A failed close: the original failure, shared by every close and the observation.
#[derive(Debug)]
struct CustomCloseFailure(Arc<dyn Error + Send + Sync>);

impl fmt::Display for CustomCloseFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Error for CustomCloseFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&*self.0)
    }
}

/// Where a custom view is in its life.
enum Phase {
    /// Building or shown.
    Open,
    /// Settled without a close failure.
    Closed,
    /// Its close failed with this original failure.
    Failed(Arc<dyn Error + Send + Sync>),
}

/// One custom view: its options, the editor text it restores, its component and overlay,
/// kept by the mode until it settles.
pub(in super::super) struct CustomView {
    /// The mode that shows the view.
    mode: Weak<InteractiveMode>,
    /// Whether the view is an overlay rather than in place of the editor.
    overlay: bool,
    /// The extension's options until setup takes them or the view settles.
    options: RefCell<Option<CustomUIOptions>>,
    /// The current editor's raw text when the view started.
    saved_text: String,
    /// Where the view is in its life.
    phase: RefCell<Phase>,
    /// The built component, until the view settles.
    component: RefCell<Option<Rc<ExtensionComponent>>>,
    /// The view's own overlay, until the view settles.
    handle: RefCell<Option<SharedHandle>>,
    /// Settles the observation.
    settle: RefCell<Option<oneshot::Sender<Result<(), EventError>>>>,
    /// Removes the view from the mode's views.
    release: RefCell<Option<Box<dyn Fn()>>>,
}

impl InteractiveMode {
    /// Shows the component `factory` builds in place of the editor, or as an overlay with the
    /// options the extension supplies or its own width; the returned future settles when the
    /// view closes or fails. The mode keeps the view and its building work, so dropping the
    /// future neither closes nor cancels anything.
    pub(super) fn show_extension_custom(
        &self,
        factory: &CustomComponentFactory,
        options: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        let saved_text = match self.current_editor().try_get_text() {
            Ok(text) => text,
            Err(error) => return Box::pin(std::future::ready(Err(error))),
        };
        let options = options.unwrap_or_default();
        let (settle, settled) = oneshot::channel();
        let view = Rc::new(CustomView {
            mode: self.this.clone(),
            overlay: options.overlay.unwrap_or(false),
            options: RefCell::new(Some(options)),
            saved_text,
            phase: RefCell::new(Phase::Open),
            component: RefCell::default(),
            handle: RefCell::default(),
            settle: RefCell::new(Some(settle)),
            release: RefCell::default(),
        });
        let release = Listeners::subscribe(&self.custom_views, Rc::clone(&view));
        view.release.replace(Some(Box::new(release)));
        let closing = Rc::clone(&view);
        let close: CustomClose = Rc::new(move || closing.close());
        let keys = keybindings(self.keybindings.clone());
        match factory.call((self.ui.as_ref().clone(), self.themes.theme(), keys, close)) {
            Ok(CustomFactoryOutcome::Ready(component)) => view.install(component),
            Ok(CustomFactoryOutcome::Async(building)) => {
                self.tool_environment
                    .runtime
                    .spawn_local(Box::pin(async move {
                        match building.await {
                            Ok(component) => view.install(component),
                            Err(error) => view.fail(error),
                        }
                        Ok(())
                    }));
            }
            Err(error) => view.fail(error),
        }
        Box::pin(async move { settled.await.unwrap_or(Ok(())) })
    }

    /// Settles every custom view that has not settled without a value: each removes its own
    /// overlay and disposes its component once, and the editor reset puts the editor back.
    ///
    /// # Errors
    /// Returns the first overlay removal failure, after that view settled.
    pub(super) fn abandon_custom_views(&self) -> Result<(), EventError> {
        Listeners::try_for_each_live(&self.custom_views, CustomView::abandon)
    }

    /// Removes an inline custom view's `shown` component from the dialog stack, when it was
    /// built, then puts `text` back in the current editor.
    ///
    /// # Errors
    /// Returns the editor's original failure, after the stack changed.
    fn restore_custom_editor(
        &self,
        shown: Option<&ComponentHandle>,
        text: &str,
    ) -> Result<(), EventError> {
        if let Some(shown) = shown {
            self.remove_dialog(shown);
        }
        self.current_editor().try_set_text(text)?;
        self.ui.request_render(false);
        Ok(())
    }
}

impl CustomView {
    /// Whether the view has not settled.
    fn is_open(&self) -> bool {
        matches!(*self.phase.borrow(), Phase::Open)
    }

    /// Closes the view once: removes its own overlay or restores the editor, settles, then
    /// disposes the component, ignoring a disposal failure because the close stands. A later
    /// close returns `false`, or the first close's failure again.
    ///
    /// # Errors
    /// Returns the failure to remove the overlay or restore the editor, shared with the
    /// observation.
    fn close(&self) -> Result<bool, EventError> {
        let failed = match &*self.phase.borrow() {
            Phase::Open => None,
            Phase::Closed => return Ok(false),
            Phase::Failed(failure) => Some(Arc::clone(failure)),
        };
        if let Some(failure) = failed {
            return Err(Box::new(CustomCloseFailure(failure)));
        }
        drop(self.phase.replace(Phase::Closed));
        if let Err(error) = self.remove() {
            let failure: Arc<dyn Error + Send + Sync> = Arc::from(error);
            drop(self.phase.replace(Phase::Failed(Arc::clone(&failure))));
            self.finish(Err(Box::new(CustomCloseFailure(Arc::clone(&failure)))));
            return Err(Box::new(CustomCloseFailure(failure)));
        }
        let component = self.component.take();
        self.finish(Ok(()));
        if let Some(component) = component {
            dispose(&component);
        }
        Ok(true)
    }

    /// Settles an open view without a value: removes its own overlay, leaves the editor to
    /// the editor reset and disposes the component once, ignoring a disposal failure.
    ///
    /// # Errors
    /// Returns the failure to remove the overlay, after the view settled.
    fn abandon(&self) -> Result<(), EventError> {
        if !self.is_open() {
            return Ok(());
        }
        drop(self.phase.replace(Phase::Closed));
        let handle = self.handle.borrow().clone().filter(|_| self.overlay);
        let removed = handle.map_or(Ok(()), |handle| handle.borrow_mut().hide());
        let component = self.component.take();
        self.finish(Ok(()));
        if let Some(component) = component {
            dispose(&component);
        }
        Ok(removed?)
    }

    /// Removes the view's own overlay, or takes an inline view off the dialog stack and puts
    /// the saved text back in the editor.
    ///
    /// # Errors
    /// Returns the overlay's or the editor's failure.
    fn remove(&self) -> Result<(), EventError> {
        let Some(mode) = self.mode.upgrade() else {
            return Ok(());
        };
        if !self.overlay {
            let shown = self
                .component
                .borrow()
                .as_ref()
                .map(|component| Rc::clone(&component.component));
            return mode.restore_custom_editor(shown.as_ref(), &self.saved_text);
        }
        let handle = self.handle.borrow().clone();
        if let Some(handle) = handle {
            handle.borrow_mut().hide()?;
        }
        Ok(())
    }

    /// Shows a built component, or disposes it without showing it when the view already
    /// closed; a failure to show it fails the view.
    fn install(&self, component: ExtensionComponent) {
        let component = Rc::new(component);
        let mode = self.mode.upgrade();
        let Some(mode) = mode.filter(|_| self.is_open()) else {
            dispose(&component);
            return;
        };
        self.component.replace(Some(Rc::clone(&component)));
        if !self.overlay {
            mode.push_dialog(Rc::clone(&component.component), None);
        } else if let Err(error) = self.show_overlay(&mode, &component) {
            self.fail(error);
        }
    }

    /// Shows the component as an overlay with the supplied options, or its own nonzero
    /// width in whole cells, then hands the extension its handle. Closing while the options
    /// resolve shows nothing.
    ///
    /// A terminal failure settles the view before its component is disposed.
    ///
    /// # Errors
    /// Returns the resolver's or the handle callback's failure.
    fn show_overlay(
        &self,
        mode: &InteractiveMode,
        component: &ExtensionComponent,
    ) -> Result<(), EventError> {
        let setup = self.options.take().unwrap_or_default();
        let options = match &setup.overlay_options {
            Some(OverlayOptionsSource::Static(options)) => Some(Rc::clone(options)),
            Some(OverlayOptionsSource::Resolver(resolve)) => Some(resolve.call(())?),
            None => component.width.filter(|width| *width != 0.0).map(|width| {
                let cells = isize::try_from(whole_cell_width(width)).unwrap_or(isize::MAX);
                Rc::new(RefCell::new(OverlayOptions {
                    width: Some(SizeValue::Cells(cells)),
                    ..OverlayOptions::default()
                }))
            }),
        };
        if !self.is_open() {
            return Ok(());
        }
        let mut shown = match mode
            .ui
            .show_overlay(Rc::clone(&component.component), options)
        {
            Ok(shown) => shown,
            Err(error) => {
                if self.is_open() {
                    self.fail(error.into());
                    dispose(component);
                }
                return Ok(());
            }
        };
        if !self.is_open() {
            shown.hide()?;
            return Ok(());
        }
        let handle: SharedHandle = Rc::new(RefCell::new(shown));
        self.handle.replace(Some(Rc::clone(&handle)));
        if let Some(on_handle) = &setup.on_handle {
            on_handle.call(handle)?;
        }
        Ok(())
    }

    /// Fails an open view with `error`: an inline view restores the editor and a shown
    /// overlay is removed and its component disposed. Overlay cleanup keeps the initiating
    /// error; a failed inline restoration settles with its own failure instead.
    fn fail(&self, error: EventError) {
        if !self.is_open() {
            return;
        }
        drop(self.phase.replace(Phase::Closed));
        let admitted = self.handle.borrow().is_some();
        let removed = if admitted || !self.overlay {
            self.remove()
        } else {
            Ok(())
        };
        let component = self.component.take();
        if let Some(component) = component.filter(|_| admitted) {
            dispose(&component);
        }
        let error = if self.overlay {
            error
        } else {
            removed.err().unwrap_or(error)
        };
        self.finish(Err(error));
    }

    /// Settles the observation and releases the component, the overlay and the mode's hold
    /// on the view.
    fn finish(&self, result: Result<(), EventError>) {
        if let Some(settle) = self.settle.take() {
            drop(settle.send(result));
        }
        drop(self.options.take());
        drop(self.handle.take());
        drop(self.component.take());
        if let Some(release) = self.release.take() {
            release();
        }
    }
}

/// Disposes `component`; its failure is ignored, as the view's outcome already stands.
fn dispose(component: &ExtensionComponent) {
    if let Some(dispose) = &component.dispose {
        drop(dispose.call(()));
    }
}
