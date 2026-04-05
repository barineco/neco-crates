use crate::error::Base58Error;
use crate::ALPHABET;

pub fn decode(input: &str) -> Result<Vec<u8>, Base58Error> {
    let leading_zeroes = input.chars().take_while(|&ch| ch == '1').count();
    let mut decoded = Vec::<u8>::new();

    for ch in input.chars() {
        if !ch.is_ascii() {
            return Err(Base58Error::InvalidCharacter(ch));
        }
        let value = ALPHABET
            .iter()
            .position(|&b| b == ch as u8)
            .ok_or(Base58Error::InvalidCharacter(ch))?;
        let mut carry = value as u32;

        for byte in decoded.iter_mut().rev() {
            let acc = u32::from(*byte) * 58 + carry;
            *byte = (acc & 0xff) as u8;
            carry = acc >> 8;
        }

        while carry > 0 {
            decoded.insert(0, (carry & 0xff) as u8);
            carry >>= 8;
        }
    }

    if leading_zeroes > 0 {
        let mut output = vec![0; leading_zeroes];
        output.extend(decoded);
        return Ok(output);
    }

    Ok(decoded)
}
