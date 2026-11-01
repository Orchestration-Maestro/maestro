//! The structural application capability of a replacement editor and the result of an
//! editor factory.
use indexmap::IndexMap;
use maestro_models::Cancellation;
use maestro_tui::EditorComponent;
use std::{cell::RefCell, ops::Deref, rc::Rc};

/// Shared insertion-ordered application action handlers, by action name.
pub type ActionHandlers = Rc<RefCell<IndexMap<String, Rc<dyn Fn()>>>>;

/// A shortcut interceptor returning whether it consumed the input.
pub type ExtensionShortcutCallback = Rc<dyn Fn(&str) -> bool>;

/// An editor that also carries the application's action map and special callbacks, so the
/// host can share its own handlers with it.
pub trait CustomEditorComponent: EditorComponent<Signal = Cancellation> {
    /// An alias to the editor's live action map.
    fn action_handlers(&self) -> ActionHandlers;
    /// The interrupt callback, if any.
    fn on_escape(&self) -> Option<Rc<dyn Fn()>>;
    /// Replaces or removes the interrupt callback.
    fn set_on_escape(&self, callback: Option<Rc<dyn Fn()>>);
    /// The empty-editor exit callback, if any.
    fn on_ctrl_d(&self) -> Option<Rc<dyn Fn()>>;
    /// Replaces or removes the empty-editor exit callback.
    fn set_on_ctrl_d(&self, callback: Option<Rc<dyn Fn()>>);
    /// The image-paste callback, if any.
    fn on_paste_image(&self) -> Option<Rc<dyn Fn()>>;
    /// Replaces or removes the image-paste callback.
    fn set_on_paste_image(&self, callback: Option<Rc<dyn Fn()>>);
    /// The first input interceptor, if any.
    fn on_extension_shortcut(&self) -> Option<ExtensionShortcutCallback>;
    /// Replaces or removes the first input interceptor.
    fn set_on_extension_shortcut(&self, callback: Option<ExtensionShortcutCallback>);
}

/// The editor an extension's editor factory built: a plain editor or one with the
/// application capability.
#[derive(Clone)]
pub enum EditorFactoryOutcome {
    /// An editor with only the editor operations.
    Editor(Rc<dyn EditorComponent<Signal = Cancellation>>),
    /// An editor that also accepts the application's actions and callbacks.
    CustomEditor(Rc<dyn CustomEditorComponent>),
}

impl EditorFactoryOutcome {
    /// The built editor through its editor operations.
    #[must_use]
    pub fn editor(&self) -> Rc<dyn EditorComponent<Signal = Cancellation>> {
        match self {
            Self::Editor(editor) => Rc::clone(editor),
            Self::CustomEditor(editor) => Rc::clone(editor) as _,
        }
    }

    /// The application capability, when the editor has it.
    #[must_use]
    pub fn as_custom(&self) -> Option<&dyn CustomEditorComponent> {
        match self {
            Self::Editor(_) => None,
            Self::CustomEditor(editor) => Some(editor.as_ref()),
        }
    }
}

impl Deref for EditorFactoryOutcome {
    type Target = dyn EditorComponent<Signal = Cancellation>;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Editor(editor) => editor.as_ref(),
            Self::CustomEditor(editor) => editor.as_ref(),
        }
    }
}
