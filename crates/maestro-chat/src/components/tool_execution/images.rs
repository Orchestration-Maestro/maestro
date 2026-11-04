//! Inline images of a tool row: their conversion for Kitty and their retained children.
use super::{Row, ToolExecutionResult};
use maestro_app::{DiagnosticErrorInfo, UserBlock, convert_to_png};
use maestro_theme::ThemeColor;
use maestro_tui::{Image, ImageOptions, ImageProtocol, ImageTheme, Spacer, tui::ComponentHandle};
use std::rc::Rc;

impl Row {
    /// Converts current non-PNG payloads once per successful authored identity.
    pub(super) fn convert_images(
        &self,
        result: &ToolExecutionResult,
    ) -> Result<(), DiagnosticErrorInfo> {
        if self
            .params
            .environment
            .terminal_image
            .get_capabilities()
            .images
            != Some(ImageProtocol::Kitty)
        {
            return Ok(());
        }
        for block in &result.content {
            let UserBlock::Image(image) = block else {
                continue;
            };
            if image.data.is_empty() || image.mime_type.is_empty() || image.mime_type == "image/png"
            {
                continue;
            }
            if self
                .converted
                .borrow()
                .get(&image.data)
                .is_some_and(|formats| formats.contains_key(&image.mime_type))
            {
                continue;
            }
            if let Some(converted) = convert_to_png(&image.data, &image.mime_type) {
                self.converted
                    .borrow_mut()
                    .entry(image.data.clone())
                    .or_default()
                    .insert(image.mime_type.clone(), converted);
                self.rebuild()?;
                (self.params.request_render)();
            }
        }
        Ok(())
    }
    /// Replaces inline children without changing caller additions or outer aliases.
    pub(super) fn rebuild_images(&self) {
        let old = self.images.take();
        for child in old {
            self.container.remove_child(&child);
        }
        let terminal = &self.params.environment.terminal_image;
        let Some(protocol) = terminal.get_capabilities().images else {
            return;
        };
        if !self.show_images.get() {
            return;
        }
        let result = self.result.borrow().clone();
        let Some(result) = result else {
            return;
        };
        for block in &result.content {
            let UserBlock::Image(image) = block else {
                continue;
            };
            if image.data.is_empty() || image.mime_type.is_empty() {
                continue;
            }
            let Some(child) = self.image(image, protocol) else {
                continue;
            };
            let children: [ComponentHandle; 2] = [Rc::new(Spacer::new(1)), child];
            for child in children {
                self.container.add_child(child.clone());
                self.images.borrow_mut().push(child);
            }
        }
    }
    /// Builds an inline child after releasing the conversion cache borrow.
    fn image(
        &self,
        image: &maestro_app::ImageContent,
        protocol: ImageProtocol,
    ) -> Option<Rc<Image>> {
        let (data, mime) = {
            let converted = self.converted.borrow();
            let converted = converted
                .get(&image.data)
                .and_then(|formats| formats.get(&image.mime_type));
            let (data, mime) =
                converted.map_or((&image.data, &image.mime_type), |c| (&c.data, &c.mime_type));
            if protocol == ImageProtocol::Kitty && mime != "image/png" {
                return None;
            }
            (data.clone(), mime.clone())
        };
        let theme = self.params.environment.live_theme.clone();
        let child = Rc::new(Image::new(
            data,
            mime,
            ImageTheme {
                fallback_color: Box::new(move |text| {
                    maestro_theme::paint(theme.get().ok().as_deref(), &ThemeColor::ToolOutput, text)
                }),
            },
            ImageOptions {
                max_width_cells: Some(crate::whole_cell_width::whole_cell_width(
                    self.image_width.get(),
                )),
                ..Default::default()
            },
            self.params.environment.terminal_image.clone(),
        ));
        Some(child)
    }
}
