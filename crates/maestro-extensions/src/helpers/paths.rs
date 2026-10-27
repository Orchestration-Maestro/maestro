//! Native path requests reuse the existing lexical and filesystem owner.
use super::{decode, encode};
/// Runs one path operation without interpreting its spelling in the adapter.
pub(super) fn call(operation: &str, data: &str) -> Result<String, String> {
    if operation == "paths.expand_path" {
        let (path,): (String,) = decode(data)?;
        return encode(
            &maestro_tools::path_utils::expand_path(&path).map_err(|error| error.to_string())?,
        );
    }
    if operation == "paths.resolve" {
        let (paths,): (Vec<String>,) = decode(data)?;
        let paths: Vec<_> = paths.iter().map(String::as_str).collect();
        return encode(
            &maestro_resources::paths::resolve(&paths).map_err(|error| error.to_string())?,
        );
    }
    let (path, cwd): (String, String) = decode(data)?;
    match operation {
        "paths.resolve_to_cwd" => encode(
            &maestro_tools::path_utils::resolve_to_cwd(&path, &cwd)
                .map_err(|error| error.to_string())?,
        ),
        "paths.resolve_read_path" => encode(
            &maestro_tools::path_utils::resolve_read_path(&path, &cwd)
                .map_err(|error| error.to_string())?,
        ),
        _ => Err(format!("Unknown native helper: {operation}")),
    }
}
