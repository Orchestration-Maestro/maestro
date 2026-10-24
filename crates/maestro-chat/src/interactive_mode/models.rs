//! Model selection: the model panel and applying a chosen model to the session.
use super::InteractiveMode;
use crate::{ModelSelectorCallbacks, ModelSelectorComponent, ModelSelectorOptions};
use maestro_app::Model;
use maestro_tui::Component;
use std::{
    error::Error,
    rc::Rc,
    sync::{Arc, PoisonError, RwLock},
};

impl InteractiveMode {
    /// Selects `model` on the current session. On success it refreshes the footer and
    /// border, runs `done` and shows the model; on refusal it runs `done` and shows the
    /// session's error. The status reads the selected descriptor after the session's
    /// listeners ran.
    ///
    /// # Errors
    /// Returns the published theme's failure.
    async fn apply_model_selection(
        self: &Rc<Self>,
        model: Arc<RwLock<Model>>,
        done: Option<Rc<dyn Fn()>>,
    ) -> Result<(), Box<dyn Error>> {
        let session = self.runtime_host.session();
        let refused = session.set_model(Arc::clone(&model)).await.err();
        if refused.is_none() {
            self.footer.invalidate();
            self.update_editor_border_color()?;
        }
        if let Some(done) = done {
            done();
        }
        if let Some(error) = refused {
            return self.show_error(&error.message);
        }
        let id = model
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .id
            .clone();
        self.show_status(&format!("Model: {id}"))
    }

    /// The model panel's callbacks, which do not keep the mode alive: a selection is
    /// applied on the terminal runtime, owning the mode until it completes; cancelling
    /// puts the editor back and requests a frame.
    fn model_selector_callbacks(self: &Rc<Self>, done: Rc<dyn Fn()>) -> ModelSelectorCallbacks {
        let mode = Rc::downgrade(self);
        let selected = Rc::clone(&done);
        let cancelled = mode.clone();
        ModelSelectorCallbacks {
            on_select: Rc::new(move |model| {
                let Some(mode) = mode.upgrade() else {
                    return;
                };
                let done = Rc::clone(&selected);
                let runtime = Rc::clone(&mode.tool_environment.runtime);
                runtime.spawn_local(Box::pin(async move {
                    mode.apply_model_selection(model, Some(done)).await
                }));
            }),
            on_cancel: Rc::new(move || {
                done();
                if let Some(mode) = cancelled.upgrade() {
                    mode.ui.request_render(false);
                }
            }),
        }
    }

    /// Shows the model panel over the current session's settings, catalog, model and
    /// scope, with `initial_search_input` as its search.
    ///
    /// # Errors
    /// Returns the panel's catalog refresh, locale or key-hint failure.
    pub(super) fn show_model_selector(
        self: &Rc<Self>,
        initial_search_input: Option<String>,
    ) -> Result<(), Box<dyn Error>> {
        self.show_selector(|done| {
            let session = self.runtime_host.session();
            let selector = Rc::new(ModelSelectorComponent::new(
                &self.ui,
                Rc::clone(session.settings_manager()),
                session.model_registry(),
                self.model_selector_callbacks(done),
                ModelSelectorOptions {
                    current_model: Some(session.model()),
                    scoped_models: session.scoped_models(),
                    initial_search_input,
                    theme: self.themes.theme(),
                },
            )?);
            Ok((selector.clone(), selector))
        })
    }
}
