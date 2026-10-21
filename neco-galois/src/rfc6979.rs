//! RFC 6979 の HMAC-DRBG による決定論的なノンス生成です。

use neco_sha2::Hmac;

use crate::bigint::U256;

/// 32 バイトの秘密鍵・メッセージダイジェストと群位数からノンスを生成します。
/// 初期化時の HMAC 入力は `V`、識別バイト、秘密鍵、ダイジェストの順に連結します。
/// 戻り値は `1 <= k < order` を満たし、範囲外の候補は更新後に再生成します。
pub fn generate_k(secret: &[u8; 32], digest: &[u8; 32], order: &U256) -> U256 {
    let mut v = [0x01u8; 32];
    let mut k = [0x00u8; 32];
    k = hmac_drbg_update(&k, &v, secret, digest, 0x00);
    v = Hmac::mac(&k, &v);
    k = hmac_drbg_update(&k, &v, secret, digest, 0x01);
    v = Hmac::mac(&k, &v);
    loop {
        v = Hmac::mac(&k, &v);
        let candidate = U256::from_be_bytes(v);
        if !U256::is_zero(candidate) {
            if let core::cmp::Ordering::Less = U256::cmp(candidate, *order) {
                return candidate;
            }
        }
        k = hmac_drbg_update_no_extra(&k, &v);
        v = Hmac::mac(&k, &v);
    }
}

/// 秘密鍵とダイジェストを含む入力から HMAC-DRBG の `K` を更新します。
fn hmac_drbg_update(
    k: &[u8; 32],
    v: &[u8; 32],
    secret: &[u8; 32],
    digest: &[u8; 32],
    marker: u8,
) -> [u8; 32] {
    let mut h = Hmac::new(k);
    h.update(v);
    h.update(&[marker]);
    h.update(secret);
    h.update(digest);
    h.finalize()
}

/// 範囲外の候補に続く HMAC-DRBG の `K` 更新です。
fn hmac_drbg_update_no_extra(k: &[u8; 32], v: &[u8; 32]) -> [u8; 32] {
    let mut h = Hmac::new(k);
    h.update(v);
    h.update(&[0x00]);
    h.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fp::PrimeField;
    use crate::secp256k1::Secp256k1Order;

    #[test]
    fn rfc6979_secp256k1_sample() {
        let secret =
            hex_to_bytes("C9AFA9D845BA75166B5C215767B1D6934E50C3DB36E89B127B8A622B120F6721");
        let digest = neco_sha2::Sha256::digest(b"sample");
        let order = Secp256k1Order::MODULUS;
        let k = generate_k(&secret, &digest, &order);
        let expected =
            hex_to_u256("A6E3C57DD01ABE90086538398355DD4C3B17AA873382B0F24D6129493D8AAD60");
        assert_eq!(k, expected);
    }

    fn hex_to_bytes(s: &str) -> [u8; 32] {
        let mut out = [0u8; 32];
        for i in 0..32 {
            out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap();
        }
        out
    }

    fn hex_to_u256(s: &str) -> U256 {
        U256::from_be_bytes(hex_to_bytes(s))
    }
}
