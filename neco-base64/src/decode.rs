use crate::error::Base64Error;

fn standard_value(byte: u8) -> Result<u8, Base64Error> {
    match byte {
        b'A'..=b'Z' => Ok(byte - b'A'),
        b'a'..=b'z' => Ok(byte - b'a' + 26),
        b'0'..=b'9' => Ok(byte - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(Base64Error::InvalidCharacter),
    }
}

fn url_value(byte: u8) -> Result<u8, Base64Error> {
    match byte {
        b'A'..=b'Z' => Ok(byte - b'A'),
        b'a'..=b'z' => Ok(byte - b'a' + 26),
        b'0'..=b'9' => Ok(byte - b'0' + 52),
        b'-' => Ok(62),
        b'_' => Ok(63),
        _ => Err(Base64Error::InvalidCharacter),
    }
}

fn decode_with_alphabet(
    input: &str,
    value_fn: fn(u8) -> Result<u8, Base64Error>,
) -> Result<Vec<u8>, Base64Error> {
    let unpadded = input.trim_end_matches('=');

    if unpadded.bytes().any(|b| b == b'=') {
        return Err(Base64Error::InvalidCharacter);
    }

    if unpadded.len() % 4 == 1 {
        return Err(Base64Error::InvalidLength);
    }

    let mut output = Vec::with_capacity((unpadded.len() * 3) / 4);
    let bytes = unpadded.as_bytes();
    let mut i = 0;

    while i + 4 <= bytes.len() {
        let a = value_fn(bytes[i])?;
        let b = value_fn(bytes[i + 1])?;
        let c = value_fn(bytes[i + 2])?;
        let d = value_fn(bytes[i + 3])?;
        output.push((a << 2) | (b >> 4));
        output.push(((b & 0x0f) << 4) | (c >> 2));
        output.push(((c & 0x03) << 6) | d);
        i += 4;
    }

    match bytes.len() - i {
        0 => {}
        2 => {
            let a = value_fn(bytes[i])?;
            let b = value_fn(bytes[i + 1])?;
            output.push((a << 2) | (b >> 4));
        }
        3 => {
            let a = value_fn(bytes[i])?;
            let b = value_fn(bytes[i + 1])?;
            let c = value_fn(bytes[i + 2])?;
            output.push((a << 2) | (b >> 4));
            output.push(((b & 0x0f) << 4) | (c >> 2));
        }
        _ => return Err(Base64Error::InvalidLength),
    }

    Ok(output)
}

/// パディングの有無にかかわらず標準 Base64 を復号します。途中にある `=`、無効な文字、4 で割った余りが 1 になる長さは失敗します。
pub fn decode(input: &str) -> Result<Vec<u8>, Base64Error> {
    decode_with_alphabet(input, standard_value)
}

/// パディングの有無にかかわらず URL セーフ Base64 を復号します。標準 Base64 と同じく、途中にある `=`、無効な文字、4 で割った余りが 1 になる長さは失敗します。
pub fn decode_url(input: &str) -> Result<Vec<u8>, Base64Error> {
    decode_with_alphabet(input, url_value)
}

/// パディング文字と非ゼロの未使用末尾ビットを拒否して URL セーフ Base64 を復号します。
pub fn decode_url_strict(input: &str) -> Result<Vec<u8>, Base64Error> {
    if input.contains('=') {
        return Err(Base64Error::InvalidCharacter);
    }

    let remainder = input.len() % 4;
    if remainder == 1 {
        return Err(Base64Error::InvalidLength);
    }

    if remainder > 0 {
        let last = *input.as_bytes().last().ok_or(Base64Error::InvalidLength)?;
        let val = url_value(last)?;
        let padding_bits = match remainder {
            2 => 4,
            3 => 2,
            _ => 0,
        };
        let mask = (1u8 << padding_bits) - 1;
        if val & mask != 0 {
            return Err(Base64Error::NonZeroPaddingBits);
        }
    }

    decode_with_alphabet(input, url_value)
}
