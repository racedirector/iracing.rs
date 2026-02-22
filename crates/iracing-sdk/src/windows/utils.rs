/// Convert string to null-terminated wide string for Windows APIs
pub fn wide_string(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Encodes a car number string into a sortable `u16`, keeping leading zeros
/// distinct by folding them into the thousands place (`1` -> `1`, `01` -> `2001`,
/// `001` -> `3001`). All-zero strings treat one zero as the number itself so we
/// only count the extra zeros.
pub fn pad_car_number(s: &str) -> u16 {
    let bytes = s.as_bytes();
    let len = bytes.len();

    // Count leading zeros without allocating
    let mut zeros = 0usize;
    for &b in bytes {
        if b == b'0' {
            zeros += 1;
        } else {
            break;
        }
    }

    // If the entire string was zeros, subtract 1
    if zeros > 0 && zeros == len {
        zeros -= 1;
    }

    // Parse the numeric value (leading zeros are fine). Fall back to zero for
    // any malformed car numbers so we avoid panicking the caller.
    let num: u16 = s.parse().unwrap_or(0);

    if zeros > 0 {
        let num_place = if num > 99 {
            3
        } else if num > 9 {
            2
        } else {
            1
        };

        num + 1000 * (num_place + zeros as u16)
    } else {
        num
    }
}
