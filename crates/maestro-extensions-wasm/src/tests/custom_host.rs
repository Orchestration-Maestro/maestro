//! The controlled component host constructs custom views through their actual guest exports.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{
    bus_host::Emitter,
    host::{Identity, Imports, State},
};
use crate::bindings::host_side::{
    exports::maestro::extension::guest::Guest, maestro::extension::host,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use wasmtime::component::{Accessor, Resource};

/// Input and rendering expectations supplied to either custom-view adapter.
#[derive(Clone)]
pub struct Script {
    /// Width requested by the authored view.
    pub width: Option<f64>,
    /// Frames before input and after every input.
    pub frames: Vec<Vec<String>>,
    /// Terminal input delivered in order.
    pub inputs: Vec<String>,
    /// Expected recoverable errors keyed by input index; the view stays open afterward.
    pub input_errors: BTreeMap<usize, String>,
    /// Native waits to complete before each frame; empty leaves tails with the caller.
    pub ticks: Vec<usize>,
    /// Terminal columns supplied to every render.
    pub columns: u32,
}
impl Default for Script {
    fn default() -> Self {
        Self {
            ticks: Vec::new(),
            columns: 20,
            width: Some(13.0),
            frames: vec![vec!["custom 23".into()]; 2],
            inputs: vec!["commit".into()],
            input_errors: BTreeMap::new(),
        }
    }
}

/// Shared native close decision, independent of callback-table slot reuse.
#[derive(Clone, Default)]
pub struct Close(Arc<AtomicBool>);
impl Close {
    /// Accepts exactly the first close.
    pub fn close(&self) -> bool {
        !self.0.swap(true, Ordering::SeqCst)
    }
    /// Reports whether completion asked the host to close.
    pub fn is_closed(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
impl host::HostCustomClose for State {
    fn close(&mut self, this: Resource<Close>) -> wasmtime::Result<Resource<super::host::Route>> {
        let accepted = self.table.get(&this)?.close();
        self.push(super::host::Route {
            steps: std::collections::VecDeque::default(),
            running: None,
            result: Some(Ok(host::RouteValue::Boolean(accepted))),
            rows: None,
        })
    }
    fn drop(&mut self, this: Resource<Close>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

/// Shows a custom view until its retained input completes it, with explicit disposal.
pub(super) async fn run(
    accessor: &Accessor<State, Imports>,
    ui: Resource<super::ui_host::Ui>,
    factory: Resource<Identity>,
    options: Option<host::CustomUiOptions>,
) -> wasmtime::Result<Result<(), String>> {
    let admitted = accessor.with(|mut access| {
        let state = access.get();
        let key = state.table.get(&ui)?.0;
        Ok::<_, wasmtime::Error>(super::ui_host::call(
            &mut state.observed,
            key,
            "custom",
            &serde_json::json!([]),
        ))
    })?;
    if let Err(error) = admitted {
        return Ok(Err(error));
    }
    let close = Close::default();
    let (guest, input, emitter) = accessor.with(|mut access| {
        let state = access.get();
        let guest = state
            .guest
            .clone()
            .ok_or_else(|| wasmtime::format_err!("guest is not installed"))?;
        let input = host::CustomInput {
            tui: state.push(super::ui_host::Tui)?,
            theme: state.push(super::renderer_host::Theme("dark".into()))?,
            keybindings: state.push(super::widget_component::KeybindingsResource(0))?,
            close: state.push(close.clone())?,
        };
        let emitter = state.table.push(Emitter { key: 0 })?;
        Ok::<_, wasmtime::Error>((guest, input, emitter))
    })?;
    let result = async {
        if let Some(options) = options {
            assert!(!options.overlay.unwrap_or(false));
            assert!(options.overlay_options.is_none());
        }
        let built = factory_outcome(
            accessor,
            &guest,
            (factory, input, Resource::new_borrow(emitter.rep())),
        )
        .await?;
        match built {
            Ok(built) => view(accessor, &guest, built, Resource::new_borrow(emitter.rep())).await,
            Err(error) => Ok(Err(error)),
        }
    }
    .await;
    accessor.with(|mut access| access.get().table.delete(emitter))?;
    if matches!(&result, Ok(Ok(()))) {
        assert!(close.is_closed());
    }
    result
}

/// Runs the prefix now, then awaits only the separately retained future when needed.
async fn factory_outcome(
    accessor: &Accessor<State, Imports>,
    guest: &Guest,
    (factory, input, emitter): (Resource<Identity>, host::CustomInput, Resource<Emitter>),
) -> wasmtime::Result<Result<host::FactoryComponent, String>> {
    let prefix = accessor.with(|access| {
        guest.func_invoke_custom_factory().func().typed::<(
            Resource<Identity>,
            host::CustomInput,
            Resource<Emitter>,
        ), (Result<host::CustomBuilt, String>,)>(
            access
        )
    })?;
    match prefix
        .call_concurrent(accessor, (factory, input, emitter))
        .await?
        .0
    {
        Ok(host::CustomBuilt::Ready(built)) => Ok(Ok(built)),
        Ok(host::CustomBuilt::Pending(pending)) => {
            let future = accessor.with(|access| {
                guest
                    .func_invoke_custom_future()
                    .func()
                    .typed::<(Resource<Identity>,), (Result<host::FactoryComponent, String>,)>(
                        access,
                    )
            })?;
            let result = future
                .call_concurrent(accessor, (Resource::new_borrow(pending.rep()),))
                .await?
                .0;
            release(accessor, guest, pending).await?;
            Ok(result)
        }
        Err(error) => Ok(Err(error)),
    }
}

/// Exercises a component after its factory prefix has returned.
async fn view(
    accessor: &Accessor<State, Imports>,
    guest: &Guest,
    built: host::FactoryComponent,
    emitter: Resource<Emitter>,
) -> wasmtime::Result<Result<(), String>> {
    let script = accessor
        .with(|mut access| access.get().observed.ui.custom_script.clone())
        .unwrap_or_default();
    assert_eq!(built.width, script.width);
    let clock = (!script.ticks.is_empty()).then(super::custom_ticks::Clock::default);
    accessor.with(|mut access| access.get().observed.tick_clock = clock.clone());
    let work = async {
        let result = drive_view(accessor, guest, (&built, &emitter), script).await;
        if let Some(dispose) = &built.dispose {
            let disposed = dispose_once(accessor, guest, dispose, &emitter).await;
            if matches!(result, Ok(Ok(())))
                && let Err(error) = disposed?
            {
                return Ok(Err(error));
            }
        }
        result
    };
    let result = if clock.is_some() {
        super::custom_ticks::drive(accessor, guest, work).await
    } else {
        work.await
    };
    if let Some(clock) = clock {
        clock.assert_idle();
    }
    accessor.with(|mut access| access.get().observed.tick_clock = None);
    for callback in [Some(built.component), built.dispose, built.set_expanded]
        .into_iter()
        .flatten()
    {
        release(accessor, guest, callback).await?;
    }
    result
}
/// Renders supplied frames and delivers terminal input through the retained component.
async fn drive_view(
    accessor: &Accessor<State, Imports>,
    guest: &Guest,
    (built, emitter): (&host::FactoryComponent, &Resource<Emitter>),
    script: Script,
) -> wasmtime::Result<Result<(), String>> {
    let render = accessor.with(|access| {
        guest
            .func_component_render()
            .func()
            .typed::<(Resource<Identity>, u32, Resource<Emitter>), (Result<Vec<String>, String>,)>(
                access,
            )
    })?;
    let input = accessor.with(|access| {
        guest
            .func_component_handle_input()
            .func()
            .typed::<(Resource<Identity>, String, Resource<Emitter>), (Result<(), String>,)>(access)
    })?;
    assert_eq!(script.frames.len(), script.inputs.len() + 1);
    for (index, frame) in script.frames.iter().enumerate() {
        let clock = accessor.with(|mut access| access.get().observed.tick_clock.clone());
        if let Some(clock) = clock {
            clock.advance(*script.ticks.get(index).unwrap_or(&0)).await;
        }
        let (rows,) = render
            .call_concurrent(
                accessor,
                (
                    Resource::new_borrow(built.component.rep()),
                    script.columns,
                    Resource::new_borrow(emitter.rep()),
                ),
            )
            .await?;
        assert_eq!(rows.unwrap(), *frame);
        if let Some(data) = script.inputs.get(index) {
            let (result,) = input
                .call_concurrent(
                    accessor,
                    (
                        Resource::new_borrow(built.component.rep()),
                        data.clone(),
                        Resource::new_borrow(emitter.rep()),
                    ),
                )
                .await?;
            if let Some(error) = script.input_errors.get(&index) {
                assert_eq!(result, Err(error.clone()));
            } else if let Err(error) = result {
                return Ok(Err(error));
            }
        }
    }
    Ok(Ok(()))
}
/// Runs the component's disposal through the guest export.
async fn dispose_once(
    accessor: &Accessor<State, Imports>,
    guest: &Guest,
    dispose: &Resource<Identity>,
    emitter: &Resource<Emitter>,
) -> wasmtime::Result<Result<(), String>> {
    let function = accessor.with(|access| {
        guest
            .func_invoke_dispose()
            .func()
            .typed::<(Resource<Identity>, Resource<Emitter>), (Result<(), String>,)>(access)
    })?;
    let (result,) = function
        .call_concurrent(
            accessor,
            (
                Resource::new_borrow(dispose.rep()),
                Resource::new_borrow(emitter.rep()),
            ),
        )
        .await?;
    Ok(result)
}
/// Releases the original guest body before removing its host-side identity.
async fn release(
    accessor: &Accessor<State, Imports>,
    guest: &Guest,
    callback: Resource<Identity>,
) -> wasmtime::Result<()> {
    release_retained(accessor, guest, &callback).await?;
    accessor.with(|mut access| access.get().table.delete(callback))?;
    Ok(())
}
/// Releases a guest-owned callback; its original guest handle removes the host resource.
pub(super) async fn release_retained(
    accessor: &Accessor<State, Imports>,
    guest: &Guest,
    callback: &Resource<Identity>,
) -> wasmtime::Result<()> {
    let function = accessor.with(|access| {
        guest
            .func_release()
            .func()
            .typed::<(Resource<Identity>,), ()>(access)
    })?;
    function
        .call_concurrent(accessor, (Resource::new_borrow(callback.rep()),))
        .await?;
    Ok(())
}
