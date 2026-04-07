//! Minimal P-256 ECDSA signing core.
//!
//! p256 crate への依存を排除し、neco-galois ベースの自前実装に置換。

use core::fmt;

use neco_galois::generate_k;
use neco_galois::{Fp, P256Field, P256Order, PrimeField, SQRT_EXP_P256, U256};

// -----------------------------------------------------------------------
// 型エイリアス
// -----------------------------------------------------------------------

type FpField = Fp<P256Field>;
type Scalar = Fp<P256Order>;

// -----------------------------------------------------------------------
// P-256 曲線定数
// a = -3 (mod p)
// b = 0x5ac635d8aa3a93e7b3ebbd55769886bc651d06b0cc53b0f63bce3c3e27d2604b
// G = (Gx, Gy)
// -----------------------------------------------------------------------

/// P-256 の係数 a = p - 3 (= -3 mod p)
fn curve_a() -> FpField {
    let (a_val, _) = U256::sub(P256Field::MODULUS, U256::from_u64(3));
    FpField::from_u256(a_val)
}

/// P-256 の係数 b
fn curve_b() -> FpField {
    FpField::from_u256(U256::from_be_bytes([
        0x5a, 0xc6, 0x35, 0xd8, 0xaa, 0x3a, 0x93, 0xe7, 0xb3, 0xeb, 0xbd, 0x55, 0x76, 0x98, 0x86,
        0xbc, 0x65, 0x1d, 0x06, 0xb0, 0xcc, 0x53, 0xb0, 0xf6, 0x3b, 0xce, 0x3c, 0x3e, 0x27, 0xd2,
        0x60, 0x4b,
    ]))
}

/// P-256 生成元 G の x 座標
fn generator_x() -> FpField {
    FpField::from_u256(U256::from_be_bytes([
        0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4, 0x40,
        0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39, 0x45, 0xd8, 0x98,
        0xc2, 0x96,
    ]))
}

/// P-256 生成元 G の y 座標
fn generator_y() -> FpField {
    FpField::from_u256(U256::from_be_bytes([
        0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b, 0x8e, 0xe7, 0xeb, 0x4a, 0x7c, 0x0f, 0x9e,
        0x16, 0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e, 0xce, 0xcb, 0xb6, 0x40, 0x68, 0x37, 0xbf,
        0x51, 0xf5,
    ]))
}

/// P-256 の群位数 n
fn order_u256() -> U256 {
    P256Order::MODULUS
}

// -----------------------------------------------------------------------
// 点の表現
// -----------------------------------------------------------------------

/// アフィン座標点
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AffinePoint {
    x: FpField,
    y: FpField,
}

/// Jacobian 射影座標点 (X:Y:Z)、アフィン座標では (X/Z², Y/Z³)
#[derive(Clone, Copy, Debug)]
struct ProjectivePoint {
    x: FpField,
    y: FpField,
    z: FpField,
    is_infinity: bool,
}

impl ProjectivePoint {
    fn infinity() -> Self {
        ProjectivePoint {
            x: FpField::ZERO,
            y: FpField::ZERO,
            z: FpField::ZERO,
            is_infinity: true,
        }
    }

    fn from_affine(p: AffinePoint) -> Self {
        ProjectivePoint {
            x: p.x,
            y: p.y,
            z: FpField::one(),
            is_infinity: false,
        }
    }
}

// -----------------------------------------------------------------------
// P-256 群演算
// -----------------------------------------------------------------------

