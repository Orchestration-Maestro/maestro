//! Tool call and result renderers the host runs for one row, with the row's guest state.
use std::rc::Rc;

use super::renderers::with_emitter;
use super::session::decode;
use super::{Exports, Imports, callbacks};
use crate::bindings::maestro::extension::host::{ToolRender, ToolRenderSlot};
use crate::types::ToolRow;
use crate::{
    Component, ExtensionResult, ToolRenderCall, ToolRenderContext, ToolRenderPhase,
    ToolRenderResult, ToolRenderResultOptions,
};

/// Keeps a new row's state under a fresh identity, as a component transfers it, and returns
/// that identity.
#[cfg(test)]
pub(crate) fn keep_row<I: Imports>(imports: &I, row: Rc<ToolRow>) -> u32 {
    let identity = callbacks::Identity::new(imports);
    let id = imports.callback_id(&identity.handle);
    identity.keep(row);
    id
}

impl<I: Imports> Exports<I> {
    /// Runs the renderer under this identity for one row with the lent theme and redraw
    /// capability, returning the component or error with any new row state.
    pub(crate) fn invoke_tool_render(
        &self,
        handler: u32,
        render: &ToolRender,
        (theme, invalidator, native_row): (I::Theme, I::Invalidator, Option<crate::NativeRow>),
        emitter: &I::Emitter,
    ) -> ExtensionResult<(ExtensionResult<Component>, Option<Rc<ToolRow>>)> {
        let (row, created) = match render.row {
            Some(row) => (callbacks::find::<Rc<ToolRow>>(row)?, false),
            None => (Rc::new(ToolRow::default()), true),
        };
        let slot = usize::from(matches!(render.slot, ToolRenderSlot::Result));
        let mut context = self.context(render, &row, slot, invalidator)?;
        context.native_row = native_row;
        let theme = super::theme::theme(self.imports.clone(), theme);
        let component = with_emitter(&self.imports, emitter, |emitter| {
            if slot == 0 {
                let renderer = callbacks::find::<ToolRenderCall>(handler)?;
                let args = render.args.as_deref().map(decode).transpose()?;
                return renderer(args, theme, &context, emitter);
            }
            let renderer = callbacks::find::<ToolRenderResult>(handler)?;
            let result = decode(render.result.as_deref().unwrap_or("null"))?;
            let options = render.options.as_ref().ok_or("missing result options")?;
            let options = ToolRenderResultOptions {
                expanded: options.expanded,
                is_partial: options.is_partial,
            };
            renderer(result, options, theme, &context, emitter)
        });
        row.last.borrow_mut()[slot] = component.as_ref().ok().cloned();
        Ok((component, created.then_some(row)))
    }

    /// What the renderer reads about its row at this rendering.
    fn context(
        &self,
        render: &ToolRender,
        row: &Rc<ToolRow>,
        slot: usize,
        invalidator: I::Invalidator,
    ) -> ExtensionResult<ToolRenderContext> {
        let info = &render.info;
        let (imports, invalidator) = (self.imports.clone(), Rc::new(invalidator));
        Ok(ToolRenderContext {
            args: info.args.as_deref().map(decode).transpose()?,
            tool_call_id: info.tool_call_id.clone(),
            cwd: info.cwd.clone(),
            phase: ToolRenderPhase {
                execution_started: info.phase.execution_started,
                args_complete: info.phase.args_complete,
                is_partial: info.phase.is_partial,
            },
            expanded: info.expanded,
            show_images: info.show_images,
            is_error: info.is_error,
            last_component: row.last.borrow()[slot].clone(),
            invalidate: Rc::new(move || imports.invalidate(&invalidator)),
            row: Rc::clone(row),
            native_row: None,
        })
    }
}
