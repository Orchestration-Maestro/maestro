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
    ToolRenderView::new(
        host,
        frame.i18n,
        frame.presentation,
        frame.highlighter,
        frame.modal,
    )
}
