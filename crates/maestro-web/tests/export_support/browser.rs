//! Export mounting through the existing native reading transaction harness.
pub use tests::*;
#[cfg(test)]
mod tests {
    use maestro_web::{
        browser::attachment_content::{ReadingContent, ReadingRoot},
        export_html::template::bind_expandable_output,
    };
    use std::rc::Rc;
    use web_sys::Element;

    /// Install an owned export factory and publish its complete mounted parent.
    pub fn mount(slots: &mut crate::reading_slots_support::Slots, html: String) -> Element {
        let target = Rc::clone(&slots.root);
        slots.content = Rc::new(ReadingContent {
            mount: Rc::new(move |document| {
                let root = document.create_element("section").unwrap();
                root.set_class_name("maestro-export");
                root.set_attribute("style", "color:var(--maestro-color-text);background:var(--maestro-color-background);font-family:var(--maestro-type-mono-font);font-size:var(--maestro-type-mono-size);line-height:var(--maestro-type-mono-line-height)").unwrap();
                root.set_inner_html(&html);
                let owner = bind_expandable_output(&root)?;
                *target.borrow_mut() = Some(ReadingRoot {
                    element: root.clone(),
                    slots: Vec::new(),
                    owner: None,
                    commit: None,
                });
                Ok(ReadingRoot {
                    element: root,
                    slots: Vec::new(),
                    owner,
                    commit: None,
                })
            }),
        });
        let layout = slots.publish(egui::vec2(600., 500.), &[]);
        assert!(layout.height > 0.);
        slots.root()
    }
}
