use neco_p256::{EcdsaSignature, PublicKey, SecretKey};

#[test]
fn ecdsa_sign_verify_roundtrip() {
    let secret = SecretKey::generate().expect("generate");
    let public = secret.public_key().expect("public_key");
    let digest = [0x42; 32];

    let signature = secret.sign_ecdsa_prehash(digest).expect("sign");

    public
        .verify_ecdsa_prehash(digest, &signature)
        .expect("verify");
}

#[test]
fn ecdsa_verify_wrong_message() {
    let secret = SecretKey::generate().expect("generate");
    let public = secret.public_key().expect("public_key");
    let signature = secret.sign_ecdsa_prehash([0x11; 32]).expect("sign");

    let err = public
        .verify_ecdsa_prehash([0x22; 32], &signature)
        .expect_err("wrong message should fail");

    assert_eq!(err.to_string(), "invalid signature");
}

#[test]
fn ecdsa_reject_high_s() {
    let secret = SecretKey::generate().expect("generate");
    let public = secret.public_key().expect("public_key");
    let digest = [0x77; 32];
    let signature = secret.sign_ecdsa_prehash(digest).expect("sign");

    // s の high-s 相当 (n - s) を big-endian バイト演算で計算する
    let mut bytes = signature.to_bytes();
    let s_bytes: [u8; 32] = bytes[32..].try_into().unwrap();

    // P-256 の群位数 n (big-endian)
    const N: [u8; 32] = [
        0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xBC, 0xE6, 0xFA, 0xAD, 0xA7, 0x17, 0x9E, 0x84, 0xF3, 0xB9, 0xCA, 0xC2, 0xFC, 0x63,
        0x25, 0x51,
    ];

    // high_s = n - s (バイトレベル減算)
    let high_s = be256_sub(N, s_bytes);

    // half_n = n >> 1
    let half_n = be256_shr1(N);

    // high_s > half_n の場合のみ拒否される
    if be256_gt(high_s, half_n) {
        bytes[32..].copy_from_slice(&high_s);
        let high_s_signature = EcdsaSignature::from_bytes(bytes);

        let err = public
            .verify_ecdsa_prehash(digest, &high_s_signature)
            .expect_err("high-S should fail");

        assert_eq!(err.to_string(), "invalid signature");
    }
}

#[test]
fn hex_roundtrip() {
    let secret = SecretKey::generate().expect("generate");
    let public = secret.public_key().expect("public_key");
    let signature = secret.sign_ecdsa_prehash([0x55; 32]).expect("sign");

    let secret_hex = secret.to_hex();
    let public_hex = public.to_hex();
    let signature_hex = signature.to_hex();

    assert_eq!(
        SecretKey::from_hex(&secret_hex).expect("secret hex"),
        secret
    );
    assert_eq!(
        PublicKey::from_hex(&public_hex).expect("public hex"),
        public
    );
    assert_eq!(
        EcdsaSignature::from_hex(&signature_hex).expect("signature hex"),
        signature
    );
}

// --- テスト用 256-bit 演算補助 ---

/// big-endian 256-bit 減算: a - b
fn be256_sub(a: [u8; 32], b: [u8; 32]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut borrow: u16 = 0;
    for i in (0..32).rev() {
        let diff = (a[i] as i16) - (b[i] as i16) - (borrow as i16);
        if diff < 0 {
            out[i] = (diff + 256) as u8;
            borrow = 1;
        } else {
            out[i] = diff as u8;
            borrow = 0;
        }
    }
    out
}

/// big-endian 256-bit 右シフト 1 bit
fn be256_shr1(a: [u8; 32]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut carry = 0u8;
    for i in 0..32 {
        out[i] = (a[i] >> 1) | carry;
        carry = (a[i] & 1) << 7;
    }
    out
}

/// big-endian 256-bit 大小比較: a > b
fn be256_gt(a: [u8; 32], b: [u8; 32]) -> bool {
    for i in 0..32 {
        if a[i] > b[i] {
            return true;
        }
        if a[i] < b[i] {
            return false;
        }
    }
    false
}
