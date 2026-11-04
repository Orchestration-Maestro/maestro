//! Native runtime construction shared by unit and integration observations.

/// Build the current-thread runtime used by browser qualification tests.
///
/// # Errors
/// Returns an error if the runtime's native resources cannot be initialized.
pub fn runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
}
