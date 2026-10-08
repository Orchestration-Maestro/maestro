//! A component made of other components.

use std::cell::RefCell;
use std::rc::Rc;

use super::component::Component;

/// A component shared between a container and whoever keeps editing it.
pub type ComponentHandle = Rc<dyn Component>;

/// The array of children a container holds. Whoever keeps a handle edits the array the
/// container renders, until `clear` or `set_children` gives the container another one; a
/// walk keeps the array it started on. Borrow it only for the statement that edits or reads
/// it, because the container borrows it itself to render, invalidate, add and remove.
pub type ChildArray = Rc<RefCell<Vec<ComponentHandle>>>;

/// Renders its children one after another.
#[derive(Default)]
pub struct Container {
    /// Children in render order, the same child possibly more than once. Adding and
    /// removing edit this array in place; `clear` replaces it with a new empty one.
    array: RefCell<ChildArray>,
}

impl Container {
    /// Creates a container without children.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The array of children in render order, shared with the container.
    #[must_use]
    pub fn children(&self) -> ChildArray {
        Rc::clone(&self.array.borrow())
    }

    /// Makes `children` the array the container holds. The previous array is left to its
    /// holders, and a walk already running keeps it.
    pub fn set_children(&self, children: ChildArray) {
        let replaced = self.array.replace(children);
        drop(replaced);
    }

    /// Appends a child.
    pub fn add_child(&self, component: ComponentHandle) {
        self.children().borrow_mut().push(component);
    }

    /// Removes the first occurrence of `component`; a missing child is ignored.
    pub fn remove_child(&self, component: &ComponentHandle) {
        let array = self.children();
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
    /// array it started on, and a handle kept from `children` stays with the old array.
    pub fn clear(&self) {
        let replaced = self.array.replace(ChildArray::default());
        drop(replaced);
    }

    /// The children by position in the array held when the walk starts, each fetched when
    /// the walk reaches it, so the walk ends at the length that array has by then. A child
    /// added to that array is visited only if the walk reaches its position, and one
    /// removed ahead of the walk is not visited. A removal at or before the walk's position
    /// shifts the later children back, so the next one is skipped, and a child listed twice
    /// is visited once per position reached. `clear`, and `set_children` with another
    /// array, install an array that this walk never visits anything in.
    fn walk(&self) -> impl Iterator<Item = ComponentHandle> + use<> {
        let array = self.children();
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
    /// also one that is rendering or handling input, once per position the walk visits.
    fn invalidate(&self) {
        for child in self.walk() {
            child.invalidate();
        }
    }
}