/// 射影点の2倍算。P-256 の a=-3 最適化を使用。
/// M = 3*(X - Z²)*(X + Z²) (Hankerson et al. Algorithm 3.21)
fn point_double(p: ProjectivePoint) -> ProjectivePoint {
    if p.is_infinity {
        return ProjectivePoint::infinity();
    }

    let x = p.x;
    let y = p.y;
    let z = p.z;

    // a=-3 最適化: M = 3*(X - Z²)*(X + Z²)
    let z2 = FpField::sqr(z);
    let xmz2 = FpField::sub(x, z2);
    let xpz2 = FpField::add(x, z2);
    let m_part = FpField::mul(xmz2, xpz2);
    let two_m = FpField::add(m_part, m_part);
    let m = FpField::add(two_m, m_part); // M = 3*(X-Z²)*(X+Z²)

    // S = 4*X*Y²
    let y2 = FpField::sqr(y);
    let xy = FpField::mul(x, y2);
    let two_xy = FpField::add(xy, xy);
    let s = FpField::add(two_xy, two_xy);

    // X' = M² - 2*S
    let m2 = FpField::sqr(m);
    let two_s = FpField::add(s, s);
    let x_new = FpField::sub(m2, two_s);

    // Y' = M*(S - X') - 8*Y⁴
    let s_minus_x = FpField::sub(s, x_new);
    let my = FpField::mul(m, s_minus_x);
    let y4 = FpField::sqr(y2);
    let eight_y4 = {
        let two = FpField::add(y4, y4);
        let four = FpField::add(two, two);
        FpField::add(four, four)
    };
    let y_new = FpField::sub(my, eight_y4);

    // Z' = 2*Y*Z
    let yz = FpField::mul(y, z);
    let z_new = FpField::add(yz, yz);

    ProjectivePoint {
        x: x_new,
        y: y_new,
        z: z_new,
        is_infinity: false,
    }
}

/// 射影点の加算 (Jacobian)
fn point_add(p: ProjectivePoint, q: ProjectivePoint) -> ProjectivePoint {
    if p.is_infinity {
        return q;
    }
    if q.is_infinity {
        return p;
    }

    let x1 = p.x;
    let y1 = p.y;
    let z1 = p.z;
    let x2 = q.x;
    let y2 = q.y;
    let z2 = q.z;

    // U1 = X1*Z2², U2 = X2*Z1²
    let z1_sq = FpField::sqr(z1);
    let z2_sq = FpField::sqr(z2);
    let u1 = FpField::mul(x1, z2_sq);
    let u2 = FpField::mul(x2, z1_sq);

    // S1 = Y1*Z2³, S2 = Y2*Z1³
    let s1 = FpField::mul(y1, FpField::mul(z2, z2_sq));
    let s2 = FpField::mul(y2, FpField::mul(z1, z1_sq));

    let h = FpField::sub(u2, u1);
    let r = FpField::sub(s2, s1);

    if FpField::is_zero(h) {
        if FpField::is_zero(r) {
            return point_double(p);
        } else {
            return ProjectivePoint::infinity();
        }
    }

    let h2 = FpField::sqr(h);
    let h3 = FpField::mul(h, h2);
    let u1h2 = FpField::mul(u1, h2);

    // X' = R² - H³ - 2*U1*H²
    let r2 = FpField::sqr(r);
    let two_u1h2 = FpField::add(u1h2, u1h2);
    let x3 = FpField::sub(FpField::sub(r2, h3), two_u1h2);

    // Y' = R*(U1*H² - X') - S1*H³
    let u1h2_minus_x3 = FpField::sub(u1h2, x3);
    let s1h3 = FpField::mul(s1, h3);
    let y3 = FpField::sub(FpField::mul(r, u1h2_minus_x3), s1h3);

    // Z' = H*Z1*Z2
    let z3 = FpField::mul(h, FpField::mul(z1, z2));

    ProjectivePoint {
        x: x3,
        y: y3,
        z: z3,
        is_infinity: false,
    }
}

/// 射影点をアフィン点に変換 (Z^-1 で正規化)
fn to_affine(p: ProjectivePoint) -> Option<AffinePoint> {
    if p.is_infinity {
        return None;
    }
    let z_inv = FpField::inv(p.z);
    let z_inv2 = FpField::sqr(z_inv);
    let z_inv3 = FpField::mul(z_inv, z_inv2);
    let x = FpField::mul(p.x, z_inv2);
    let y = FpField::mul(p.y, z_inv3);
    Some(AffinePoint { x, y })
}

