//! A component made of other components.

use std::cell::{Ref, RefCell};
use std::rc::Rc;

use super::component::Component;

/// A component shared between a container and whoever keeps editing it.
pub type ComponentHandle = Rc<dyn Component>;

/// Renders its children one after another.
#[derive(Default)]
pub struct Container {
    /// Children in render order; the same child may appear more than once.
    children: RefCell<Vec<ComponentHandle>>,
}

impl Container {
    /// Creates a container without children.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The children in render order.
    #[must_use]
    pub fn children(&self) -> Ref<'_, [ComponentHandle]> {
        Ref::map(self.children.borrow(), Vec::as_slice)
    }

    /// Appends a child.
    pub fn add_child(&self, component: ComponentHandle) {
        self.children.borrow_mut().push(component);
    }

    /// Removes the first occurrence of `component`; a missing child is ignored.
    pub fn remove_child(&self, component: &ComponentHandle) {
        let removed = {
            let mut children = self.children.borrow_mut();
            let index = children
                .iter()
                .position(|child| Rc::ptr_eq(child, component));
            index.map(|index| children.remove(index))
        };
        drop(removed);
    }

    /// Removes every child.
    pub fn clear(&self) {
        let removed = std::mem::take(&mut *self.children.borrow_mut());
        drop(removed);
    }

    /// The children by position, each fetched when the walk reaches it: a child added
    /// during the walk is visited and one removed before its turn is not.
    fn live_children(&self) -> impl Iterator<Item = ComponentHandle> + '_ {
        (0..).map_while(|index| self.children.borrow().get(index).cloned())
    }
}

impl Component for Container {
    /// Renders the children in order, walking the live list of children.
    fn render(&self, width: usize) -> Vec<String> {
        self.live_children()
            .flat_map(|child| child.render(width))
            .collect()
    }

    /// Invalidates the children in order, walking the live list of children. Each call
    /// reaches every child before it returns, also one that is rendering or handling input.
    fn invalidate(&self) {
        for child in self.live_children() {
            child.invalidate();
        }
    }
}
