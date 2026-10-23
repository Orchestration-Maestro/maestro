//! Recorded display palette, published environment and render context.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

#[allow(
    dead_code,
    reason = "This program uses part of the shared terminal runtime."
)]
#[path = "../../maestro-test-terminal/src/manual_runtime.rs"]
pub mod manual_runtime;

use crate::display_palette::palette_keys;
use maestro_theme::{
    ColorMode, ColorValue, LiveTheme, SyntaxHighlighter, SyntectHighlighter, Theme, ThemeColor,
    ThemeDirectories, ThemeOptions, ThemeState,
};
use maestro_tools::{ToolRenderContext, ToolRenderPhase};
use maestro_tui::{ImageProtocol, TerminalCapabilities, TerminalImage, tui::ComponentHandle};
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};

/// The theme keys of the display recording, in their recorded order.
pub fn theme_keys() -> Vec<String> {
    palette_keys(include_str!("fixtures/read_display_palette.json"))
}

/// A theme whose key `i` of the read corpus has palette color `i + 1 + shift`.
pub fn palette(shift: u8) -> Theme {
    palette_of(&theme_keys(), shift)
}

/// A theme whose key `i` of `keys` has palette color `i + 1 + shift`.
pub fn palette_of(keys: &[String], shift: u8) -> Theme {
    let colors = keys.iter().enumerate().map(|(index, key)| {
        let color = ThemeColor::Named(key.clone());
        (
            color,
            ColorValue::Index(u8::try_from(index + 1).unwrap() + shift),
        )
    });
    Theme::new(colors, [], ColorMode::Color256, ThemeOptions::default()).unwrap()
}

/// The native highlighter, whose grammars load once per test thread.
pub fn native_highlighter() -> Rc<SyntectHighlighter> {
    thread_local! {
        /// Grammars load once per test thread.
        static NATIVE: Rc<SyntectHighlighter> = Rc::new(SyntectHighlighter::new().unwrap());
    }
    NATIVE.with(Rc::clone)
}

/// The collaborators a renderer receives besides its arguments.
pub struct Env {
    /// Controlled terminal runtime.
    pub runtime: Rc<manual_runtime::ManualRuntime>,
    /// Owner of the published theme.
    pub state: Rc<ThemeState>,
    /// The published theme's handle.
    pub live: LiveTheme,
    /// Classifies code.
    pub highlighter: Rc<dyn SyntaxHighlighter>,
    /// Image support of the terminal.
    pub terminal: TerminalImage,
}

impl Env {
    /// A published palette theme shifted by `shift`, the native highlighter and no image support.
    pub fn new(shift: u8) -> Self {
        Self::with(Rc::new(palette(shift)), native_highlighter())
    }

    /// The environment a recorded context lays its rows out in.
    pub fn from_setup(setup: &Setup) -> Self {
        Self::new(setup.shift)
    }

    /// An environment publishing `theme` and highlighting with `highlighter`.
    pub fn with(theme: Rc<Theme>, highlighter: Rc<dyn SyntaxHighlighter>) -> Self {
        let state = Rc::new(ThemeState::new(ThemeDirectories {
            themes_dir: String::new(),
            custom_themes_dir: String::new(),
        }));
        let live = state.theme();
        let env = Self {
            runtime: Rc::new(manual_runtime::ManualRuntime::default()),
            state,
            live,
            highlighter,
            terminal: TerminalImage::new(|_| None, || 1),
        };
        env.publish(theme);
        env
    }

    /// Publish `theme` as the readers' theme.
    pub fn publish(&self, theme: Rc<Theme>) {
        self.state.set_theme_instance(theme).unwrap();
    }
}

/// Everything a recorded context says.
pub struct Setup {
    /// Absolute documentation README.
    pub readme: String,
    /// Whether the row is expanded.
    pub expanded: bool,
    /// Working directory.
    pub cwd: String,
    /// Image protocol of the terminal.
    pub images: Option<ImageProtocol>,
    /// Whether images are shown.
    pub show_images: bool,
    /// Whether the result is an error.
    pub is_error: bool,
    /// Execution progress.
    pub phase: ToolRenderPhase,
    /// Palette shift of the published theme.
    pub shift: u8,
}

impl Setup {
    /// Decode a recorded context object.
    pub fn from(context: &Value) -> Self {
        let flag = |name: &str| context.get(name).and_then(Value::as_bool).unwrap_or(false);
        let images = match context.get("images").and_then(Value::as_str) {
            Some("kitty") => Some(ImageProtocol::Kitty),
            Some("iterm2") => Some(ImageProtocol::Iterm2),
            Some(other) => panic!("unknown image protocol {other}"),
            None => None,
        };
        Self {
            readme: context["readme"].as_str().unwrap().to_owned(),
            expanded: flag("expanded"),
            cwd: context
                .get("cwd")
                .and_then(Value::as_str)
                .unwrap_or("/work")
                .to_owned(),
            images,
            show_images: context
                .get("showImages")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            is_error: flag("isError"),
            phase: ToolRenderPhase {
                execution_started: flag("executionStarted"),
                args_complete: flag("argsComplete"),
                is_partial: false,
            },
            shift: context
                .get("themeShift")
                .and_then(Value::as_u64)
                .map_or(0, |shift| u8::try_from(shift).unwrap()),
        }
    }
}

/// A render context over `args` for the recorded setup.
pub fn context<'a, S: Default>(
    args: &'a Value,
    setup: &Setup,
    env: &Env,
    last: Option<ComponentHandle>,
) -> ToolRenderContext<'a, S> {
    env.terminal.set_capabilities(TerminalCapabilities {
        images: setup.images,
        true_color: true,
        hyperlinks: true,
    });
    ToolRenderContext {
        renderer_state: None,
        args: Some(args),
        tool_call_id: "call".into(),
        invalidate: Rc::new(|| {}),
        last_component: last,
        state: Rc::new(RefCell::new(S::default())),
        cwd: setup.cwd.clone(),
        phase: setup.phase,
        expanded: setup.expanded,
        show_images: setup.show_images,
        is_error: setup.is_error,
        environment: Rc::new(maestro_tools::ToolRenderEnvironment {
            runtime: env.runtime.clone(),
            readme_path: setup.readme.clone(),
            live_theme: env.live.clone(),
            highlighter: Rc::clone(&env.highlighter),
            terminal_image: env.terminal.clone(),
        }),
    }
}
