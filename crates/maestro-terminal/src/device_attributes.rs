//! Device-attribute request selection.

/// Device-attribute report requested by the input stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Req {
    /// Primary terminal capabilities.
    Primary,
    /// Secondary terminal identification.
    Secondary,
    /// Tertiary terminal identification.
    Tertiary,
}

/// Primary device attributes (DA1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Primary {
    /// Numeric terminal conformance level.
    pub conformance_level: u16,
    /// Advertised numeric feature attributes, in wire order.
    pub features: Vec<u16>,
}

impl Default for Primary {
    fn default() -> Self {
        Self {
            conformance_level: 62,
            features: vec![22],
        }
    }
}

impl Primary {
    /// Encode the primary response.
    ///
    /// # Errors
    /// Returns the writer's original error on failure.
    pub fn encode<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        write!(writer, "\x1b[?{}", self.conformance_level)?;
        for feature in &self.features {
            write!(writer, ";{feature}")?;
        }
        writer.write_all(b"c")
    }
}

/// Secondary device attributes (DA2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Secondary {
    /// Numeric terminal type identifier.
    pub device_type: u16,
    /// Firmware or patch version.
    pub firmware_version: u16,
    /// ROM cartridge registration number.
    pub rom_cartridge: u16,
}

impl Default for Secondary {
    fn default() -> Self {
        Self {
            device_type: 1,
            firmware_version: 0,
            rom_cartridge: 0,
        }
    }
}

impl Secondary {
    /// Encode the secondary response.
    ///
    /// # Errors
    /// Returns the writer's original error on failure.
    pub fn encode<W: std::io::Write>(self, writer: &mut W) -> std::io::Result<()> {
        write!(
            writer,
            "\x1b[>{};{};{}c",
            self.device_type, self.firmware_version, self.rom_cartridge
        )
    }
}

/// Tertiary device attributes (DA3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tertiary {
    /// Unit identification encoded as eight uppercase hexadecimal digits.
    pub unit_id: u32,
}

impl Tertiary {
    /// Encode the tertiary response.
    ///
    /// # Errors
    /// Returns the writer's original error on failure.
    pub fn encode<W: std::io::Write>(self, writer: &mut W) -> std::io::Result<()> {
        write!(writer, "\x1bP!|{:08X}\x1b\\", self.unit_id)
    }
}
