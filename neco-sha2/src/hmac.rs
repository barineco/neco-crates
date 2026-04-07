// HMAC-SHA256 per RFC 2104

use crate::sha256::Sha256;

const BLOCK_SIZE: usize = 64;

pub struct Hmac {
    inner: Sha256,
    outer_key: [u8; BLOCK_SIZE],
}

impl Hmac {
    pub fn new(key: &[u8]) -> Self {
        // Keys longer than block size are hashed down
        let mut k = [0u8; BLOCK_SIZE];
        if key.len() > BLOCK_SIZE {
            let hk = Sha256::digest(key);
            k[..32].copy_from_slice(&hk);
        } else {
            k[..key.len()].copy_from_slice(key);
        }

        let mut ipad_key = [0u8; BLOCK_SIZE];
        let mut opad_key = [0u8; BLOCK_SIZE];
        for i in 0..BLOCK_SIZE {
            ipad_key[i] = k[i] ^ 0x36;
            opad_key[i] = k[i] ^ 0x5c;
        }

        let mut inner = Sha256::new();
        inner.update(&ipad_key);

        Self {
            inner,
            outer_key: opad_key,
        }
    }

    pub fn update(&mut self, data: &[u8]) -> &mut Self {
        self.inner.update(data);
        self
    }

    pub fn finalize(self) -> [u8; 32] {
        let inner_hash = self.inner.finalize();
        let mut outer = Sha256::new();
        outer.update(&self.outer_key);
        outer.update(&inner_hash);
        outer.finalize()
    }

    pub fn mac(key: &[u8], data: &[u8]) -> [u8; 32] {
        let mut h = Self::new(key);
        h.update(data);
        h.finalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    // RFC 4231 Test Cases

    #[test]
    fn rfc4231_tc1() {
        // Key = 20 bytes of 0x0b, Data = "Hi There"
        let key = [0x0bu8; 20];
        let data = b"Hi There";
        assert_eq!(
            hex(&Hmac::mac(&key, data)),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn rfc4231_tc2() {
        // Key = "Jefe", Data = "what do ya want for nothing?"
        assert_eq!(
            hex(&Hmac::mac(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn rfc4231_tc3() {
        // Key = 20 bytes of 0xaa, Data = 50 bytes of 0xdd
        let key = [0xaau8; 20];
        let data = [0xddu8; 50];
        assert_eq!(
            hex(&Hmac::mac(&key, &data)),
            "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe"
        );
    }

    #[test]
    fn rfc4231_tc4() {
        // Key = 25 bytes 0x01..0x19, Data = 50 bytes of 0xcd
        let key: Vec<u8> = (0x01..=0x19u8).collect();
        let data = [0xcdu8; 50];
        assert_eq!(
            hex(&Hmac::mac(&key, &data)),
            "82558a389a443c0ea4cc819899f2083a85f0faa3e578f8077a2e3ff46729665b"
        );
    }

    #[test]
    fn rfc4231_tc5() {
        // Key = 20 bytes of 0x0c, Data = "Test With Truncation"
        // (RFC 4231 specifies only first 128 bits; we verify full 256-bit output here)
        let key = [0x0cu8; 20];
        let data = b"Test With Truncation";
        assert_eq!(
            hex(&Hmac::mac(&key, data)),
            "a3b6167473100ee06e0c796c2955552bfa6f7c0a6a8aef8b93f860aab0cd20c5"
        );
    }

    #[test]
    fn rfc4231_tc6() {
        // Key = 131 bytes of 0xaa (longer than block size), Data = "Test Using Larger Than Block-Size Key - Hash Key First"
        let key = [0xaau8; 131];
        let data = b"Test Using Larger Than Block-Size Key - Hash Key First";
        assert_eq!(
            hex(&Hmac::mac(&key, data)),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn rfc4231_tc7() {
        // Key = 131 bytes of 0xaa, Data = "This is a test using a larger than block-size key and a larger than block-size data. The key needs to be hashed before being used by the HMAC algorithm."
        let key = [0xaau8; 131];
        let data = b"This is a test using a larger than block-size key and a larger than block-size data. The key needs to be hashed before being used by the HMAC algorithm.";
        assert_eq!(
            hex(&Hmac::mac(&key, data)),
            "9b09ffa71b942fcb27635fbcd5b0e944bfdc63644f0713938a7f51535c3a35e2"
        );
    }
}
