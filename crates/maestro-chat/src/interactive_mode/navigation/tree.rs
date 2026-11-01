//! Tree selection and navigation within the current session.
use super::InteractiveMode;
use crate::components::tree_selector::{
    FilterMode, TreeSelectorCallbacks, TreeSelectorComponent, TreeSelectorOptions,
};
use crate::interactive_mode::status_indicator::StatusKind;
use maestro_app::{
    DiagnosticErrorInfo, EventError, NavigateTreeOptions, NavigateTreeResult, Presence,
    ReadonlySessionManager, TreeFilterMode, is_whitespace_scalar,
};
use maestro_theme::ThemeColor;
use maestro_tui::Spacer;
use std::{error::Error, rc::Rc};

impl InteractiveMode {
    /// Opens the tree using the current leaf, saved filter and requested selection.
    pub(in crate::interactive_mode) fn show_tree_selector(
        self: &Rc<Self>,
        initial_selected_id: Option<String>,
    ) -> Result<(), Box<dyn Error>> {
        let session = self.runtime_host.session();
        if session.session_manager().borrow().get_tree().is_empty() {
            return self
                .show_status("No entries in session")
                .map_err(Into::into);
        }
        let filter = match session.settings_manager().borrow().get_tree_filter_mode() {
            TreeFilterMode::Default => FilterMode::Default,
            TreeFilterMode::NoTools => FilterMode::NoTools,
            TreeFilterMode::UserOnly => FilterMode::UserOnly,
            TreeFilterMode::LabeledOnly => FilterMode::LabeledOnly,
            TreeFilterMode::All => FilterMode::All,
        };
        self.show_selector(|done| {
            let manager = session.session_manager().borrow();
            let leaf = manager.get_leaf_id().map(str::to_owned);
            let selector = Rc::new(TreeSelectorComponent::new(
                &manager.get_tree(),
                self.tree_callbacks(done, leaf.clone()),
                TreeSelectorOptions {
                    current_leaf_id: leaf,
                    terminal_height: self.ui.terminal().borrow().rows(),
                    initial_selected_id,
                    initial_filter_mode: Some(filter),
                    theme: self.themes.theme(),
                    runtime: Rc::clone(&self.tool_environment.runtime),
                    styles_enabled: true,
                    now: None,
                },
            )?);
            Ok((selector.clone(), selector))
        })
    }

    /// Builds non-owning callbacks for selection, cancellation and persisted labels.
    fn tree_callbacks(
        self: &Rc<Self>,
        done: Rc<dyn Fn()>,
        leaf: Option<String>,
    ) -> TreeSelectorCallbacks {
        let mode = Rc::downgrade(self);
        TreeSelectorCallbacks {
            on_select: self.tree_selection(Rc::clone(&done), leaf),
            on_cancel: self.cancelled_selection(done),
            on_label_change: Some(Rc::new(move |id, label| {
                if let Some(mode) = mode.upgrade() {
                    let result = mode
                        .runtime_host
                        .session()
                        .session_manager()
                        .borrow_mut()
                        .append_label_change(
                            id,
                            label.map_or(Presence::Missing, Presence::Present),
                        );
                    mode.report(result.map(drop).map_err(Into::into));
                    mode.ui.request_render(false);
                }
            })),
        }
    }

    /// Closes the panel and admits navigation without retaining the mode in the panel.
    fn tree_selection(
        self: &Rc<Self>,
        done: Rc<dyn Fn()>,
        leaf: Option<String>,
    ) -> Rc<dyn Fn(&str)> {
        let mode = Rc::downgrade(self);
        Rc::new(move |id| {
            done();
            let Some(mode) = mode.upgrade() else {
                return;
            };
            if leaf.as_deref() == Some(id) {
                mode.report(
                    mode.show_status("Already at this point")
                        .map_err(Into::into),
                );
                return;
            }
            let id = id.to_owned();
            mode.tool_environment
                .runtime
                .clone()
                .spawn_local(Box::pin(
                    async move { mode.navigate_selected_tree(id).await },
                ));
        })
    }

