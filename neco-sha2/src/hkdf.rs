// HKDF-SHA256 per RFC 5869

use crate::hmac::Hmac;

const HASH_LEN: usize = 32;
// RFC 5869 §2.3: L ≤ 255 * HashLen
const MAX_OUTPUT: usize = 255 * HASH_LEN;

pub struct Prk([u8; HASH_LEN]);

#[derive(Debug, PartialEq, Eq)]
pub struct HkdfError;

impl core::fmt::Display for HkdfError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("HKDF output length exceeds maximum (255 * 32 = 8160 bytes)")
    }
}

pub struct Hkdf;

impl Hkdf {
    /// HKDF-Extract: PRK = HMAC-Hash(salt, IKM)
    pub fn extract(salt: &[u8], ikm: &[u8]) -> Prk {
        // If salt is not provided, use a string of HashLen zeros
        let salt_used: &[u8] = if salt.is_empty() {
            &[0u8; HASH_LEN]
        } else {
            salt
        };
        Prk(Hmac::mac(salt_used, ikm))
    }
}

impl Prk {
    /// Construct a `Prk` directly from an existing 32-byte key.
    pub fn from_bytes(bytes: &[u8; HASH_LEN]) -> Self {
        Prk(*bytes)
    }

    /// Return the raw PRK bytes.
    pub fn as_bytes(&self) -> &[u8; HASH_LEN] {
        &self.0
    }

    /// HKDF-Expand: OKM = T(1) || T(2) || ... || T(N)
    pub fn expand(&self, info: &[u8], len: usize) -> Result<Vec<u8>, HkdfError> {
        if len > MAX_OUTPUT {
            return Err(HkdfError);
        }
        if len == 0 {
            return Ok(Vec::new());
        }

        let n = len.div_ceil(HASH_LEN);
        let mut okm = Vec::with_capacity(n * HASH_LEN);
        let mut t_prev: Vec<u8> = Vec::new();

        for i in 1..=n {
            let mut h = Hmac::new(&self.0);
            h.update(&t_prev);
            h.update(info);
            h.update(&[i as u8]);
            let t = h.finalize();
            t_prev = t.to_vec();
            okm.extend_from_slice(&t);
        }

        okm.truncate(len);
        Ok(okm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from_hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    // RFC 5869 Appendix A

    #[test]
    fn rfc5869_tc1() {
        let ikm = from_hex("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b");
        let salt = from_hex("000102030405060708090a0b0c");
        let info = from_hex("f0f1f2f3f4f5f6f7f8f9");
        let len = 42;

        let prk = Hkdf::extract(&salt, &ikm);
        assert_eq!(
            hex(&prk.0),
            "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5"
        );

        let okm = prk.expand(&info, len).unwrap();
        assert_eq!(
            hex(&okm),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
        );
    }

    #[test]
    fn rfc5869_tc2() {
        let ikm = from_hex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f404142434445464748494a4b4c4d4e4f");
        let salt = from_hex("606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9fa0a1a2a3a4a5a6a7a8a9aaabacadaeaf");
        let info = from_hex("b0b1b2b3b4b5b6b7b8b9babbbcbdbebfc0c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5e6e7e8e9eaebecedeeeff0f1f2f3f4f5f6f7f8f9fafbfcfdfeff");
        let len = 82;

        let prk = Hkdf::extract(&salt, &ikm);
        assert_eq!(
            hex(&prk.0),
            "06a6b88c5853361a06104c9ceb35b45cef760014904671014a193f40c15fc244"
        );

        let okm = prk.expand(&info, len).unwrap();
        assert_eq!(
            hex(&okm),
            "b11e398dc80327a1c8e7f78c596a49344f012eda2d4efad8a050cc4c19afa97c59045a99cac7827271cb41c65e590e09da3275600c2f09b8367793a9aca3db71cc30c58179ec3e87c14c01d5c1f3434f1d87"
        );
    }

    #[test]
    fn rfc5869_tc3() {
        // No salt, no info
        let ikm = from_hex("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b");
        let salt = b"";
        let info = b"";
        let len = 42;

        let prk = Hkdf::extract(salt, &ikm);
        assert_eq!(
            hex(&prk.0),
            "19ef24a32c717b167f33a91d6f648bdf96596776afdb6377ac434c1c293ccb04"
        );

        let okm = prk.expand(info, len).unwrap();
        assert_eq!(
            hex(&okm),
            "8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d9d201395faa4b61a96c8"
        );
    }

    #[test]
    fn error_on_too_long() {
        let prk = Hkdf::extract(b"salt", b"ikm");
        assert_eq!(prk.expand(b"info", 8161), Err(HkdfError));
    }

    #[test]
    fn empty_output() {
        let prk = Hkdf::extract(b"salt", b"ikm");
        assert_eq!(prk.expand(b"info", 0).unwrap(), Vec::<u8>::new());
    }
}
