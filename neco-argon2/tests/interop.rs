use argon2::{Algorithm, Argon2, Params, Version};
use neco_argon2::{argon2id_verify, Argon2Params};

#[test]
fn test_external_phc_verify_by_neco() {
    let password = b"correct horse battery staple";
    let salt = b"saltsalt";

    let params = Params::new(32, 1, 1, Some(32)).unwrap();
    let hasher = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut output = [0u8; 32];
    hasher
        .hash_password_into(password, salt, &mut output)
        .unwrap();

    let neco_hash = neco_argon2::argon2id_hash(password, salt, 32, 1, 1, 32);
    assert_eq!(
        neco_hash,
        output.to_vec(),
        "raw hash bytes must match external crate"
    );
}

#[test]
fn test_neco_phc_roundtrip_with_external_hash() {
    let password = b"password123!";
    let salt = b"randomsalt16byt";

    let params_ext = Params::new(64, 2, 1, Some(32)).unwrap();
    let hasher = Argon2::new(Algorithm::Argon2id, Version::V0x13, params_ext);
    let mut expected = [0u8; 32];
    hasher
        .hash_password_into(password, salt, &mut expected)
        .unwrap();

    let neco_hash = neco_argon2::argon2id_hash(password, salt, 64, 2, 1, 32);
    assert_eq!(
        neco_hash,
        expected.to_vec(),
        "neco hash must match external crate"
    );

    let params = Argon2Params {
        m_cost: 64,
        t_cost: 2,
        p_cost: 1,
        output_len: 32,
    };
    let encoded = neco_argon2::argon2id_hash_encoded(password, params);
    assert!(
        argon2id_verify(&encoded, password),
        "neco verify must pass for own PHC string"
    );
    assert!(
        !argon2id_verify(&encoded, b"wrong password"),
        "neco verify must fail for wrong password"
    );
}

#[test]
fn test_hash_parity_various_params() {
    let password = b"testpassword";
    let salt = b"testsalt12345678";

    for (m, t, p) in [(32u32, 1u32, 1u32), (64, 2, 2), (128, 1, 4)] {
        let params_ext = Params::new(m, t, p, Some(32)).unwrap();
        let hasher = Argon2::new(Algorithm::Argon2id, Version::V0x13, params_ext);
        let mut expected = [0u8; 32];
        hasher
            .hash_password_into(password, salt, &mut expected)
            .unwrap();

        let neco_hash = neco_argon2::argon2id_hash(password, salt, m, t, p, 32);
        assert_eq!(
            neco_hash,
            expected.to_vec(),
            "hash mismatch for m={m}, t={t}, p={p}"
        );
    }
}
