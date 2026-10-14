//! Portable completed process status for clipboard effects.
#![cfg(test)]
use std::process::ExitStatus;
/// Portable synthetic completed process status.
#[cfg(unix)]
pub fn status(success: bool) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    ExitStatus::from_raw(if success { 0 } else { 256 })
}
/// Portable synthetic completed process status.
#[cfg(windows)]
pub fn status(success: bool) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    ExitStatus::from_raw(u32::from(!success))
}
