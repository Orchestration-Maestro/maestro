//! Image command execution against terminal-owned screen storage.

use super::{
    command::{
        Command, Control, CursorMovement, Display, Format, Medium, Quiet, Response, Transmission,
    },
    image::{ImageError, Limits, LoadingImage},
    storage::{Location, Placement},
};
use crate::{Terminal, page_list::TrackedPin, screen_set::Key};

impl Terminal {
    /// Enable images for this terminal, including PNG and supported local media.
    ///
    /// Each screen admits 64 MiB of pixels; APC payloads admit 16 MiB. Newly
    /// selected screens inherit these permissions. Resets retain host policy.
    pub fn enable_kitty_graphics(&mut self) {
        self.kitty_png_enabled = true;
        self.apc_handler.max_bytes = 16 * 1024 * 1024;
        for key in [Key::Primary, Key::Alternate] {
            if let Some(screen) = self.screens.get_mut(key) {
                screen
                    .kitty_images
                    .set_limit(&mut screen.pages, 64 * 1024 * 1024);
                screen.kitty_images.image_limits = Limits::ALL;
            }
        }
    }

    /// Execute one command, returning only replies admitted by its quiet policy.
    /// A zero storage quota disables the entire protocol, including queries.
    pub fn kitty_graphics(&mut self, command: &Command) -> Option<Response> {
        execute(self, command)
    }

    /// Current cell pixel dimensions for image sizing and deletion geometry.
    pub(super) fn image_cell_size(&self) -> (u32, u32) {
        (
            self.width_px / u32::from(self.cols()),
            self.height_px / u32::from(self.rows()),
        )
    }
}

/// Execute and apply the transmission's inherited response policy.
pub fn execute(terminal: &mut Terminal, command: &Command) -> Option<Response> {
    if !terminal.screens.active().kitty_images.enabled() {
        return None;
    }
    let mut quiet = command.quiet;
    let response = match command.control {
        Control::Query(t) => query(terminal, command, t),
        Control::Display(d) => display(terminal, d),
        Control::Delete(d) => {
            let cell = terminal.image_cell_size();
            let screen = terminal.screens.active_mut();
            screen.kitty_images.delete(
                &mut screen.pages,
                cell,
                crate::Coordinate {
                    x: screen.cursor.x,
                    y: u32::from(screen.cursor.y),
                },
                d,
            );
            Response::default()
        }
        Control::Transmit(_) | Control::TransmitAndDisplay { .. } => {
            if let Some(loading) = &mut terminal.screens.active_mut().kitty_images.loading {
                if quiet == Quiet::No {
                    quiet = loading.quiet;
                } else {
                    loading.quiet = quiet;
                }
            }
            transmit(terminal, command)
        }
        Control::TransmitAnimationFrame(_)
        | Control::ControlAnimation(_)
        | Control::ComposeAnimation(_) => failure("ERROR: unimplemented action"),
    };
    match quiet {
        Quiet::No => (!response.empty()).then_some(response),
        Quiet::Ok => (!response.ok()).then_some(response),
        Quiet::Failures => None,
    }
}

/// A protocol error without image identity, suppressed when it cannot be encoded.
fn failure(message: &str) -> Response {
    Response {
        message: message.as_bytes().to_vec(),
        ..Response::default()
    }
}

/// Build the identity portion before loading can assign an automatic ID.
fn identity(t: Transmission) -> Response {
    Response {
        id: t.image_id,
        image_number: t.image_number,
        placement_id: t.placement_id,
        ..Response::default()
    }
}

/// Query only initial media admission; it does not validate or persist pixels.
fn query(terminal: &Terminal, command: &Command, t: Transmission) -> Response {
    if t.image_id == 0 {
        return failure("EINVAL: image ID required");
    }
    let mut result = identity(t);
    if let Err(error) = init_loading(terminal, command) {
        result.message = error_message(error).as_bytes().to_vec();
    }
    result
}

/// Load first-command media while respecting terminal-local PNG admission.
fn init_loading(terminal: &Terminal, command: &Command) -> Result<LoadingImage, ImageError> {
    if command
        .transmission()
        .is_some_and(|t| t.format == Format::Png && t.medium != Medium::Direct)
        && !terminal.kitty_png_enabled
    {
        return Err(ImageError::UnsupportedMedium);
    }
    LoadingImage::init(command, terminal.screens.active().kitty_images.image_limits)
}

/// Transmit pixels, then apply the first chunk's deferred display request.
fn transmit(terminal: &mut Terminal, command: &Command) -> Response {
    let Some(t) = command.transmission().copied() else {
        return Response::default();
    };
    if t.image_id > 0 && t.image_number > 0 {
        return failure("EINVAL: image ID and number are mutually exclusive");
    }
    let mut result = identity(t);
    match load_and_add_image(terminal, command, t.more_chunks) {
        Ok(None) => return Response::default(),
        Ok(Some(loaded)) => {
            if let Some(mut d) = loaded.display {
                d.image_id = loaded.id;
                result = display(terminal, d);
            }
            if loaded.implicit {
                return Response::default();
            }
            result.id = loaded.id;
        }
        Err(message) => result.message = message.as_bytes().to_vec(),
    }
    result
}

/// Completed upload identity and its first-command placement request.
struct LoadedImage {
    /// Stored protocol identity.
    id: u32,
    /// Suppress acknowledgements for anonymously assigned identities.
    implicit: bool,
    /// Deferred first-command display request.
    display: Option<Display>,
}

