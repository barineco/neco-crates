const MODULUS: u16 = 0x11B;
const GENERATOR: u8 = 0x03;

const fn mul_raw(mut lhs: u8, mut rhs: u8) -> u8 {
    let mut result = 0u8;

    while rhs != 0 {
        if rhs & 1 != 0 {
            result ^= lhs;
        }

        let carry = lhs & 0x80;
        lhs <<= 1;
        if carry != 0 {
            lhs ^= (MODULUS & 0xFF) as u8;
        }

        rhs >>= 1;
    }

    result
}

const fn build_exp_table() -> [u8; 512] {
    let mut table = [0u8; 512];
    let mut value = 1u8;
    let mut index = 0usize;

    while index < 255 {
        table[index] = value;
        value = mul_raw(value, GENERATOR);
        index += 1;
    }

    while index < 512 {
        table[index] = table[index - 255];
        index += 1;
    }

    table
}

const fn build_log_table(exp_table: &[u8; 512]) -> [u8; 256] {
    let mut table = [0u8; 256];
    let mut index = 0usize;

    while index < 255 {
        table[exp_table[index] as usize] = index as u8;
        index += 1;
    }

    table
}

pub(crate) const EXP_TABLE: [u8; 512] = build_exp_table();
pub(crate) const LOG_TABLE: [u8; 256] = build_log_table(&EXP_TABLE);
