//! Active named definitions on the conversion call path.

use serde_json::{Map, Value};

use super::schema_view::{Document, View};

/// The definitions visible at the point conversion currently reaches.
///
/// A definition is visible to the nodes converted inside the step that entered it, so a
/// sibling converted afterwards reads the enclosing definitions again. The context owns no
/// schema data of its own: it lends the document's nodes and the frames on the call path.
pub(crate) struct Context<'a> {
    /// Indexed authored nodes the context reads definitions from.
    pub(crate) document: &'a Document<'a>,
    /// Invocation-local callbacks indexed against the original schema.
    pub(crate) code: &'a super::super::check::Code<'a>,
    /// Definition maps active during this conversion.
    frames: Vec<&'a Map<String, Value>>,
}

impl<'a> Context<'a> {
    /// Start converting the supplied document with no definition entered yet.
    pub(crate) fn new(document: &'a Document<'a>, code: &'a super::super::check::Code<'a>) -> Self {
        Self {
            document,
            code,
            frames: Vec::new(),
        }
    }

    /// The authored node the innermost visible definition with this name declares.
    pub(crate) fn definition(&self, name: &str) -> Option<View<'a>> {
        super::super::references::definition(&self.frames, name).map(|raw| self.document.view(raw))
    }

    /// The definition maps entered by the conversions currently running.
    pub(crate) fn frames(&self) -> &[&'a Map<String, Value>] {
        &self.frames
    }

    /// Run one conversion step with the definitions a cyclic node declares visible.
    ///
    /// The declarations overlay the current frames for that step alone, so a sibling converted
    /// afterwards sees the enclosing frames again.
    pub(crate) fn within<R>(&mut self, view: View<'a>, step: impl FnOnce(&mut Self) -> R) -> R {
        let depth = self.frames.len();
        self.frames
            .extend(view.raw.get("$defs").and_then(Value::as_object));
        let result = step(self);
        self.frames.truncate(depth);
        result
    }
}
