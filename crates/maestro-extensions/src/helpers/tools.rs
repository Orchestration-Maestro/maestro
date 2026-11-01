//! Tool-output helpers delegate all selection and accounting to their existing owner.
use super::{decode, encode};
/// Executes one native output operation.
pub(super) fn call(operation: &str, data: &str) -> Result<String, String> {
    if operation == "tools.truncate_line" {
        let (line, max_chars): (String, Option<usize>) = decode(data)?;
        return encode(&maestro_tools::truncate_line(&line, max_chars));
    }
    let (content, max_lines, max_bytes): (String, Option<usize>, Option<usize>) = decode(data)?;
    let options = maestro_tools::TruncationOptions {
        max_lines,
        max_bytes,
    };
    let result = match operation {
        "tools.truncate_head" => maestro_tools::truncate_head(&content, options),
        "tools.truncate_tail" => maestro_tools::truncate_tail(&content, options),
        _ => return Err(format!("Unknown native helper: {operation}")),
    };
    encode(&result)
}