    /// Asks for optional summary instructions, returning to the tree on cancellation.
    async fn tree_summary_options(&self) -> Result<Option<NavigateTreeOptions>, Box<dyn Error>> {
        if self
            .runtime_host
            .session()
            .settings_manager()
            .borrow()
            .get_branch_summary_skip_prompt()
        {
            return Ok(Some(NavigateTreeOptions {
                summarize: Some(false),
                ..NavigateTreeOptions::default()
            }));
        }
        loop {
            let choice = self
                .show_extension_selector(
                    "Summarize branch?".to_owned(),
                    ["No summary", "Summarize", "Summarize with custom prompt"]
                        .map(str::to_owned)
                        .to_vec(),
                    None,
                )
                .await
                .map_err(|error| error as Box<dyn Error>)?;
            let Some(choice) = choice else {
                return Ok(None);
            };
            let custom = choice == "Summarize with custom prompt";
            let custom_instructions = if custom {
                self.tree_custom_instructions().await?
            } else {
                None
            };
            if custom && custom_instructions.is_none() {
                continue;
            }
            return Ok(Some(NavigateTreeOptions {
                summarize: Some(choice != "No summary"),
                custom_instructions,
                ..NavigateTreeOptions::default()
            }));
        }
    }

    /// Asks for instructions, preserving cancellation separately from accepted empty text.
    async fn tree_custom_instructions(&self) -> Result<Option<String>, Box<dyn Error>> {
        self.show_extension_editor("Custom summarization instructions".to_owned(), None)
            .await
            .map_err(|error| -> Box<dyn Error> { error })
    }

    /// Moves to the selection with a cancellable loader while a summary runs, then restores
    /// the interrupt callback whatever the outcome.
    async fn navigate_selected_tree(self: &Rc<Self>, id: String) -> Result<(), Box<dyn Error>> {
        let Some(options) = self.tree_summary_options().await? else {
            return self.show_tree_selector(Some(id));
        };
        let original_escape = self.editor.on_escape();
        let summarizing = options.summarize == Some(true);
        if summarizing {
            self.show_summary_indicator();
        }
        let navigated = self
            .runtime_host
            .session()
            .navigate_tree(&id, options)
            .await;
        let reported = self.tree_navigated(id, navigated);
        if summarizing {
            self.clear_status_indicator(Some(StatusKind::BranchSummary));
        }
        self.editor.set_on_escape(original_escape);
        reported
    }

    /// Makes the interrupt key abort the branch summary and shows its status indicator.
    fn show_summary_indicator(&self) {
        let host = Rc::clone(&self.runtime_host);
        self.editor
            .set_on_escape(Some(Rc::new(move || host.session().abort_branch_summary())));
        self.chat_container.add_child(Rc::new(Spacer::new(1)));
        let label = format!(
            "Summarizing branch... ({} to cancel)",
            maestro_app::key_text("app.interrupt")
        );
        let loader = self.progress_loader(Some(label), ThemeColor::Accent);
        self.show_status_indicator(StatusKind::BranchSummary, loader, None);
        self.ui.request_render(false);
    }

    /// Reports the navigation's distinct outcomes.
    fn tree_navigated(
        self: &Rc<Self>,
        id: String,
        navigated: Result<NavigateTreeResult, DiagnosticErrorInfo>,
    ) -> Result<(), Box<dyn Error>> {
        match navigated {
            Ok(result) if result.aborted => {
                self.show_status("Branch summarization cancelled")?;
                self.show_tree_selector(Some(id))?;
            }
            Ok(result) if result.cancelled => self.show_status("Navigation cancelled")?,
            Ok(result) => self
                .present_tree_navigation(result)
                .map_err(|error| -> Box<dyn Error> { error })?,
            Err(error) => self.show_error(&error.message)?,
        }
        Ok(())
    }

    /// Replays navigation, fills only a still-blank draft and starts captured input delivery.
    ///
    /// # Errors
    /// Returns the original replay, editor or theme failure.
    pub(in crate::interactive_mode) fn present_tree_navigation(
        self: &Rc<Self>,
        result: NavigateTreeResult,
    ) -> Result<(), EventError> {
        self.chat_container.clear();
        self.render_initial_messages()?;
        if let Some(text) = result.editor_text.filter(|text| !text.is_empty())
            && self
                .current_editor()
                .try_get_text()?
                .trim_matches(is_whitespace_scalar)
                .is_empty()
        {
            self.current_editor().try_set_text(&text)?;
        }
        self.show_status("Navigated to selected point")?;
        self.flush_compaction_queue(false)?;
        Ok(())
    }
}
