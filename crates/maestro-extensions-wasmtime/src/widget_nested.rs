//! Host widgets inside other components, whose rendering returns their authored functions'
//! failures.
use crate::guest_calls::paints::Failure;
use crate::widget_reads::Widget;
use maestro_extensions::widgets::Focusable;
use maestro_extensions::{Component, ComponentHandle, InputHandler};
use std::rc::Rc;

/// A host widget inside another component: rendering returns the failure its authored
/// functions reported, as the widget's own rendering does.
struct Nested {
    /// The toolkit widget.
    component: ComponentHandle,
    /// The widget's failure.
    failure: Failure,
}

impl Component for Nested {
    fn render(&self, width: usize) -> Vec<String> {
        self.component.render(width)
    }
    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let rows = self.component.try_render(width);
        let failed = self.failure.borrow_mut().take();
        failed.map_or(rows, |error| Err(error.into()))
    }
    fn invalidate(&self) {
        self.component.invalidate();
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        self.component.input_handler()
    }
    fn wants_key_release(&self) -> bool {
        self.component.wants_key_release()
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        self.component.focusable()
    }
}

impl Widget {
    /// The widget as another component's child.
    pub(crate) fn nested(&self) -> ComponentHandle {
        let nested = || -> ComponentHandle {
            Rc::new(Nested {
                component: self.component(),
                failure: self.failure.clone(),
            })
        };
        self.nested.get_or_init(nested).clone()
    }
}