/// Montgomery ladder: constant-time scalar multiplication
fn scalar_mul(k: U256, p: AffinePoint) -> ProjectivePoint {
    let mut r0 = ProjectivePoint::infinity();
    let mut r1 = ProjectivePoint::from_affine(p);

    for i in (0..256u32).rev() {
        if U256::bit(k, i) {
            r0 = point_add(r0, r1);
            r1 = point_double(r1);
        } else {
            r1 = point_add(r0, r1);
            r0 = point_double(r0);
        }
    }
    r0
}

/// 点が曲線上にあるか検証: y² = x³ + ax + b (mod p)
fn is_on_curve(p: AffinePoint) -> bool {
    let x = p.x;
    let y = p.y;
    let a = curve_a();
    let b = curve_b();

    let y2 = FpField::sqr(y);
    let x3 = FpField::mul(FpField::sqr(x), x);
    let ax = FpField::mul(a, x);
    let rhs = FpField::add(FpField::add(x3, ax), b);

    FpField::eq(y2, rhs)
}

// -----------------------------------------------------------------------
// SEC1 エンコード/デコード
// -----------------------------------------------------------------------

/// アフィン点を SEC1 圧縮形式 (33バイト) にエンコード
fn encode_sec1_compressed(p: AffinePoint) -> [u8; 33] {
    let x_bytes = p.x.to_u256().to_be_bytes();
    let y_val = p.y.to_u256();
    let prefix = if y_val.l0 & 1 == 1 { 0x03u8 } else { 0x02u8 };
    let mut out = [0u8; 33];
    out[0] = prefix;
    out[1..].copy_from_slice(&x_bytes);
    out
}

/// SEC1 圧縮バイト列から AffinePoint に復元
fn decode_sec1_compressed(bytes: &[u8]) -> Option<AffinePoint> {
    if bytes.len() != 33 {
        return None;
    }
    let prefix = bytes[0];
    if prefix != 0x02 && prefix != 0x03 {
        return None;
    }
    let x_bytes: [u8; 32] = bytes[1..].try_into().ok()?;
    let x_u256 = U256::from_be_bytes(x_bytes);

    // x < p の範囲チェック
    if let core::cmp::Ordering::Less = U256::cmp(x_u256, P256Field::MODULUS) {
        // ok
    } else {
        return None;
    }

    let x = FpField::from_u256(x_u256);
    let a = curve_a();
    let b = curve_b();

    // y² = x³ + ax + b
    let x3 = FpField::mul(FpField::sqr(x), x);
    let ax = FpField::mul(a, x);
    let rhs = FpField::add(FpField::add(x3, ax), b);

    let y = FpField::sqrt(rhs, SQRT_EXP_P256)?;
    let y_u256 = y.to_u256();

    // prefix に合わせて y の奇偶を選択
    let y_odd = y_u256.l0 & 1 == 1;
    let want_odd = prefix == 0x03;

    let y_final = if y_odd == want_odd {
        y
    } else {
        FpField::neg(y)
    };

    let point = AffinePoint { x, y: y_final };

    if !is_on_curve(point) {
        return None;
    }

    Some(point)
}

// -----------------------------------------------------------------------
// ECDSA 署名/検証
// -----------------------------------------------------------------------

/// s > n/2 なら n - s を返す (low-s 正規化)
fn normalize_s(s: U256, n: U256) -> U256 {
    let half_n = U256::shr1(n);
    if let core::cmp::Ordering::Greater = U256::cmp(s, half_n) {
        let (ns, _) = U256::sub(n, s);
        ns
    } else {
        s
    }
}

