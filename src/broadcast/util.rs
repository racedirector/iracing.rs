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

    // Parse the numeric value (leading zeros are fine)
    let num: u16 = s.parse().unwrap();

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
