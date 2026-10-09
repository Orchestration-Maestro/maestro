//! An optional animated indicator beside a styled message.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use super::Text;
use crate::tui::{RenderTimer, TuiRuntime};
use crate::{Component, TUI};

/// Optional frames and their animation interval.
#[derive(Default)]
pub struct LoaderIndicatorOptions {
    /// Animation frames; an empty vector hides the indicator.
    pub frames: Option<Vec<String>>,
    /// Frame interval in milliseconds.
    pub interval_ms: Option<f64>,
}

/// Styles a spinner or message.
type ColorFn = Rc<dyn Fn(&str) -> String>;

/// A shared indicator and message composed with [`Text`].
#[derive(Clone)]
pub struct Loader(Rc<State>);

/// Mutable display state shared by loader handles.
struct State {
    /// Wrapped generated content.
    text: Text,
    /// Immutable frame snapshot.
    frames: RefCell<Rc<[String]>>,
    /// Current frame index.
    index: Cell<usize>,
    /// Whether supplied frames bypass spinner styling.
    verbatim: Cell<bool>,
    /// Unstyled message.
    message: RefCell<Rc<str>>,
    /// Spinner styling.
    spinner_color: ColorFn,
    /// Message styling.
    message_color: ColorFn,
    /// Host scheduling effects.
    runtime: Rc<dyn TuiRuntime>,
    /// Delay between animated frames.
    delay: Cell<Option<Duration>>,
    /// Pending animation callback.
    timer: RefCell<Option<Box<dyn RenderTimer>>>,
    /// Weak request to the existing writer.
    request: Rc<dyn Fn()>,
}

impl Loader {
    /// Creates and starts a loader; an absent message is `Loading...`.
    #[must_use]
    pub fn new(
        tui: TUI,
        spinner_color_fn: ColorFn,
        message_color_fn: ColorFn,
        message: Option<String>,
        indicator: Option<LoaderIndicatorOptions>,
    ) -> Self {
        let loader = Self(Rc::new(State {
            text: Text::new(String::new(), 1, 0, None),
            frames: RefCell::new(Rc::from([])),
            index: Cell::new(0),
            verbatim: Cell::new(false),
            message: RefCell::new(message.unwrap_or_else(|| "Loading...".into()).into()),
            spinner_color: spinner_color_fn,
            message_color: message_color_fn,
            runtime: tui.runtime(),
            delay: Cell::new(Some(Duration::from_millis(80))),
            timer: RefCell::new(None),
            request: tui.weak_render_request(),
        }));
        drop(tui);
        loader.set_indicator(indicator);
        loader
    }

    /// Refreshes the display without resetting its frame.
    pub fn start(&self) {
        self.0.update_display();
        self.stop();
        self.0.arm();
    }

    /// Stops animation while retaining displayed content.
    pub fn stop(&self) {
        let timer = self.0.timer.take();
        if let Some(mut timer) = timer {
            timer.cancel();
        }
    }

    /// Replaces the message and refreshes the display, also while stopped.
    pub fn set_message(&self, message: String) {
        self.0.message.replace(message.into());
        self.0.update_display();
    }

    /// Replaces indicator options, resets the frame and starts the loader.
    pub fn set_indicator(&self, indicator: Option<LoaderIndicatorOptions>) {
        self.0.verbatim.set(indicator.is_some());
        let options = indicator.unwrap_or_default();
        let frames = options.frames.unwrap_or_else(|| {
            ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
                .into_iter()
                .map(String::from)
                .collect()
        });
        self.0.frames.replace(frames.into());
        let ms = options.interval_ms.filter(|ms| *ms > 0.0).unwrap_or(80.0);
        self.0.delay.set(
            Duration::try_from_secs_f64(ms / 1000.0)
                .ok()
                .map(|delay| delay.max(Duration::from_nanos(1))),
        );
        self.0.index.set(0);
        self.start();
    }
}

impl State {
    /// Installs the continuation before any tick calls a color function.
    fn arm(self: &Rc<Self>) {
        if self.frames.borrow().len() <= 1 {
            return;
        }
        let Some(delay) = self.delay.get() else {
            return;
        };
        let weak = Rc::downgrade(self);
        let timer = self.runtime.schedule(
            delay,
            Box::new(move || {
                if let Some(state) = weak.upgrade() {
                    state
                        .index
                        .set((state.index.get() + 1) % state.frames.borrow().len());
                    state.arm();
                    state.update_display();
                }
                Ok(())
            }),
        );
        let replaced = self.timer.replace(Some(timer));
        drop(replaced);
    }

    /// Builds content in spinner-then-message callback order, then requests rendering.
    fn update_display(&self) {
        let frames = Rc::clone(&self.frames.borrow());
        let frame = frames.get(self.index.get()).map_or("", String::as_str);
        let rendered = if self.verbatim.get() {
            Cow::Borrowed(frame)
        } else {
            Cow::Owned((self.spinner_color)(frame))
        };
        let indicator = if frame.is_empty() {
            String::new()
        } else {
            format!("{rendered} ")
        };
        let message = Rc::clone(&self.message.borrow());
        self.text
            .set_text(format!("{indicator}{}", (self.message_color)(&message)));
        (self.request)();
    }
}

impl Drop for State {
    fn drop(&mut self) {
        if let Some(mut timer) = self.timer.get_mut().take() {
            timer.cancel();
        }
    }
}

impl Component for Loader {
    fn render(&self, width: usize) -> Vec<String> {
        let mut lines = vec![String::new()];
        lines.extend(self.0.text.render(width));
        lines
    }

    fn invalidate(&self) {
        self.0.text.invalidate();
    }
}
