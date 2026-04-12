// SHA-1 per RFC 3174

const H0: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];

#[inline(always)]
fn ch(x: u32, y: u32, z: u32) -> u32 {
    (x & y) | (!x & z)
}

#[inline(always)]
fn parity(x: u32, y: u32, z: u32) -> u32 {
    x ^ y ^ z
}

#[inline(always)]
fn maj(x: u32, y: u32, z: u32) -> u32 {
    (x & y) | (x & z) | (y & z)
}

fn compress(state: &mut [u32; 5], block: &[u8; 64]) {
    let mut w = [0u32; 80];
    for i in 0..16 {
        w[i] = u32::from_be_bytes(block[i * 4..i * 4 + 4].try_into().unwrap());
    }
    for i in 16..80 {
        w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
    }

    let [mut a, mut b, mut c, mut d, mut e] = *state;

    for (i, &wi) in w.iter().enumerate() {
        let (f, k) = match i {
            0..=19 => (ch(b, c, d), 0x5A827999u32),
            20..=39 => (parity(b, c, d), 0x6ED9EBA1u32),
            40..=59 => (maj(b, c, d), 0x8F1BBCDCu32),
            _ => (parity(b, c, d), 0xCA62C1D6u32),
        };
        let temp = a
            .rotate_left(5)
            .wrapping_add(f)
            .wrapping_add(e)
            .wrapping_add(k)
            .wrapping_add(wi);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = temp;
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
}

/// SHA-1 ハッシュコンテキスト。`new()` で生成し `update()` でデータを投入、`finalize()` でダイジェストを得る。
pub struct Sha1 {
    state: [u32; 5],
    buf: [u8; 64],
    buf_len: usize,
    total_len: u64,
}

impl Sha1 {
    /// 新しいハッシュコンテキストを返す。
    pub fn new() -> Self {
        Self {
            state: H0,
            buf: [0u8; 64],
            buf_len: 0,
            total_len: 0,
        }
    }

    /// データを追加する。複数回呼び出せる。
    pub fn update(&mut self, data: &[u8]) {
        let mut data = data;
        self.total_len += data.len() as u64;

        if self.buf_len > 0 {
            let need = 64 - self.buf_len;
            let take = need.min(data.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];
            if self.buf_len == 64 {
                let block: [u8; 64] = self.buf;
                compress(&mut self.state, &block);
                self.buf_len = 0;
            }
        }

        while data.len() >= 64 {
            let block: [u8; 64] = data[..64].try_into().unwrap();
            compress(&mut self.state, &block);
            data = &data[64..];
        }

        if !data.is_empty() {
            self.buf[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    /// パディングを施し 20 バイトのダイジェストを返す。`self` を消費する。
    pub fn finalize(mut self) -> [u8; 20] {
        let bit_len = self.total_len * 8;

        self.buf[self.buf_len] = 0x80;
        self.buf_len += 1;

        if self.buf_len > 56 {
            self.buf[self.buf_len..].fill(0);
            let block: [u8; 64] = self.buf;
            compress(&mut self.state, &block);
            self.buf = [0u8; 64];
            self.buf_len = 0;
        }

        self.buf[self.buf_len..56].fill(0);
        self.buf[56..64].copy_from_slice(&bit_len.to_be_bytes());
        let block: [u8; 64] = self.buf;
        compress(&mut self.state, &block);

        let mut digest = [0u8; 20];
        for (i, word) in self.state.iter().enumerate() {
            digest[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        digest
    }

    /// ワンショットで SHA-1 ダイジェストを計算する。
    pub fn digest(data: &[u8]) -> [u8; 20] {
        let mut h = Self::new();
        h.update(data);
        h.finalize()
    }
}

impl Default for Sha1 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    // RFC 3174 §7.3 テストベクタ

    #[test]
    fn empty() {
        assert_eq!(
            hex(&Sha1::digest(b"")),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
    }

    #[test]
    fn abc() {
        assert_eq!(
            hex(&Sha1::digest(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq() {
        assert_eq!(
            hex(&Sha1::digest(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
    }

    #[test]
    fn million_a() {
        let data = vec![b'a'; 1_000_000];
        assert_eq!(
            hex(&Sha1::digest(&data)),
            "34aa973cd4c4daa4f61eeb2bdbad27316534016f"
        );
    }

    #[test]
    fn streaming_equals_oneshot() {
        let data = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        let oneshot = Sha1::digest(data);
        let mut h = Sha1::new();
        for chunk in data.chunks(7) {
            h.update(chunk);
        }
        assert_eq!(h.finalize(), oneshot);
    }

    #[test]
    fn block_boundary_55_bytes() {
        // 55 bytes: パディング後 0x80 が 56 バイト目に来て同一ブロックに収まる
        let data = vec![b'a'; 55];
        let oneshot = Sha1::digest(&data);
        let mut h = Sha1::new();
        h.update(&data);
        assert_eq!(h.finalize(), oneshot);
    }

    #[test]
    fn block_boundary_56_bytes() {
        // 56 bytes: パディングが 2 ブロックにまたがる
        let data = vec![b'a'; 56];
        let oneshot = Sha1::digest(&data);
        let mut h = Sha1::new();
        h.update(&data[..28]);
        h.update(&data[28..]);
        assert_eq!(h.finalize(), oneshot);
    }
}
