//! Native keyed widgets and footer status text.
use super::super::InteractiveMode;
use maestro_app::{
    EventError, ExtensionComponent, ExtensionWidgetOptions, WidgetContent, WidgetPlacement,
};
use maestro_theme::ThemeColor;
use maestro_tui::{Container, Spacer, Text};
use std::rc::Rc;

/// Maximum source lines rendered by each text widget.
const MAX_WIDGET_LINES: usize = 10;

impl InteractiveMode {
    /// Updates the live footer text and requests a frame, including on removal.
    pub(super) fn set_extension_status(&self, key: &str, text: Option<&str>) {
        self.footer_data_provider.set_extension_status(key, text);
        self.ui.request_render(false);
    }

    /// Disposes and removes either placement before building the replacement.
    ///
    /// # Errors
    /// Returns the original disposer, factory or text-theme failure, preserving prior effects.
    pub(super) fn set_extension_widget(
        &self,
        key: String,
        content: Option<WidgetContent>,
        options: Option<ExtensionWidgetOptions>,
    ) -> Result<(), EventError> {
        for map in [&self.extension_widgets_above, &self.extension_widgets_below] {
            if let Some(widget) = map.get(&key)
                && let Some(dispose) = &widget.dispose
            {
                dispose.call(())?;
            }
            drop(map.remove(&key));
        }
        let Some(content) = content else {
            self.render_widgets();
            return Ok(());
        };
        let component = match content {
            WidgetContent::Factory(factory) => {
                factory.call((self.ui.as_ref().clone(), self.themes.theme()))?
            }
            WidgetContent::Lines(lines) => {
                let container = Rc::new(Container::new());
                for line in lines.iter().take(MAX_WIDGET_LINES) {
                    container.add_child(Rc::new(Text::new(line.clone(), 1, 0, None)));
                }
                if lines.len() > MAX_WIDGET_LINES {
                    let text = self
                        .themes
                        .theme()
                        .get()?
                        .fg(&ThemeColor::Muted, "... (widget truncated)")?;
                    container.add_child(Rc::new(Text::new(text, 1, 0, None)));
                }
                ExtensionComponent {
                    component: container,
                    dispose: None,
                    set_expanded: None,
                    width: None,
                }
            }
        };
        let map = if options.and_then(|options| options.placement)
            == Some(WidgetPlacement::BelowEditor)
        {
            &self.extension_widgets_below
        } else {
            &self.extension_widgets_above
        };
        map.insert(key, Rc::new(component));
        self.render_widgets();
        Ok(())
    }

    /// Calls every widget's disposer, above then below, walking the live maps so a disposer's
    /// own edits are seen, then empties both maps and rebuilds the containers.
    ///
    /// # Errors
    /// Returns the original disposer failure, before either map is emptied.
    pub(in super::super) fn clear_extension_widgets(&self) -> Result<(), EventError> {
        for map in [&self.extension_widgets_above, &self.extension_widgets_below] {
            for (_, widget) in map {
                widget
                    .dispose
                    .as_ref()
                    .map_or(Ok(()), |dispose| dispose.call(()))?;
            }
        }
        self.extension_widgets_above.clear();
        self.extension_widgets_below.clear();
        self.render_widgets();
        Ok(())
    }

    /// Rebuilds the two containers from their live maps, retaining the above spacer.
    pub(in super::super) fn render_widgets(&self) {
        for (container, map, leading) in [
            (
                &self.widget_container_above,
                &self.extension_widgets_above,
                true,
            ),
            (
                &self.widget_container_below,
                &self.extension_widgets_below,
                false,
            ),
        ] {
            container.clear();
            if leading {
                container.add_child(Rc::new(Spacer::new(1)));
            }
            for (_, widget) in map {
                container.add_child(Rc::clone(&widget.component));
            }
        }
        self.ui.request_render(false);
    }
}
