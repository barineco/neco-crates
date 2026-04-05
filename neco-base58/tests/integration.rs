use neco_base58::{decode, encode, Base58Error};

#[test]
fn encode_empty() {
    assert_eq!(encode(b""), "");
}

#[test]
fn decode_empty() {
    assert_eq!(decode("").unwrap(), Vec::<u8>::new());
}

#[test]
fn encode_abc() {
    assert_eq!(encode(b"abc"), "ZiCa");
}

#[test]
fn decode_abc() {
    assert_eq!(decode("ZiCa").unwrap(), b"abc".to_vec());
}

#[test]
fn encode_leading_zero_bytes() {
    assert_eq!(encode(&[0, 0]), "11");
}

#[test]
fn decode_leading_zero_bytes() {
    assert_eq!(decode("11").unwrap(), vec![0, 0]);
}

#[test]
fn invalid_char_zero() {
    assert_eq!(decode("0"), Err(Base58Error::InvalidCharacter('0')));
}

#[test]
fn invalid_char_uppercase_o() {
    assert_eq!(decode("O"), Err(Base58Error::InvalidCharacter('O')));
}

#[test]
fn invalid_char_uppercase_i() {
    assert_eq!(decode("I"), Err(Base58Error::InvalidCharacter('I')));
}

#[test]
fn invalid_char_lowercase_l() {
    assert_eq!(decode("l"), Err(Base58Error::InvalidCharacter('l')));
}

#[test]
fn rejects_non_ascii_that_truncates_to_valid_byte() {
    // U+0141 'Ł' truncates to 0x41 = 'A' via `as u8`
    assert_eq!(decode("Ł"), Err(Base58Error::InvalidCharacter('Ł')));
}

#[test]
fn roundtrip_single_bytes() {
    for b in 0..=255u8 {
        let encoded = encode(&[b]);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, vec![b], "roundtrip failed for byte {}", b);
    }
}

#[test]
fn roundtrip_various() {
    let cases: &[&[u8]] = &[
        b"hello",
        b"\x00\x00\xff",
        b"\x01\x02\x03\x04\x05",
        &[0; 10],
        b"The quick brown fox jumps over the lazy dog",
    ];
    for input in cases {
        let encoded = encode(input);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(&decoded, input);
    }
}

#[test]
fn roundtrip_large_input() {
    let input: Vec<u8> = (0..=255).cycle().take(1024).collect();
    let encoded = encode(&input);
    let decoded = decode(&encoded).unwrap();
    assert_eq!(decoded, input);
}
