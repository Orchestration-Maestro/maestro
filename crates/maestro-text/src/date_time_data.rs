//! Validated fixed-profile date-time data.
use std::io;

/// Reads one committed recipe.
fn recipe(_record: &'static str) -> io::Result<()> {
    Ok(())
}

/// Loads committed recipes and their locale index.
fn load(_recipes: &'static str, _index: &'static str) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests;
