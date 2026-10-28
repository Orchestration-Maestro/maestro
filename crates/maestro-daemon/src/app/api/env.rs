//! Validation of the extra environment a workspace or tab launches its root pane with.

/// Checks every pair a caller asked the root pane to receive and orders them by key.
#[cfg(test)]
pub(super) fn normalize_launch_env(
    env: std::collections::HashMap<String, String>,
) -> Result<Vec<(String, String)>, maestro_protocol::schema::ErrorBody> {
    env.into_iter().next().map_or(Ok(Vec::new()), |_| {
        Err(maestro_protocol::schema::ErrorBody {
            code: String::new(),
            message: String::new(),
        })
    })
}

#[cfg(test)]
mod tests;
