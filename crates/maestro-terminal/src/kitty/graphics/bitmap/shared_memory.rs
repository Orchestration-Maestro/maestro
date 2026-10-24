//! Native named shared-memory reading and unconditional opened-segment cleanup.
#[cfg(target_os = "linux")]
use super::super::command::Format;
use super::super::command::Transmission;
use super::ImageError;
#[cfg(target_os = "linux")]
use super::{MAX_DIMENSION, MAX_SIZE};

/// Select a checked range within the logical image, excluding segment padding.
#[cfg(target_os = "linux")]
fn shared_memory_range(
    t: &Transmission,
    stat_size: u64,
) -> Result<std::ops::Range<u64>, ImageError> {
    if stat_size == 0 {
        return Err(ImageError::InvalidData);
    }
    let expected = if t.format == Format::Png {
        stat_size
    } else {
        if t.width == 0 || t.height == 0 || t.width > MAX_DIMENSION || t.height > MAX_DIMENSION {
            return Err(ImageError::InvalidData);
        }
        u64::from(t.width)
            .checked_mul(u64::from(t.height))
            .and_then(|pixels| pixels.checked_mul(u64::from(Transmission::format_bpp(t.format)?)))
            .ok_or(ImageError::InvalidData)?
    };
    if stat_size < expected {
        return Err(ImageError::InvalidData);
    }
    let start = u64::from(t.offset);
    let end = if t.size == 0 {
        expected
    } else {
        start
            .checked_add(u64::from(t.size))
            .ok_or(ImageError::InvalidData)?
            .min(expected)
    };
    // A PNG segment's size is the caller's; bound it as the file medium bounds its reads.
    if start > end || end - start > MAX_SIZE {
        return Err(ImageError::InvalidData);
    }
    Ok(start..end)
}

/// Read a native segment; release its name on every returned path after open.
#[cfg(target_os = "linux")]
pub(super) fn read(bytes: &[u8], t: &Transmission) -> Result<Vec<u8>, ImageError> {
    use rustix::shm::{self, Mode, OFlags};
    use std::{
        ffi::OsStr,
        fs::File,
        io::{Read, Seek, SeekFrom},
        os::unix::ffi::OsStrExt,
    };
    let name = OsStr::from_bytes(bytes);
    let fd = shm::open(name, OFlags::RDONLY, Mode::empty()).map_err(|_| ImageError::InvalidData)?;
    let mut file = File::from(fd);
    let _cleanup = ReleaseSegment(name);
    let size = file.metadata().map_err(|_| ImageError::InvalidData)?.len();
    let range = shared_memory_range(t, size)?;
    let len = usize::try_from(range.end - range.start).map_err(|_| ImageError::InvalidData)?;
    file.seek(SeekFrom::Start(range.start))
        .map_err(|_| ImageError::InvalidData)?;
    let mut data = vec![0; len];
    file.read_exact(&mut data)
        .map_err(|_| ImageError::InvalidData)?;
    Ok(data)
}

/// Scope-bound native-name release after a successful open.
#[cfg(target_os = "linux")]
struct ReleaseSegment<'a>(&'a std::ffi::OsStr);
#[cfg(target_os = "linux")]
impl Drop for ReleaseSegment<'_> {
    fn drop(&mut self) {
        let _ = rustix::shm::unlink(self.0);
    }
}

/// Platforms awaiting a safe native adapter reject the medium without effects.
#[cfg(not(target_os = "linux"))]
pub(super) fn read(_bytes: &[u8], _t: &Transmission) -> Result<Vec<u8>, ImageError> {
    Err(ImageError::UnsupportedMedium)
}
