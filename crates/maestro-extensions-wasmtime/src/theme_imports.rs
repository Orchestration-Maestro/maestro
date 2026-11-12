//! Canonical theme methods over the live theme the local owner retains.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    bindings::maestro::extension::host::{ColorMode, HostTheme},
    lifecycle::State,
    read_imports::forward,
    theme_reads::Endpoint,
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};
use wasmtime_wasi::WasiView;

impl From<maestro_extensions::ColorMode> for ColorMode {
    fn from(mode: maestro_extensions::ColorMode) -> Self {
        match mode {
            maestro_extensions::ColorMode::Truecolor => Self::Truecolor,
            maestro_extensions::ColorMode::Color256 => Self::Color256,
        }
    }
}

/// Clones the endpoint a guest handle names before native code runs.
fn endpoint(state: &mut State, this: &Resource<Endpoint>) -> wasmtime::Result<Endpoint> {
    Ok(state.ctx().table.get(this)?.clone())
}

/// Declares the methods that read one endpoint operation with string operands.
macro_rules! reads {
    ($($method:ident($($operand:ident),*) -> $result:ty;)+) => {
        $(fn $method(
            &mut self,
            this: Resource<Endpoint>,
            $($operand: String),*
        ) -> wasmtime::Result<Result<$result, String>> {
            Ok(endpoint(self, &this)?.$method($($operand),*))
        })+
    };
}

impl HostTheme for State {
    reads! {
        name() -> Option<String>;
        source_path() -> Option<String>;
        source_info() -> Option<String>;
        fg(color, text) -> String;
        bg(color, text) -> String;
        bold(text) -> String;
        italic(text) -> String;
        underline(text) -> String;
        inverse(text) -> String;
        strikethrough(text) -> String;
        get_fg_ansi(color) -> String;
        get_bg_ansi(color) -> String;
        thinking_border(level, text) -> String;
        bash_mode_border(text) -> String;
    }

    fn set_source_info(
        &mut self,
        this: Resource<Endpoint>,
        source: Option<String>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(endpoint(self, &this)?.set_source_info(source))
    }

    fn get_color_mode(
        &mut self,
        this: Resource<Endpoint>,
    ) -> wasmtime::Result<Result<ColorMode, String>> {
        Ok(endpoint(self, &this)?.get_color_mode())
    }

    fn drop(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<()> {
        self.ctx().table.delete(this)?;
        Ok(())
    }
}

/// Installs standalone access to the widget host's live palette.
pub(crate) fn add_global<T: crate::api_imports::ApiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "global-theme",
        |mut store: StoreContextMut<'_, T>, (): ()| {
            let theme = store.data().api().0.request(|_, reply| {
                crate::local_resources::Message::Widgets(
                    crate::widget_reads::WidgetMessage::GlobalTheme(reply),
                )
            });
            let theme = match theme {
                Ok(theme) => Ok(store.data_mut().ctx().table.push(theme)?),
                Err(error) => Err(error),
            };
            Ok((theme,))
        },
    )
}

/// Registers each method as a forward to the trait.
macro_rules! forwards {
    ($host:ident, $($wit:literal => $method:ident($($operand:ident: $kind:ty),*);)+) => {$(
        forward(
            $host,
            concat!("[method]theme.", $wit),
            |state, (this, $($operand),*): (Resource<Endpoint>, $($kind),*)| {
                HostTheme::$method(state, this, $($operand),*)
            },
        )?;
    )+};
}

/// Forwards every theme method to its trait implementation.
pub(crate) fn add_to_linker(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    forwards! { host,
        "name" => name();
        "source-path" => source_path();
        "source-info" => source_info();
        "set-source-info" => set_source_info(source: Option<String>);
        "fg" => fg(color: String, text: String);
        "bg" => bg(color: String, text: String);
        "bold" => bold(text: String);
        "italic" => italic(text: String);
        "underline" => underline(text: String);
        "inverse" => inverse(text: String);
        "strikethrough" => strikethrough(text: String);
        "get-fg-ansi" => get_fg_ansi(color: String);
        "get-bg-ansi" => get_bg_ansi(color: String);
        "get-color-mode" => get_color_mode();
        "thinking-border" => thinking_border(level: String, text: String);
        "bash-mode-border" => bash_mode_border(text: String);
    }
    Ok(())
}
