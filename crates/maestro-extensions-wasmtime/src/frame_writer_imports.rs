//! Canonical frame writer imports: the writer widget of a lent terminal and its requests.
use crate::{
    callback_imports::{CallbackView, retain},
    frame_writer::WireRequest,
    guest_calls::Callback,
    import_route::push,
    local_resources::{Message, effects::TuiEndpoint},
    widget_reads::{Request, WidgetEndpoint, WidgetMessage},
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};

/// Installs the writer widget constructor, its requests, input listeners and input draining.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]tui.writer",
        |mut store: StoreContextMut<'_, T>, (this,): (Resource<TuiEndpoint>,)| {
            let store = store.data_mut();
            let tui = store.ctx().table.get(&this)?.clone();
            let writer = store
                .api()
                .0
                .request(|api, reply| Message::Widgets(WidgetMessage::Writer(api, tui, reply)));
            Ok((match writer {
                Ok(writer) => Ok(store.ctx().table.push(writer)?),
                Err(error) => Err(error),
            },))
        },
    )?;
    host.func_wrap(
        "[method]widget.writer",
        |mut store: StoreContextMut<'_, T>,
         (this, request): (Resource<WidgetEndpoint>, WireRequest)| {
            let store = store.data_mut();
            let request = request.retained(store)?;
            let widget = store.ctx().table.get(&this)?.clone();
            let route = widget.route(|reply| Request::Writer(request, reply));
            Ok((push(store, route)?,))
        },
    )?;
    host.func_wrap(
        "[method]widget.add-input-listener",
        |mut store: StoreContextMut<'_, T>,
         (this, listener): (Resource<WidgetEndpoint>, Resource<Callback>)| {
            let store = store.data_mut();
            let listener = retain(store, &listener)?;
            let widget = store.ctx().table.get(&this)?.clone();
            let added = widget.read(|reply| Request::AddInputListener(listener, reply));
            Ok((match added {
                Ok(subscription) => Ok(store.ctx().table.push(subscription)?),
                Err(error) => Err(error),
            },))
        },
    )?;
    host.func_wrap_concurrent(
        "[method]widget.drain-input",
        |accessor, (this, max, idle): (Resource<WidgetEndpoint>, Option<f64>, Option<f64>)| {
            Box::pin(async move {
                let widget =
                    accessor.with(|mut access| access.get().ctx().table.get(&this).cloned())?;
                let drained = widget
                    .0
                    .request_async(|id, reply, admission| {
                        Message::Widgets(WidgetMessage::Drain(id, (max, idle), reply, admission))
                    })
                    .await;
                Ok((drained,))
            })
        },
    )
}