/// Append a continuation or start an upload, retaining partial state until completion.
fn load_and_add_image(
    terminal: &mut Terminal,
    command: &Command,
    more: bool,
) -> Result<Option<LoadedImage>, &'static str> {
    let storage = &mut terminal.screens.active_mut().kitty_images;
    if let Some(loading) = &mut storage.loading {
        loading.add_data(&command.data).map_err(error_message)?;
        if more {
            return Ok(None);
        }
    }
    let mut loading = if let Some(loading) = storage.loading.take() {
        loading
    } else {
        init_loading(terminal, command).map_err(error_message)?
    };
    let screen = terminal.screens.active_mut();
    if loading.image.id == 0 {
        loading.image.id = screen.kitty_images.next_image_id;
        screen.kitty_images.next_image_id = screen.kitty_images.next_image_id.wrapping_add(1);
        loading.image.implicit_id = loading.image.number == 0;
    }
    if more {
        screen.kitty_images.loading = Some(loading);
        return Ok(None);
    }
    if loading.image.format == Format::Png && !terminal.kitty_png_enabled {
        return Err("EINVAL: unsupported format");
    }
    let image = loading.complete().map_err(error_message)?;
    let result = LoadedImage {
        id: image.id,
        implicit: image.implicit_id,
        display: loading.display,
    };
    let screen = terminal.screens.active_mut();
    screen
        .kitty_images
        .add_image(&mut screen.pages, image)
        .map_err(|_| "ENOMEM: out of memory")?;
    Ok(Some(result))
}

/// Look up an image and retain its placement before moving the cursor.
fn display(terminal: &mut Terminal, d: Display) -> Response {
    if d.image_id == 0 && d.image_number == 0 {
        return failure("EINVAL: image ID or number required");
    }
    let mut result = Response {
        id: d.image_id,
        image_number: d.image_number,
        placement_id: d.placement_id,
        ..Response::default()
    };
    let storage = &terminal.screens.active().kitty_images;
    let image = if d.image_id == 0 {
        storage.image_by_number(d.image_number)
    } else {
        storage.image_by_id(d.image_id)
    };
    let Some(image) = image else {
        result.message = b"ENOENT: image not found".to_vec();
        return result;
    };
    result.id = image.id;
    if let Err(message) = add_placement(terminal, result.id, d) {
        result.message = message.as_bytes().to_vec();
    }
    result
}

/// Validate virtual-parent policy or acquire a placement-owned cursor pin.
fn placement_location(screen: &mut crate::Screen, d: Display) -> Result<Location, &'static str> {
    if d.virtual_placement {
        if d.parent_id > 0 {
            return Err("EINVAL: virtual placement cannot refer to a parent");
        }
        Ok(Location::Virtual)
    } else {
        screen
            .pages
            .tracked_pin(screen.cursor.page_pin)
            .copied()
            .and_then(|pin| screen.pages.track_pin(pin))
            .map(Location::Pin)
            .ok_or("EINVAL: failed to prepare terminal state")
    }
}

/// Own a cursor pin for a physical placement, or a pinless placeholder prototype.
fn add_placement(terminal: &mut Terminal, image_id: u32, d: Display) -> Result<(), &'static str> {
    let cell = terminal.image_cell_size();
    let screen = terminal.screens.active_mut();
    let location = placement_location(screen, d)?;
    let placement = Placement {
        location,
        x_offset: d.x_offset,
        y_offset: d.y_offset,
        source_x: d.x,
        source_y: d.y,
        source_width: d.width,
        source_height: d.height,
        columns: d.columns,
        rows: d.rows,
        z: d.z,
    };
    let size = screen
        .kitty_images
        .image_by_id(image_id)
        .map(|image| placement.grid_size(image, cell))
        .unwrap_or_default();
    if screen
        .kitty_images
        .add_placement(&mut screen.pages, image_id, d.placement_id, placement)
        .is_err()
    {
        if let Location::Pin(pin) = location {
            screen.pages.untrack_pin(pin);
        }
        return Err("EINVAL: invalid data");
    }
    if let Location::Pin(pin) = location
        && d.cursor_movement == CursorMovement::After
    {
        move_cursor(terminal, pin, size);
    }
    Ok(())
}

/// Index through scroll margins, then move right without reapplying origin offsets.
fn move_cursor(terminal: &mut Terminal, pin: TrackedPin, size: (u32, u32)) {
    for _ in 0..size.1 {
        if terminal.index().is_err() {
            break;
        }
    }
    let screen = terminal.screens.active_mut();
    if let Some(pin) = screen.pages.tracked_pin(pin) {
        let x = u32::from(pin.x)
            .saturating_add(size.0)
            .min(u32::from(screen.pages.cols() - 1));
        screen.cursor.pending_wrap = false;
        screen.cursor_horizontal_absolute(u16::try_from(x).unwrap_or(u16::MAX));
    }
}

/// Map loader failures to the protocol's fixed diagnostic strings.
fn error_message(error: ImageError) -> &'static str {
    match error {
        ImageError::InvalidData => "EINVAL: invalid data",
        ImageError::DecompressionFailed => "EINVAL: decompression failed",
        ImageError::DimensionsRequired => "EINVAL: dimensions required",
        ImageError::DimensionsTooLarge => "EINVAL: dimensions too large",
        ImageError::FilePathTooLong => "EINVAL: file path too long",
        ImageError::TemporaryFileNotInTempDir => "EINVAL: temporary file not in temp dir",
        ImageError::TemporaryFileNotNamedCorrectly => "EINVAL: temporary file not named correctly",
        ImageError::UnsupportedFormat => "EINVAL: unsupported format",
        ImageError::UnsupportedMedium => "EINVAL: unsupported medium",
        ImageError::UnsupportedDepth => "EINVAL: unsupported pixel depth",
    }
}
