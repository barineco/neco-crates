use neco_base64::{
    decode, decode_url, decode_url_strict, encode, encode_url, encode_url_padded, Base64Error,
};

#[test]
fn rfc4648_standard_encode() {
    assert_eq!(encode(b""), "");
    assert_eq!(encode(b"f"), "Zg==");
    assert_eq!(encode(b"fo"), "Zm8=");
    assert_eq!(encode(b"foo"), "Zm9v");
    assert_eq!(encode(b"foob"), "Zm9vYg==");
    assert_eq!(encode(b"fooba"), "Zm9vYmE=");
    assert_eq!(encode(b"foobar"), "Zm9vYmFy");
}

#[test]
fn rfc4648_standard_decode() {
    assert_eq!(decode("").unwrap(), b"");
    assert_eq!(decode("Zg==").unwrap(), b"f");
    assert_eq!(decode("Zm8=").unwrap(), b"fo");
    assert_eq!(decode("Zm9v").unwrap(), b"foo");
    assert_eq!(decode("Zm9vYg==").unwrap(), b"foob");
    assert_eq!(decode("Zm9vYmE=").unwrap(), b"fooba");
    assert_eq!(decode("Zm9vYmFy").unwrap(), b"foobar");
}

#[test]
fn url_safe_alphabet_substitution() {
    let input: &[u8] = &[0xfb, 0xef, 0xbe];
    let standard = encode(input);
    let url = encode_url(input);

    assert!(standard.contains('+') || standard.contains('/'));
    let expected_url = standard
        .replace('+', "-")
        .replace('/', "_")
        .trim_end_matches('=')
        .to_string();
    assert_eq!(url, expected_url);
}

#[test]
fn url_safe_padded() {
    let input = b"f";
    let result = encode_url_padded(input);
    assert!(result.ends_with("=="));
    assert_eq!(decode_url(&result).unwrap(), input);
}

#[test]
fn roundtrip_standard() {
    for input in [
        b"" as &[u8],
        b"a",
        b"ab",
        b"abc",
        b"hello world",
        &[0, 1, 255, 128],
    ] {
        let encoded = encode(input);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }
}

#[test]
fn roundtrip_url_safe() {
    for input in [
        b"" as &[u8],
        b"a",
        b"ab",
        b"abc",
        b"hello world",
        &[0, 1, 255, 128],
    ] {
        let encoded = encode_url(input);
        let decoded = decode_url(&encoded).unwrap();
        assert_eq!(decoded, input);
    }
}

#[test]
fn decode_accepts_padded_and_unpadded() {
    assert_eq!(decode("Zg==").unwrap(), b"f");
    assert_eq!(decode("Zg").unwrap(), b"f");
    assert_eq!(decode("Zm8=").unwrap(), b"fo");
    assert_eq!(decode("Zm8").unwrap(), b"fo");
}

#[test]
fn decode_url_accepts_padded_and_unpadded() {
    assert_eq!(decode_url("Zg==").unwrap(), b"f");
    assert_eq!(decode_url("Zg").unwrap(), b"f");
}

#[test]
fn strict_rejects_non_zero_padding_bits() {
    assert_eq!(
        decode_url_strict("Zh").unwrap_err(),
        Base64Error::NonZeroPaddingBits
    );
}

#[test]
fn strict_accepts_zero_padding_bits() {
    assert_eq!(decode_url_strict("Zg").unwrap(), b"f");
}

#[test]
fn strict_rejects_padding_characters() {
    assert_eq!(
        decode_url_strict("Zg==").unwrap_err(),
        Base64Error::InvalidCharacter
    );
}

#[test]
fn decode_rejects_invalid_characters() {
    assert_eq!(decode("!!!").unwrap_err(), Base64Error::InvalidCharacter);
}

#[test]
fn decode_url_rejects_standard_chars() {
    assert_eq!(
        decode_url("ab+c").unwrap_err(),
        Base64Error::InvalidCharacter
    );
    assert_eq!(
        decode_url("ab/c").unwrap_err(),
        Base64Error::InvalidCharacter
    );
}

#[test]
fn decode_rejects_invalid_length() {
    assert_eq!(decode("A").unwrap_err(), Base64Error::InvalidLength);
}

#[test]
fn empty_input() {
    assert_eq!(encode(b""), "");
    assert_eq!(encode_url(b""), "");
    assert_eq!(decode("").unwrap(), b"");
    assert_eq!(decode_url("").unwrap(), b"");
    assert_eq!(decode_url_strict("").unwrap(), b"");
}
