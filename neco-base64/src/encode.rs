const STANDARD_TABLE: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const URL_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn encode_with_table(input: &[u8], table: &[u8; 64], pad: bool) -> String {
    let mut output = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut i = 0;

    while i + 3 <= input.len() {
        let block =
            ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8) | (input[i + 2] as u32);
        output.push(table[((block >> 18) & 0x3f) as usize] as char);
        output.push(table[((block >> 12) & 0x3f) as usize] as char);
        output.push(table[((block >> 6) & 0x3f) as usize] as char);
        output.push(table[(block & 0x3f) as usize] as char);
        i += 3;
    }

    let remainder = input.len() - i;
    if remainder == 1 {
        let block = (input[i] as u32) << 16;
        output.push(table[((block >> 18) & 0x3f) as usize] as char);
        output.push(table[((block >> 12) & 0x3f) as usize] as char);
        if pad {
            output.push_str("==");
        }
    } else if remainder == 2 {
        let block = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8);
        output.push(table[((block >> 18) & 0x3f) as usize] as char);
        output.push(table[((block >> 12) & 0x3f) as usize] as char);
        output.push(table[((block >> 6) & 0x3f) as usize] as char);
        if pad {
            output.push('=');
        }
    }

    output
}

/// Encode bytes as standard Base64 with padding.
pub fn encode(input: &[u8]) -> String {
    encode_with_table(input, STANDARD_TABLE, true)
}

/// Encode bytes as URL-safe Base64 without padding.
pub fn encode_url(input: &[u8]) -> String {
    encode_with_table(input, URL_TABLE, false)
}

/// Encode bytes as URL-safe Base64 with padding.
pub fn encode_url_padded(input: &[u8]) -> String {
    encode_with_table(input, URL_TABLE, true)
}
