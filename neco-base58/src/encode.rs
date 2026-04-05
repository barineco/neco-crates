use crate::ALPHABET;

pub fn encode(input: &[u8]) -> String {
    let leading_zeroes = input.iter().take_while(|&&b| b == 0).count();

    let mut digits: Vec<u8> = Vec::new();

    for &byte in input {
        let mut carry = u32::from(byte);

        for digit in digits.iter_mut() {
            let acc = u32::from(*digit) * 256 + carry;
            *digit = (acc % 58) as u8;
            carry = acc / 58;
        }

        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }

    let mut output = String::with_capacity(leading_zeroes + digits.len());

    for _ in 0..leading_zeroes {
        output.push('1');
    }

    for &d in digits.iter().rev() {
        output.push(ALPHABET[d as usize] as char);
    }

    output
}