/// RFC 6979 + ECDSA 署名 (prehash)
fn ecdsa_sign(secret_bytes: &[u8; 32], digest: &[u8; 32]) -> Option<[u8; 64]> {
    let n = order_u256();
    let g = AffinePoint {
        x: generator_x(),
        y: generator_y(),
    };

    let d = U256::from_be_bytes(*secret_bytes);

    // RFC 6979 で決定論的 k を生成
    let k = generate_k(secret_bytes, digest, &n);

    // R = k * G
    let r_proj = scalar_mul(k, g);
    let r_affine = to_affine(r_proj)?;

    // r = x_R mod n
    let rx = r_affine.x.to_u256();
    let r = if let core::cmp::Ordering::Less = U256::cmp(rx, n) {
        rx
    } else {
        let (v, _) = U256::sub(rx, n);
        v
    };
    if U256::is_zero(r) {
        return None;
    }

    // e = digest as integer (mod n)
    let e_raw = U256::from_be_bytes(*digest);
    let e = if let core::cmp::Ordering::Less = U256::cmp(e_raw, n) {
        e_raw
    } else {
        let (v, _) = U256::sub(e_raw, n);
        v
    };

    // s = k^-1 * (e + r*d) mod n
    let k_scalar = Scalar::from_u256(k);
    let k_inv = Scalar::inv(k_scalar);
    let r_scalar = Scalar::from_u256(r);
    let d_scalar = Scalar::from_u256(d);
    let e_scalar = Scalar::from_u256(e);

    let rd = Scalar::mul(r_scalar, d_scalar);
    let e_plus_rd = Scalar::add(e_scalar, rd);
    let s_scalar = Scalar::mul(k_inv, e_plus_rd);
    let s = s_scalar.to_u256();

    if U256::is_zero(s) {
        return None;
    }

    // low-s 正規化
    let s = normalize_s(s, n);

    let mut out = [0u8; 64];
    out[..32].copy_from_slice(&r.to_be_bytes());
    out[32..].copy_from_slice(&s.to_be_bytes());
    Some(out)
}

/// ECDSA 検証 (prehash)
fn ecdsa_verify(pubkey_sec1: &[u8; 33], digest: &[u8; 32], sig_bytes: &[u8; 64]) -> bool {
    let n = order_u256();
    let g = AffinePoint {
        x: generator_x(),
        y: generator_y(),
    };

    let r = U256::from_be_bytes(sig_bytes[..32].try_into().unwrap());
    let s = U256::from_be_bytes(sig_bytes[32..].try_into().unwrap());

    // r, s は [1, n-1] の範囲
    if U256::is_zero(r) || U256::is_zero(s) {
        return false;
    }
    if !matches!(U256::cmp(r, n), core::cmp::Ordering::Less) {
        return false;
    }
    if !matches!(U256::cmp(s, n), core::cmp::Ordering::Less) {
        return false;
    }

    // high-s 拒否
    let half_n = U256::shr1(n);
    if let core::cmp::Ordering::Greater = U256::cmp(s, half_n) {
        return false;
    }

    // 公開鍵
    let pubkey = match decode_sec1_compressed(pubkey_sec1) {
        Some(p) => p,
        None => return false,
    };

    // e = digest mod n
    let e_raw = U256::from_be_bytes(*digest);
    let e = if let core::cmp::Ordering::Less = U256::cmp(e_raw, n) {
        e_raw
    } else {
        let (v, _) = U256::sub(e_raw, n);
        v
    };

    // w = s^-1 mod n
    let s_scalar = Scalar::from_u256(s);
    let w_scalar = Scalar::inv(s_scalar);

    // u1 = e*w mod n, u2 = r*w mod n
    let e_scalar = Scalar::from_u256(e);
    let r_scalar = Scalar::from_u256(r);

    let u1 = Scalar::mul(e_scalar, w_scalar).to_u256();
    let u2 = Scalar::mul(r_scalar, w_scalar).to_u256();

    // R = u1*G + u2*Q
    let u1g = scalar_mul(u1, g);
    let u2q = scalar_mul(u2, pubkey);
    let r_point = point_add(u1g, u2q);

    let r_affine = match to_affine(r_point) {
        Some(p) => p,
        None => return false,
    };

    // r_x mod n == r
    let r_x = r_affine.x.to_u256();
    let r_x_mod_n = if let core::cmp::Ordering::Less = U256::cmp(r_x, n) {
        r_x
    } else {
        let (v, _) = U256::sub(r_x, n);
        v
    };

    U256::cmp(r_x_mod_n, r) == core::cmp::Ordering::Equal
}

