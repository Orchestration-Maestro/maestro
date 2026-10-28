//! Linux socket filters consumed by bubblewrap after the trusted proxy bridges start.

/// One Linux `sock_filter` instruction: opcode, true jump, false jump and operand.
type Instruction = (u16, u8, u8, u32);
/// Native x86-64 filtering, including the architecture and x32 ABI checks.
const X86_64: &[Instruction] = &[
    (0x20, 0, 0, 4),
    (0x15, 0, 10, 0xc000_003e),
    (0x20, 0, 0, 0),
    (0x35, 0, 1, 0x4000_0000),
    (0x15, 0, 7, 0xffff_ffff),
    (0x15, 0, 4, 41),
    (0x20, 0, 0, 20),
    (0x15, 0, 2, 0),
    (0x20, 0, 0, 16),
    (0x15, 1, 0, 1),
    (0x06, 0, 0, 0x7fff_0000),
    (0x06, 0, 0, 0x0005_0001),
    (0x06, 0, 0, 0),
];
/// Native `AArch64` filtering with its architecture check.
const AARCH64: &[Instruction] = &[
    (0x20, 0, 0, 4),
    (0x15, 0, 8, 0xc000_00b7),
    (0x20, 0, 0, 0),
    (0x15, 0, 4, 198),
    (0x20, 0, 0, 20),
    (0x15, 0, 2, 0),
    (0x20, 0, 0, 16),
    (0x15, 1, 0, 1),
    (0x06, 0, 0, 0x7fff_0000),
    (0x06, 0, 0, 0x0005_0001),
    (0x06, 0, 0, 0),
];

/// Encodes the native little-endian instruction layout for a supported host architecture.
pub(crate) fn filter(architecture: &str) -> Result<Vec<u8>, String> {
    let instructions = match architecture {
        "x86_64" => X86_64,
        "aarch64" | "arm64" => AARCH64,
        _ => {
            return Err(format!(
                "Unix socket filtering is unavailable on {architecture}"
            ));
        }
    };
    let mut bytes = Vec::with_capacity(instructions.len() * 8);
    for &(code, yes, no, value) in instructions {
        bytes.extend_from_slice(&code.to_le_bytes());
        bytes.extend_from_slice(&[yes, no]);
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    Ok(bytes)
}
