//! Extension headers and footers in place of the built-in ones.
use super::super::InteractiveMode;
use maestro_app::{ComponentFactory, EventError, FooterFactory};
use maestro_tui::tui::ComponentHandle;
use std::rc::Rc;

impl InteractiveMode {
    /// Disposes and removes the current footer, then appends the extension's footer over the
    /// live footer metadata, or the built-in footer for `None`, and requests a frame.
    ///
    /// # Errors
    /// Returns the original disposer or factory failure, keeping the effects before it.
    pub(super) fn set_extension_footer(
        &self,
        factory: Option<FooterFactory>,
    ) -> Result<(), EventError> {
        let custom = self.custom_footer.borrow().clone();
        if let Some(dispose) = custom.as_ref().and_then(|footer| footer.dispose.as_ref()) {
            dispose.call(())?;
        }
        let current: ComponentHandle = match custom {
            Some(footer) => Rc::clone(&footer.component),
            None => self.footer.clone(),
        };
        self.ui.remove_child(&current);
        if let Some(factory) = factory {
            let data = Rc::new(self.footer_data_provider.clone());
            let footer = factory.call((self.ui.as_ref().clone(), self.themes.theme(), data))?;
            self.ui.add_child(Rc::clone(&footer.component));
            self.custom_footer.replace(Some(Rc::new(footer)));
        } else {
            self.custom_footer.replace(None);
            self.ui.add_child(self.footer.clone());
        }
        self.ui.request_render(false);
        Ok(())
    }

    /// Disposes the current extension header, then puts the extension's header, or the
    /// built-in one for `None`, at the active header's position with the current expansion,
    /// and requests a frame. Before the built-in header exists nothing happens; a new header
    /// with no active position goes first.
    ///
    /// # Errors
    /// Returns the original disposer, factory or expansion failure, keeping the effects
    /// before it.
    pub(super) fn set_extension_header(
        &self,
        factory: Option<ComponentFactory>,
    ) -> Result<(), EventError> {
        let Some(built_in) = self.built_in_header.borrow().clone() else {
            return Ok(());
        };
        let custom = self.custom_header.borrow().clone();
        if let Some(dispose) = custom.as_ref().and_then(|header| header.dispose.as_ref()) {
            dispose.call(())?;
        }
        let current = custom.map_or_else(|| Rc::clone(&built_in), |h| Rc::clone(&h.component));
        let children = self.header_container.children();
        let index = children
            .borrow()
            .iter()
            .position(|child| std::ptr::addr_eq(Rc::as_ptr(child), Rc::as_ptr(&current)));
        let expanded = self.tool_output_expanded.get();
        let header = if let Some(factory) = factory {
            let header = Rc::new(factory.call((self.ui.as_ref().clone(), self.themes.theme()))?);
            self.custom_header.replace(Some(Rc::clone(&header)));
            if let Some(set_expanded) = &header.set_expanded {
                set_expanded.call(expanded)?;
            }
            if index.is_none() {
                children
                    .borrow_mut()
                    .insert(0, Rc::clone(&header.component));
            }
            Rc::clone(&header.component)
        } else {
            self.custom_header.replace(None);
            super::super::toggles::set_expanded(built_in.as_ref(), expanded)?;
            built_in
        };
        if let Some(index) = index {
            children.borrow_mut()[index] = header;
        }
        self.ui.request_render(false);
        Ok(())
    }
}