// -----------------------------------------------------------------------
// エラー型
// -----------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum P256Error {
    InvalidSecretKey,
    InvalidPublicKey,
    InvalidSignature,
    InvalidHex(&'static str),
}

impl fmt::Display for P256Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSecretKey => f.write_str("invalid secret key"),
            Self::InvalidPublicKey => f.write_str("invalid public key"),
            Self::InvalidSignature => f.write_str("invalid signature"),
            Self::InvalidHex(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for P256Error {}

// -----------------------------------------------------------------------
// Hex ユーティリティ
// -----------------------------------------------------------------------

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, P256Error> {
    let hex = hex.as_bytes();
    if hex.len() % 2 != 0 {
        return Err(P256Error::InvalidHex("odd length"));
    }

    let mut out = Vec::with_capacity(hex.len() / 2);
    for chunk in hex.chunks(2) {
        let high = hex_nibble(chunk[0])?;
        let low = hex_nibble(chunk[1])?;
        out.push((high << 4) | low);
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Result<u8, P256Error> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(P256Error::InvalidHex("invalid character")),
    }
}

// -----------------------------------------------------------------------
// 秘密鍵バリデーション: 1 ≤ d < n
// -----------------------------------------------------------------------

fn validate_secret_key(bytes: &[u8; 32]) -> bool {
    let d = U256::from_be_bytes(*bytes);
    if U256::is_zero(d) {
        return false;
    }
    matches!(U256::cmp(d, order_u256()), core::cmp::Ordering::Less)
}

// -----------------------------------------------------------------------
// 公開 API
// -----------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretKey {
    bytes: [u8; 32],
}

impl SecretKey {
    pub fn generate() -> Result<Self, P256Error> {
        let n = order_u256();
        loop {
            let mut buf = [0u8; 32];
            getrandom::getrandom(&mut buf).map_err(|_| P256Error::InvalidSecretKey)?;
            let k = U256::from_be_bytes(buf);
            if !U256::is_zero(k) {
                if let core::cmp::Ordering::Less = U256::cmp(k, n) {
                    return Ok(Self { bytes: buf });
                }
            }
        }
    }

