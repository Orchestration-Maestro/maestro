//! A component made of other components.

use std::cell::RefCell;
use std::rc::Rc;

use super::component::Component;

/// A component shared between a container and whoever keeps editing it.
pub type ComponentHandle = Rc<RefCell<dyn Component>>;

/// Renders its children one after another.
#[derive(Default)]
pub struct Container {
    /// Children in render order; the same child may appear more than once.
    pub children: Vec<ComponentHandle>,
}

impl Container {
    /// Creates a container without children.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a child.
    pub fn add_child(&mut self, component: ComponentHandle) {
        self.children.push(component);
    }

    /// Removes the first occurrence of `component`; a missing child is ignored.
    pub fn remove_child(&mut self, component: &ComponentHandle) {
        if let Some(index) = self
            .children
            .iter()
            .position(|child| Rc::ptr_eq(child, component))
        {
            self.children.remove(index);
        }
    }

    /// Removes every child.
    pub fn clear(&mut self) {
        self.children.clear();
    }
}

impl Component for Container {
    fn render(&mut self, width: usize) -> Vec<String> {
        self.children
            .iter()
            .flat_map(|child| child.borrow_mut().render(width))
            .collect()
    }

    fn invalidate(&mut self) {
        for child in &self.children {
            child.borrow_mut().invalidate();
        }
    }
}
