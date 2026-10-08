//! A component made of other components.

use std::cell::RefCell;
use std::rc::Rc;

use super::component::Component;

/// A component shared between a container and whoever keeps editing it.
pub type ComponentHandle = Rc<dyn Component>;

/// The array of children a container currently holds; a walk keeps the array it started on.
type ChildArray = Rc<RefCell<Vec<ComponentHandle>>>;

/// Renders its children one after another.
#[derive(Default)]
pub struct Container {
    /// Children in render order, the same child possibly more than once. Adding and
    /// removing edit this array in place; `clear` replaces it with a new empty one.
    children: RefCell<ChildArray>,
}

impl Container {
    /// Creates a container without children.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A copy of the children in render order.
    #[must_use]
    pub fn children(&self) -> Vec<ComponentHandle> {
        self.array().borrow().clone()
    }

    /// Appends a child.
    pub fn add_child(&self, component: ComponentHandle) {
        self.array().borrow_mut().push(component);
    }

    /// Removes the first occurrence of `component`; a missing child is ignored.
    pub fn remove_child(&self, component: &ComponentHandle) {
        let array = self.array();
        let removed = {
            let mut children = array.borrow_mut();
            let index = children
                .iter()
                .position(|child| Rc::ptr_eq(child, component));
            index.map(|index| children.remove(index))
        };
        drop(removed);
    }

    /// Replaces the children with a new empty array. A walk already running keeps the
    /// array it started on.
    pub fn clear(&self) {
        let replaced = self.children.replace(ChildArray::default());
        drop(replaced);
    }

    /// The array of children the container holds now.
    fn array(&self) -> ChildArray {
        Rc::clone(&self.children.borrow())
    }

    /// The children by position in the array held when the walk starts, each fetched when
    /// the walk reaches it, so the walk ends at the length that array has by then. A child
    /// added to the array meanwhile is visited and one removed ahead of the walk is not; a
    /// removal at or before the walk's position shifts the later children back, so the
    /// next one is skipped. `clear` installs a new array that this walk never sees.
    fn walk(&self) -> impl Iterator<Item = ComponentHandle> + use<> {
        let array = self.array();
        (0..).map_while(move |index| array.borrow().get(index).cloned())
    }
}

impl Component for Container {
    /// Renders the children in order, walking the array held when the render starts.
    fn render(&self, width: usize) -> Vec<String> {
        self.walk().flat_map(|child| child.render(width)).collect()
    }

    /// Invalidates the children in order, walking the array held when the call starts as
    /// `render` does. Each child the walk reaches is invalidated before the call returns,
    /// also one that is rendering or handling input, once per position the walk reaches.
    fn invalidate(&self) {
        for child in self.walk() {
            child.invalidate();
        }
    }
}
