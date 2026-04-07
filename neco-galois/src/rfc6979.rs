// rfc6979.rs — RFC 6979 HMAC-DRBG による決定論的 k 生成
//
// RFC 6979 Section 3.2 の HMAC-DRBG アルゴリズムを実装する。
// secp256k1 / P-256 で共有可能。
// HMAC は neco-sha2 の Hmac を使用。

use neco_sha2::Hmac;

use crate::bigint::U256;

/// HMAC-DRBG で決定論的 k を生成する。
///
/// RFC 6979 Section 3.2:
/// 1. V = 0x01...01 (32 bytes)
/// 2. K = 0x00...00 (32 bytes)
/// 3. K = HMAC_K(V || 0x00 || secret || digest)
/// 4. V = HMAC_K(V)
/// 5. K = HMAC_K(V || 0x01 || secret || digest)
/// 6. V = HMAC_K(V)
/// 7. ループ: T = HMAC_K(V); k = bits2int(T); if 1 <= k < order: return k
///
/// # 引数
/// - `secret`: 秘密鍵 (32 bytes)
/// - `digest`: メッセージダイジェスト (32 bytes)
/// - `order`: 群位数 n (U256)
///
/// # 戻り値
/// 1 ≤ k < order を満たす決定論的 k
pub fn generate_k(secret: &[u8; 32], digest: &[u8; 32], order: &U256) -> U256 {
    // Step 1: V = 0x01 * 32
    let mut v = [0x01u8; 32];
    // Step 2: K = 0x00 * 32
    let mut k = [0x00u8; 32];

    // Step 3: K = HMAC_K(V || 0x00 || secret || digest)
    k = hmac_drbg_update(&k, &v, secret, digest, 0x00);
    // Step 4: V = HMAC_K(V)
    v = Hmac::mac(&k, &v);
    // Step 5: K = HMAC_K(V || 0x01 || secret || digest)
    k = hmac_drbg_update(&k, &v, secret, digest, 0x01);
    // Step 6: V = HMAC_K(V)
    v = Hmac::mac(&k, &v);

    // Step 7: 生成ループ
    loop {
        // T = HMAC_K(V)
        v = Hmac::mac(&k, &v);
        let candidate = U256::from_be_bytes(v);

        // 1 <= candidate < order の確認
        if !U256::is_zero(candidate) {
            if let core::cmp::Ordering::Less = U256::cmp(candidate, *order) {
                return candidate;
            }
        }

        // 拒否: K, V を更新して再試行 (RFC 6979 3.2.h)
        k = hmac_drbg_update_no_extra(&k, &v);
        v = Hmac::mac(&k, &v);
    }
}

/// HMAC-DRBG の K 更新: HMAC_K(V || marker || secret || digest)
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

/// HMAC-DRBG の K 更新（拒否後のリシード）: HMAC_K(V || 0x00)
fn hmac_drbg_update_no_extra(k: &[u8; 32], v: &[u8; 32]) -> [u8; 32] {
    let mut h = Hmac::new(k);
    h.update(v);
    h.update(&[0x00]);
    h.finalize()
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fp::PrimeField;
    use crate::secp256k1::Secp256k1Order;

    /// RFC 6979 の既知テストベクタ (secp256k1, SHA-256)
    /// https://www.rfc-editor.org/rfc/rfc6979 Appendix A.2.5
    ///
    /// key = 0xC9AFA9D845BA75166B5C215767B1D6934E50C3DB36E89B127B8A622B120F6721
    /// msg = "sample" → SHA-256
    /// k   = 0xA6E3C57DD01ABE90086538398355DD4C3B17AA873382B0F24D6129493D8AAD60
    #[test]
    fn rfc6979_secp256k1_sample() {
        let secret =
            hex_to_bytes("C9AFA9D845BA75166B5C215767B1D6934E50C3DB36E89B127B8A622B120F6721");
        // SHA-256("sample")
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