    pub fn from_hex(hex: &str) -> Result<Self, P256Error> {
        let bytes = hex_decode(hex)?;
        if bytes.len() != 32 {
            return Err(P256Error::InvalidHex("expected 64 hex characters"));
        }

        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Self::from_bytes(arr)
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, P256Error> {
        if !validate_secret_key(&bytes) {
            return Err(P256Error::InvalidSecretKey);
        }
        Ok(Self { bytes })
    }

    pub fn to_hex(&self) -> String {
        hex_encode(&self.bytes)
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.bytes
    }

    pub fn public_key(&self) -> Result<PublicKey, P256Error> {
        let d = U256::from_be_bytes(self.bytes);
        let g = AffinePoint {
            x: generator_x(),
            y: generator_y(),
        };
        let q_proj = scalar_mul(d, g);
        let q = to_affine(q_proj).ok_or(P256Error::InvalidSecretKey)?;
        let sec1_bytes = encode_sec1_compressed(q);
        Ok(PublicKey { sec1_bytes })
    }

    pub fn sign_ecdsa_prehash(&self, digest32: [u8; 32]) -> Result<EcdsaSignature, P256Error> {
        let sig_bytes = ecdsa_sign(&self.bytes, &digest32).ok_or(P256Error::InvalidSignature)?;
        Ok(EcdsaSignature { bytes: sig_bytes })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicKey {
    sec1_bytes: [u8; 33],
}

impl PublicKey {
    pub fn from_hex(hex: &str) -> Result<Self, P256Error> {
        let bytes = hex_decode(hex)?;
        if bytes.len() != 33 {
            return Err(P256Error::InvalidHex("expected 66 hex characters"));
        }
        Self::from_sec1_bytes(&bytes)
    }

    pub fn from_sec1_bytes(bytes: &[u8]) -> Result<Self, P256Error> {
        if bytes.len() != 33 {
            return Err(P256Error::InvalidPublicKey);
        }
        let point = decode_sec1_compressed(bytes).ok_or(P256Error::InvalidPublicKey)?;
        let sec1_bytes = encode_sec1_compressed(point);
        Ok(Self { sec1_bytes })
    }

    pub fn to_hex(&self) -> String {
        hex_encode(&self.sec1_bytes)
    }

    pub fn to_sec1_bytes(&self) -> [u8; 33] {
        self.sec1_bytes
    }

    pub fn verify_ecdsa_prehash(
        &self,
        digest32: [u8; 32],
        sig: &EcdsaSignature,
    ) -> Result<(), P256Error> {
        if ecdsa_verify(&self.sec1_bytes, &digest32, &sig.bytes) {
            Ok(())
        } else {
            Err(P256Error::InvalidSignature)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcdsaSignature {
    bytes: [u8; 64],
}

impl EcdsaSignature {
    pub fn from_hex(hex: &str) -> Result<Self, P256Error> {
        let bytes = hex_decode(hex)?;
        if bytes.len() != 64 {
            return Err(P256Error::InvalidHex("expected 128 hex characters"));
        }

        let mut arr = [0u8; 64];
        arr.copy_from_slice(&bytes);
        Ok(Self::from_bytes(arr))
    }

    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        Self { bytes }
    }

    pub fn to_hex(&self) -> String {
        hex_encode(&self.bytes)
    }

    pub fn to_bytes(&self) -> [u8; 64] {
        self.bytes
    }
}

// -----------------------------------------------------------------------
// ユニットテスト
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_to_bytes32(s: &str) -> [u8; 32] {
        let mut out = [0u8; 32];
        for i in 0..32 {
            out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap();
        }
        out
    }

    #[test]
    fn generator_on_curve() {
        let g = AffinePoint {
            x: generator_x(),
            y: generator_y(),
        };
        assert!(is_on_curve(g), "generator G must be on P-256 curve");
    }

    #[test]
    fn secret_key_validation() {
        assert!(SecretKey::from_bytes([0u8; 32]).is_err());
        let n_bytes = order_u256().to_be_bytes();
        assert!(SecretKey::from_bytes(n_bytes).is_err());
        let (n_minus_1, _) = U256::sub(order_u256(), U256::ONE);
        assert!(SecretKey::from_bytes(n_minus_1.to_be_bytes()).is_ok());
    }

    #[test]
    fn public_key_derivation_k1() {
        // k=1 → Q = G の x 座標が一致する
        let mut sk_bytes = [0u8; 32];
        sk_bytes[31] = 1;
        let sk = SecretKey::from_bytes(sk_bytes).unwrap();
        let pk = sk.public_key().unwrap();
        let sec1 = pk.to_sec1_bytes();
        let gx = generator_x().to_u256().to_be_bytes();
        assert_eq!(&sec1[1..], &gx, "1*G should have generator x");
    }

    #[test]
    fn sign_verify_roundtrip() {
        let sk = SecretKey::generate().unwrap();
        let pk = sk.public_key().unwrap();
        let digest = [0x42u8; 32];
        let sig = sk.sign_ecdsa_prehash(digest).unwrap();
        pk.verify_ecdsa_prehash(digest, &sig).unwrap();
    }

    #[test]
    fn verify_wrong_message_fails() {
        let sk = SecretKey::generate().unwrap();
        let pk = sk.public_key().unwrap();
        let sig = sk.sign_ecdsa_prehash([0x11; 32]).unwrap();
        assert!(pk.verify_ecdsa_prehash([0x22; 32], &sig).is_err());
    }

    #[test]
    fn low_s_normalization() {
        let sk = SecretKey::generate().unwrap();
        let digest = [0x77u8; 32];
        let sig = sk.sign_ecdsa_prehash(digest).unwrap();
        let s = U256::from_be_bytes(sig.to_bytes()[32..].try_into().unwrap());
        let n = order_u256();
        let half_n = U256::shr1(n);
        assert!(
            matches!(
                U256::cmp(s, half_n),
                core::cmp::Ordering::Less | core::cmp::Ordering::Equal
            ),
            "s must be <= n/2"
        );
    }

    #[test]
    fn high_s_rejected_in_verify() {
        let sk = SecretKey::generate().unwrap();
        let pk = sk.public_key().unwrap();
        let digest = [0x77u8; 32];
        let sig = sk.sign_ecdsa_prehash(digest).unwrap();

        // s の high-s 相当 = n - s を作成
        let mut bytes = sig.to_bytes();
        let s = U256::from_be_bytes(bytes[32..].try_into().unwrap());
        let n = order_u256();
        let (high_s, _) = U256::sub(n, s);
        bytes[32..].copy_from_slice(&high_s.to_be_bytes());

        let high_s_sig = EcdsaSignature::from_bytes(bytes);
        let half_n = U256::shr1(n);
        // high_s > half_n の場合のみ拒否
        if let core::cmp::Ordering::Greater = U256::cmp(high_s, half_n) {
            assert!(
                pk.verify_ecdsa_prehash(digest, &high_s_sig).is_err(),
                "high-S should be rejected"
            );
        }
    }

    #[test]
    fn hex_roundtrip() {
        let sk = SecretKey::generate().unwrap();
        let pk = sk.public_key().unwrap();
        let sig = sk.sign_ecdsa_prehash([0x55; 32]).unwrap();

        assert_eq!(SecretKey::from_hex(&sk.to_hex()).unwrap(), sk);
        assert_eq!(PublicKey::from_hex(&pk.to_hex()).unwrap(), pk);
        assert_eq!(EcdsaSignature::from_hex(&sig.to_hex()).unwrap(), sig);
    }

    #[test]
    fn deterministic_signatures() {
        // 同じ秘密鍵 + ダイジェストで同じ署名 (RFC 6979)
        let sk_bytes =
            hex_to_bytes32("c9afa9d845ba75166b5c215767b1d6934e50c3db36e89b127b8a622b120f6721");
        let sk = SecretKey::from_bytes(sk_bytes).unwrap();
        let digest = [0x42u8; 32];
        let sig1 = sk.sign_ecdsa_prehash(digest).unwrap();
        let sig2 = sk.sign_ecdsa_prehash(digest).unwrap();
        assert_eq!(sig1.to_bytes(), sig2.to_bytes());
    }

    #[test]
    fn point_double_equals_point_add_self() {
        // 2*G = G + G
        let g = AffinePoint {
            x: generator_x(),
            y: generator_y(),
        };
        let g_proj = ProjectivePoint::from_affine(g);
        let two_g_double = to_affine(point_double(g_proj)).unwrap();
        let two_g_add = to_affine(point_add(g_proj, g_proj)).unwrap();
        assert_eq!(two_g_double.x.to_u256(), two_g_add.x.to_u256());
        assert_eq!(two_g_double.y.to_u256(), two_g_add.y.to_u256());
    }

    #[test]
    fn sec1_roundtrip() {
        let sk = SecretKey::generate().unwrap();
        let pk = sk.public_key().unwrap();
        let sec1 = pk.to_sec1_bytes();
        let pk2 = PublicKey::from_sec1_bytes(&sec1).unwrap();
        assert_eq!(pk, pk2);
    }
}
