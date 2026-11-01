//! Authored widget callbacks the host calls, through the generated bindings.

use super::Generated;
use crate::bindings::maestro::extension::host;
use crate::component_adapter::{WidgetAnswer, WidgetEvent};
use crate::types::ExtensionResult;
use crate::{SelectItem, SelectListTruncatePrimaryContext};

/// A choice from its wire form.
pub(super) fn item(item: host::SelectItem) -> SelectItem {
    SelectItem {
        value: item.value,
        label: item.label,
        description: item.description,
    }
}

/// A widget event from its wire form.
fn event(event: host::WidgetEvent) -> WidgetEvent<Generated> {
    match event {
        host::WidgetEvent::Select(choice) => WidgetEvent::Select(item(choice)),
        host::WidgetEvent::Cancel => WidgetEvent::Cancel,
        host::WidgetEvent::SelectionChange(choice) => WidgetEvent::SelectionChange(item(choice)),
        host::WidgetEvent::Change((id, value)) => WidgetEvent::Change(id, value),
        host::WidgetEvent::Abort => WidgetEvent::Abort,
        host::WidgetEvent::Truncate(context) => {
            WidgetEvent::Truncate(SelectListTruncatePrimaryContext {
                text: context.text,
                max_width: context.max_width,
                column_width: context.column_width,
                item: item(context.item),
                is_selected: context.is_selected,
            })
        }
        host::WidgetEvent::Row((text, selected)) => WidgetEvent::Row(text, selected),
        host::WidgetEvent::Submenu((value, done)) => WidgetEvent::Submenu(value, done),
        host::WidgetEvent::Text(text) => WidgetEvent::Text(text),
        host::WidgetEvent::Notice => WidgetEvent::Notice,
        host::WidgetEvent::Shortcut(data) => WidgetEvent::Shortcut(data),
        host::WidgetEvent::Visible((columns, rows)) => WidgetEvent::Visible(columns, rows),
        host::WidgetEvent::OverlayHandle(handle) => WidgetEvent::OverlayHandle(handle),
        host::WidgetEvent::OverlayOptions => WidgetEvent::OverlayOptions,
    }
}

/// Runs an authored widget callback and transfers what it answered.
pub(super) fn invoke_widget(
    handler: u32,
    wire: host::WidgetEvent,
    emitter: &host::CallbackEmitter,
) -> ExtensionResult<host::WidgetAnswer> {
    let answer =
        crate::component_adapter::invoke_widget(&Generated, handler, event(wire), emitter)?;
    Ok(match answer {
        WidgetAnswer::Done => host::WidgetAnswer::Done,
        WidgetAnswer::Text(text) => host::WidgetAnswer::Text(text),
        WidgetAnswer::Consumed(consumed) => host::WidgetAnswer::Consumed(consumed),
        WidgetAnswer::Visible(visible) => host::WidgetAnswer::Visible(visible),
        WidgetAnswer::OverlayOptions(options) => host::WidgetAnswer::OverlayOptions(
            crate::component_adapter::overlays::share(&Generated, &options)?,
        ),
        WidgetAnswer::Component(component) => host::WidgetAnswer::Component(
            crate::component_adapter::callbacks::Identity::<Generated>::new(&Generated)
                .transfer(component),
        ),
    })
}
