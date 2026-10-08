//! Two independent terminal adapters and a probe that plays the user against either.

use std::fmt::Write;
use std::future::{self, Future};
use std::io;
use std::pin::Pin;
use std::time::Duration;

use maestro_tui::terminal::Terminal;

/// Callback delivering input chunks.
pub type InputCallback = Box<dyn FnMut(&str)>;

/// Callback announcing a resize.
pub type ResizeCallback = Box<dyn FnMut()>;

/// Callbacks an adapter keeps after `start`.
#[derive(Default)]
pub struct Callbacks {
    /// Input delivery.
    pub input: Option<InputCallback>,
    /// Resize notice.
    pub resize: Option<ResizeCallback>,
}

/// Logs each effect as a readable line.
pub struct Recording {
    /// Effects in order.
    pub log: Vec<String>,
    /// Callbacks given to `start`.
    pub callbacks: Callbacks,
    /// Whether `write` fails.
    pub refuse_writes: bool,
}

impl Recording {
    /// Creates an adapter; `refuse_writes` makes every write fail.
    pub fn new(refuse_writes: bool) -> Self {
        Self {
            log: Vec::new(),
            callbacks: Callbacks::default(),
            refuse_writes,
        }
    }

    /// Notes one effect; refused writes fail instead.
    fn done(&mut self, effect: String) -> io::Result<()> {
        if self.refuse_writes && effect.starts_with("write") {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "write refused"));
        }
        self.log.push(effect);
        Ok(())
    }
}

impl Terminal for Recording {
    fn start(&mut self, on_input: InputCallback, on_resize: ResizeCallback) -> io::Result<()> {
        self.callbacks = Callbacks {
            input: Some(on_input),
            resize: Some(on_resize),
        };
        self.done("start".into())
    }

    fn stop(&mut self) -> io::Result<()> {
        self.done("stop".into())
    }

    fn drain_input(
        &mut self,
        max: Option<Duration>,
        idle: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = io::Result<()>> + '_>> {
        Box::pin(async move { self.done(format!("drain {max:?} {idle:?}")) })
    }

    fn write(&mut self, data: &str) -> io::Result<()> {
        self.done(format!("write {data}"))
    }

    fn columns(&self) -> usize {
        80
    }

    fn rows(&self) -> usize {
        24
    }

    fn kitty_protocol_active(&self) -> bool {
        true
    }

    fn move_by(&mut self, lines: isize) -> io::Result<()> {
        self.done(format!("move {lines}"))
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.done("hide".into())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.done("show".into())
    }

    fn clear_line(&mut self) -> io::Result<()> {
        self.done("clear-line".into())
    }

    fn clear_from_cursor(&mut self) -> io::Result<()> {
        self.done("clear-below".into())
    }

    fn clear_screen(&mut self) -> io::Result<()> {
        self.done("clear-screen".into())
    }

    fn set_title(&mut self, title: &str) -> io::Result<()> {
        self.done(format!("title {title}"))
    }

    fn set_progress(&mut self, active: bool) -> io::Result<()> {
        self.done(format!("progress {active}"))
    }
}

/// Appends the escape sequences a device would receive.
#[derive(Default)]
pub struct Screen {
    /// Everything written so far.
    pub output: String,
    /// Callbacks given to `start`.
    pub callbacks: Callbacks,
}

impl Terminal for Screen {
    fn start(&mut self, on_input: InputCallback, on_resize: ResizeCallback) -> io::Result<()> {
        self.callbacks = Callbacks {
            input: Some(on_input),
            resize: Some(on_resize),
        };
        Ok(())
    }

    fn stop(&mut self) -> io::Result<()> {
        self.callbacks = Callbacks::default();
        Ok(())
    }

    fn drain_input(
        &mut self,
        _max: Option<Duration>,
        _idle: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = io::Result<()>> + '_>> {
        Box::pin(future::ready(Ok(())))
    }

    fn write(&mut self, data: &str) -> io::Result<()> {
        self.output.push_str(data);
        Ok(())
    }

    fn columns(&self) -> usize {
        120
    }

    fn rows(&self) -> usize {
        40
    }

    fn kitty_protocol_active(&self) -> bool {
        false
    }

    fn move_by(&mut self, lines: isize) -> io::Result<()> {
        let code = if lines < 0 { 'A' } else { 'B' };
        write!(self.output, "\x1b[{}{code}", lines.unsigned_abs()).map_err(io::Error::other)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.write("\x1b[?25l")
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.write("\x1b[?25h")
    }

    fn clear_line(&mut self) -> io::Result<()> {
        self.write("\x1b[2K")
    }

    fn clear_from_cursor(&mut self) -> io::Result<()> {
        self.write("\x1b[J")
    }

    fn clear_screen(&mut self) -> io::Result<()> {
        self.write("\x1b[2J\x1b[H")
    }

    fn set_title(&mut self, title: &str) -> io::Result<()> {
        self.write(&format!("\x1b]0;{title}\x07"))
    }

    fn set_progress(&mut self, active: bool) -> io::Result<()> {
        self.write(if active {
            "\x1b]9;4;3\x07"
        } else {
            "\x1b]9;4;0;\x07"
        })
    }
}

/// Lets a test play the user against any adapter.
pub trait Probe: Terminal {
    /// Delivers input as the device would.
    fn press(&mut self, data: &str);

    /// Announces a resize as the device would.
    fn resize(&mut self);
}

/// Implements [`Probe`] by calling the callbacks an adapter stored.
macro_rules! probe {
    ($adapter:ty) => {
        impl Probe for $adapter {
            fn press(&mut self, data: &str) {
                if let Some(callback) = self.callbacks.input.as_mut() {
                    callback(data);
                }
            }

            fn resize(&mut self) {
                if let Some(callback) = self.callbacks.resize.as_mut() {
                    callback();
                }
            }
        }
    };
}

probe!(Recording);
probe!(Screen);
