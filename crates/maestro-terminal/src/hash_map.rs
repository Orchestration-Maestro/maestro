//! Pure logical region arithmetic for cell-indexed maps.

/// Logical map region sizes, not a native hash-table representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Layout {
    /// Aligned metadata, keys and values byte size.
    pub(crate) total_size: u64,
    /// Keys offset from the metadata start, after the 16-byte header.
    pub(crate) keys_start: u64,
    /// Values offset from the metadata start.
    pub(crate) vals_start: u64,
    /// Raw power-of-two slot count before applying load percentage.
    pub(crate) capacity: u64,
}

/// Scale the entry count before rounding once to a power of two.
pub(crate) fn layout(
    count: u64,
    key_bytes: u64,
    value_bytes: u64,
    value_align: u64,
    load_percent: u64,
) -> Layout {
    let minimum = (count * 100).div_ceil(load_percent);
    let capacity = if minimum == 0 {
        0
    } else {
        minimum.next_power_of_two().min(1 << 31)
    };
    let keys_start = (16 + capacity).div_ceil(key_bytes) * key_bytes;
    let vals_start = (keys_start + capacity * key_bytes).div_ceil(value_align) * value_align;
    Layout {
        total_size: (vals_start + capacity * value_bytes).div_ceil(value_align.max(key_bytes))
            * value_align.max(key_bytes),
        keys_start: keys_start - 16,
        vals_start: vals_start - 16,
        capacity,
    }
}

#[cfg(test)]
mod tests;
