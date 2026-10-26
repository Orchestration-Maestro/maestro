//! Shared authored methods for artifact factories and source controls.
/// Retain the artifact's typed browser body through its module-owned mount.
macro_rules! artifact_reading {
    ($field:ident) => {
        /// Reuse the published factory, otherwise retain owned state only.
        fn reading(&self) -> std::rc::Rc<crate::browser::attachment_content::ReadingContent> {
            if let Some(reading) = &self.reading {
                return std::rc::Rc::clone(reading);
            }
            #[cfg(target_arch = "wasm32")]
            let body = std::rc::Rc::clone(&self.$field);
            std::rc::Rc::new(crate::browser::attachment_content::ReadingContent {
                #[cfg(target_arch = "wasm32")]
                mount: std::rc::Rc::new(move |document| browser::mount(document, &body)),
            })
        }
    };
}
/// Draw the preview choice before visibility reconciliation and source controls.
macro_rules! artifact_header_buttons {
    () => {
        fn get_header_buttons(
            &mut self,
            ui: &mut egui::Ui,
            view: ArtifactView<'_>,
        ) -> Result<(), ArtifactUiError> {
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.x =
                    crate::browser::attachment_content::points(view.presentation.spacing["sm"]);
                self.toggle
                    .render(ui, view.i18n, view.presentation)
                    .map_err(ArtifactUiError::localized_control)?;
                self.reconcile()?;
                ArtifactElement::get_header_buttons(&mut self.source, ui, view)
            })
            .inner
        }
    };
}
/// Publish retained visibility and update stored visibility only after the host accepts it.
macro_rules! artifact_visibility {
    ($($field:ident)?) => {
        fn set_visibility(
            &mut self,
            visible: bool,
            modal: Option<egui::Id>,
        ) -> Result<(), ArtifactUiError> {
            super::retained::host(self.host.as_ref())?
                .set_retained_visibility(self.id, visible, modal)?;
            $(self.$field = visible;)?
            Ok(())
        }
    };
}
pub(super) use {artifact_header_buttons, artifact_reading, artifact_visibility};

/// Require the effects of a retained caller, never constructing a surface for page content.
pub(super) fn host(
    host: Option<&std::rc::Rc<dyn crate::browser::attachment_content::AttachmentHost>>,
) -> Result<
    &std::rc::Rc<dyn crate::browser::attachment_content::AttachmentHost>,
    super::ArtifactUiError,
> {
    host.ok_or_else(|| {
        super::ArtifactUiError::Presentation("Artifact has no retained host".to_owned().into())
    })
}
