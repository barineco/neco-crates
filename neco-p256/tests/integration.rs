use neco_p256::{EcdsaSignature, PublicKey, SecretKey};
use p256::elliptic_curve::PrimeField;

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

    let mut bytes = signature.to_bytes();
    let mut s_bytes = [0u8; 32];
    s_bytes.copy_from_slice(&bytes[32..]);
    let low_s = p256::Scalar::from_repr(s_bytes.into()).expect("low-S scalar");
    let high_s = -low_s;
    bytes[32..].copy_from_slice(&high_s.to_repr());
    let high_s_signature = EcdsaSignature::from_bytes(bytes);

    let err = public
        .verify_ecdsa_prehash(digest, &high_s_signature)
        .expect_err("high-S should fail");

    assert_eq!(err.to_string(), "invalid signature");
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
