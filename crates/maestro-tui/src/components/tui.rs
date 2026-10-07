//! Synchronous component composition.
pub use crate::text::utils::visible_width;
use std::{cell::RefCell, rc::Rc};
/// A terminal presentation component.
pub trait Component {
    /// Render at the supplied cell width.
    fn render(&mut self, width: usize) -> Vec<String>;
    /// Handle raw input when supported.
    fn handle_input(&mut self, _data: &str) {}
    /// Whether key release events are wanted.
    fn wants_key_release(&self) -> bool {
        false
    }
    /// Invalidate retained rendering state.
    fn invalidate(&mut self);
    /// Discover an optional focused member.
    fn focusable(&self) -> Option<&dyn Focusable> {
        None
    }
    /// Mutably discover an optional focused member.
    fn focusable_mut(&mut self) -> Option<&mut dyn Focusable> {
        None
    }
}
/// A component with a focused member.
pub trait Focusable {
    /// Read focus state.
    fn focused(&self) -> bool;
    /// Write focus state.
    fn set_focused(&mut self, focused: bool);
}
/// Detect focus capability, regardless of current focus state.
#[must_use]
pub fn is_focusable(component: Option<&dyn Component>) -> bool {
    component.is_some_and(|c| c.focusable().is_some())
}
/// Zero-cell cursor positioning marker.
pub const CURSOR_MARKER: &str = "\x1b_maestro:c\x07";
/// Groups shared child components.
#[derive(Default)]
pub struct Container {
    /// Children in rendering order.
    pub children: Vec<Rc<RefCell<dyn Component>>>,
}
impl Container {
    /// Construct an empty group.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Append a shared child.
    pub fn add_child(&mut self, component: Rc<RefCell<dyn Component>>) {
        self.children.push(component);
    }
    /// Remove the first matching identity, if present.
    pub fn remove_child(&mut self, component: &Rc<RefCell<dyn Component>>) {
        if let Some(index) = self.children.iter().position(|c| Rc::ptr_eq(c, component)) {
            self.children.remove(index);
        }
    }
    /// Replace the children with an empty list.
    pub fn clear(&mut self) {
        self.children = Vec::new();
    }
}
impl Component for Container {
    fn render(&mut self, width: usize) -> Vec<String> {
        self.children
            .iter()
            .flat_map(|c| c.borrow_mut().render(width))
            .collect()
    }
    fn invalidate(&mut self) {
        for child in &self.children {
            child.borrow_mut().invalidate();
        }
    }
}
/// Placement anchor.
pub enum OverlayAnchor {
    /// Center.
    Center,
    /// `TopLeft`.
    TopLeft,
    /// `TopRight`.
    TopRight,
    /// `BottomLeft`.
    BottomLeft,
    /// `BottomRight`.
    BottomRight,
    /// `TopCenter`.
    TopCenter,
    /// `BottomCenter`.
    BottomCenter,
    /// `LeftCenter`.
    LeftCenter,
    /// `RightCenter`.
    RightCenter,
}
/// Individually optional side margins.
pub struct OverlayMargin {
    /// Optional top margin.
    pub top: Option<usize>,
    /// Optional right margin.
    pub right: Option<usize>,
    /// Optional bottom margin.
    pub bottom: Option<usize>,
    /// Optional left margin.
    pub left: Option<usize>,
}
/// Numeric cells or unparsed percentage text.
pub enum SizeValue {
    /// Numeric cells.
    Number(usize),
    /// Unparsed percentage text.
    Percent(String),
}
/// Uniform or per-side margins.
pub enum OverlayMarginValue {
    /// Numeric cells.
    Number(usize),
    /// Individually optional side margins.
    Sides(OverlayMargin),
}
/// Optional sizing, positioning and visibility data; absence is retained.
pub struct OverlayOptions {
    /// Optional width.
    pub width: Option<SizeValue>,
    /// Optional minimum width.
    pub min_width: Option<usize>,
    /// Optional maximum height.
    pub max_height: Option<SizeValue>,
    /// Optional placement anchor.
    pub anchor: Option<OverlayAnchor>,
    /// Optional signed horizontal offset.
    pub offset_x: Option<isize>,
    /// Optional signed vertical offset.
    pub offset_y: Option<isize>,
    /// Optional row.
    pub row: Option<SizeValue>,
    /// Optional column.
    pub col: Option<SizeValue>,
    /// Optional uniform or per-side margin.
    pub margin: Option<OverlayMarginValue>,
    /// Optional visibility predicate of terminal columns and rows.
    pub visible: Option<Box<dyn Fn(usize, usize) -> bool>>,
    /// Optional non-capturing state.
    pub non_capturing: Option<bool>,
}
/// Caller-supplied visibility and focus operations.
pub trait OverlayHandle {
    /// Hide.
    fn hide(&mut self);
    /// Set hidden.
    fn set_hidden(&mut self, hidden: bool);
    /// Is hidden.
    fn is_hidden(&self) -> bool;
    /// Focus.
    fn focus(&mut self);
    /// Unfocus.
    fn unfocus(&mut self);
    /// Is focused.
    fn is_focused(&self) -> bool;
}
