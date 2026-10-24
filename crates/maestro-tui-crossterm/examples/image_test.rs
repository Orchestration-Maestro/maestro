//! Loads one PNG and composes the toolkit's inline image demonstration.

#[cfg(unix)]
#[path = "test_themes/color.rs"]
mod color;
#[path = "support/process_terminal.rs"]
mod process_terminal;

/// Native file observation and image display.
#[cfg(unix)]
mod unix {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use maestro_tui::{
        Image, ImageOptions, ImageTheme, Spacer, TUI, TerminalImage, Text,
        images::terminal_image::{ImageDimensions, get_image_dimensions},
        interface::InputListenerResult,
        keys::matches_key,
    };
    use std::{
        io::{self, Write},
        process::ExitCode,
        rc::Rc,
    };
    use tokio::sync::Notify;

    /// Observes the file before acquiring native terminal input.
    pub(super) fn main() -> io::Result<ExitCode> {
        let path = std::env::args()
            .nth(1)
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| "/tmp/test-image.png".into());
        let capabilities = TerminalImage::default().get_capabilities();
        writeln!(io::stdout(), "Terminal capabilities: {capabilities:?}")?;
        writeln!(io::stdout(), "Loading image from: {path}")?;
        let Ok(bytes) = std::fs::read(&path) else {
            writeln!(io::stderr(), "Failed to load image: {path}")?;
            writeln!(
                io::stderr(),
                "Usage: cargo run -p maestro-tui-crossterm --example image_test -- [path-to-image.png]"
            )?;
            return Ok(ExitCode::FAILURE);
        };
        let data = STANDARD.encode(bytes);
        let dimensions = get_image_dimensions(&data, "image/png");
        writeln!(io::stdout(), "Image dimensions: {dimensions:?}")?;
        writeln!(io::stdout())?;
        super::process_terminal::run(async move |tui, images| {
            display(tui, images, data, dimensions).await
        })
    }

    /// Composes the image widgets and waits for the cancellation key.
    async fn display(
        tui: TUI,
        images: TerminalImage,
        data: String,
        dimensions: Option<ImageDimensions>,
    ) -> io::Result<ExitCode> {
        tui.add_child(Rc::new(Text::new(
            "Image Rendering Test".into(),
            1,
            1,
            None,
        )));
        tui.add_child(Rc::new(Spacer::new(1)));
        if let Some(dimensions) = dimensions {
            let muted = super::color::color("muted")?;
            tui.add_child(Rc::new(Image::new(
                data,
                "image/png".into(),
                ImageTheme {
                    fallback_color: Box::new(move |text| muted(text)),
                },
                ImageOptions {
                    max_width_cells: Some(60),
                    dimensions: Some(dimensions),
                    ..ImageOptions::default()
                },
                images,
            )));
        } else {
            tui.add_child(Rc::new(Text::new(
                "Could not parse image dimensions".into(),
                1,
                0,
                None,
            )));
        }
        tui.add_child(Rc::new(Spacer::new(1)));
        tui.add_child(Rc::new(Text::new(
            "Press Ctrl+C to exit".into(),
            1,
            0,
            None,
        )));
        let quit = Rc::new(Notify::new());
        let input_quit = Rc::clone(&quit);
        let remove = tui.add_input_listener(Rc::new(move |data| {
            if matches_key(data, "ctrl+c") {
                input_quit.notify_one();
                InputListenerResult::Consume
            } else {
                InputListenerResult::Pass
            }
        }));
        let result = async {
            tui.start()?;
            quit.notified().await;
            Ok(ExitCode::SUCCESS)
        }
        .await;
        remove();
        result
    }
}

/// Starts the native image observation program.
#[cfg(unix)]
fn main() -> std::io::Result<std::process::ExitCode> {
    unix::main()
}

/// Reports the pending native adapter on unsupported platforms.
#[cfg(not(unix))]
fn main() -> std::io::Result<std::process::ExitCode> {
    process_terminal::unsupported()
}
