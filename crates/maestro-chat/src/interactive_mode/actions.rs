//! Editor actions: the shell-mode border, external editing and image paste.
use super::InteractiveMode;
use crate::external_editor::{ExternalEditorFile, edit_external_file, external_command};
use crate::temporary_file::write_temporary_file;
use maestro_app::is_whitespace_scalar;
use maestro_app::utils::clipboard_image::{
    ClipboardImage, extension_for_image_mime_type, read_clipboard_image,
};
use std::{error::Error, io, rc::Rc};

impl InteractiveMode {
    /// Installs the editor's change callback and image paste without keeping the mode
    /// alive: text whose first non-whitespace character is `!` selects the shell border,
    /// and only a change of mode repaints it.
    pub(super) fn setup_editor_callbacks(self: &Rc<Self>) {
        let mode = Rc::downgrade(self);
        self.editor.set_on_change(Some(Rc::new(move |text| {
            let Some(mode) = mode.upgrade() else {
                return;
            };
            let shell = text
                .trim_start_matches(is_whitespace_scalar)
                .starts_with('!');
            if mode.is_bash_mode.replace(shell) != shell {
                mode.report(mode.update_editor_border_color().map_err(Into::into));
            }
        })));
        let mode = Rc::downgrade(self);
        self.editor.set_on_paste_image(Some(Rc::new(move || {
            if let Some(mode) = mode.upgrade() {
                mode.handle_clipboard_image_paste();
            }
        })));
    }

    /// Edits the expanded editor text in the configured external editor, or warns when
    /// neither `VISUAL` nor `EDITOR` names one.
    ///
    /// # Errors
    /// Returns the warning's theme failure or the edit's file, terminal or process failure.
    pub(super) fn open_external_editor(&self) -> Result<(), Box<dyn Error>> {
        let Some(command) = external_command(self.tool_environment.runtime.as_ref()) else {
            return self.show_warning(
                "No editor configured. Set $VISUAL or $EDITOR environment variable.",
            );
        };
        let path = std::env::temp_dir().join(format!(
            "maestro-editor-{}.maestro.md",
            self.tool_environment.runtime.log_context().unix_ms
        ));
        let file = ExternalEditorFile {
            command: &command,
            path: &path,
            text: &self.editor.get_expanded_text(),
        };
        edit_external_file(
            &self.ui,
            #[cfg(windows)]
            self.tool_environment.runtime.as_ref(),
            &self.editor,
            &file,
        )?;
        Ok(())
    }

    /// Reads the clipboard image on the host, keeping the mode until the read completes,
    /// then pastes it.
    fn handle_clipboard_image_paste(self: Rc<Self>) {
        let host = Rc::clone(&self.tool_environment.runtime);
        host.spawn_local(Box::pin(async move {
            self.paste_clipboard_result(read_clipboard_image(None).await);
            Ok(())
        }));
    }

    /// Writes a read clipboard image to a new private temporary file and inserts its path
    /// at the cursor; no image and every failure leave the editor untouched.
    pub(super) fn paste_clipboard_result(&self, result: io::Result<Option<ClipboardImage>>) {
        let Ok(Some(image)) = result else {
            return;
        };
        let extension = extension_for_image_mime_type(&image.mime_type).unwrap_or("png");
        let path = std::env::temp_dir().join(format!(
            "maestro-clipboard-{}.{extension}",
            uuid::Uuid::new_v4()
        ));
        if write_temporary_file(&path, &image.bytes).is_ok() {
            self.editor.insert_text_at_cursor(&path.to_string_lossy());
            self.ui.request_render(false);
        }
    }

    /// Hands a failed editor callback's error to the terminal runtime, which reports it.
    pub(super) fn report(&self, result: Result<(), Box<dyn Error>>) {
        if let Err(error) = result {
            self.tool_environment
                .runtime
                .spawn_local(Box::pin(std::future::ready(Err(error))));
        }
    }
}
