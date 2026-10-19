//! Projection of shared browser frame operands into retained tool/message content.
use maestro_web::{
    TranslationFunction, browser::attachment_content::AttachmentHost,
    components::code_block::CodeBlockView, tools::types::ToolRenderView,
};
use std::rc::Rc;

/// Borrow the current frame without retaining its presentation or platform reference.
pub fn view<'a>(
    host: &'a Rc<dyn AttachmentHost>,
    frame: CodeBlockView<'a, TranslationFunction>,
) -> ToolRenderView<'a> {
    ToolRenderView {
        host,
        i18n: frame.i18n,
        presentation: frame.presentation,
        highlighter: frame.highlighter,
        modal: frame.modal,
    }
}
