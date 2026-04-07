use crate::hex::{hex_decode, hex_encode};
use crate::{EcdsaSignature, SchnorrSignature, SecpError};

use neco_galois::generate_k;
use neco_galois::{Fp, PrimeField, Secp256k1Field, Secp256k1Order, SQRT_EXP_SECP256K1, U256};
use neco_sha2::Sha256;

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

// -----------------------------------------------------------------------
// 型エイリアス
// -----------------------------------------------------------------------

type FpField = Fp<Secp256k1Field>;
type Scalar = Fp<Secp256k1Order>;

// -----------------------------------------------------------------------
// secp256k1 曲線定数
// a = 0, b = 7, G = (Gx, Gy)
// -----------------------------------------------------------------------

fn curve_b() -> FpField {
    FpField::from_u256(U256::from_u64(7))
}

fn generator_x() -> FpField {
    FpField::from_u256(U256::from_be_bytes([
        0x79, 0xBE, 0x66, 0x7E, 0xF9, 0xDC, 0xBB, 0xAC, 0x55, 0xA0, 0x62, 0x95, 0xCE, 0x87, 0x0B,
        0x07, 0x02, 0x9B, 0xFC, 0xDB, 0x2D, 0xCE, 0x28, 0xD9, 0x59, 0xF2, 0x81, 0x5B, 0x16, 0xF8,
        0x17, 0x98,
    ]))
}

fn generator_y() -> FpField {
    FpField::from_u256(U256::from_be_bytes([
        0x48, 0x3A, 0xDA, 0x77, 0x26, 0xA3, 0xC4, 0x65, 0x5D, 0xA4, 0xFB, 0xFC, 0x0E, 0x11, 0x08,
        0xA8, 0xFD, 0x17, 0xB4, 0x48, 0xA6, 0x85, 0x54, 0x19, 0x9C, 0x47, 0xD0, 0x8F, 0xFB, 0x10,
        0xD4, 0xB8,
    ]))
}

fn generator() -> AffinePoint {
    AffinePoint {
        x: generator_x(),
        y: generator_y(),
    }
}

fn order_u256() -> U256 {
    Secp256k1Order::MODULUS
}

// -----------------------------------------------------------------------
// 点の表現
// -----------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AffinePoint {
    pub(crate) x: FpField,
    pub(crate) y: FpField,
}

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
// secp256k1 群演算
// -----------------------------------------------------------------------

/// 射影点の2倍算。secp256k1 は a=0 なので M = 3*X²。
fn point_double(p: ProjectivePoint) -> ProjectivePoint {
    if p.is_infinity {
        return ProjectivePoint::infinity();
    }

    let x = p.x;
    let y = p.y;
    let z = p.z;

    // a=0: M = 3*X²
    let x2 = FpField::sqr(x);
    let two_x2 = FpField::add(x2, x2);
    let m = FpField::add(two_x2, x2);

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

    let z1_sq = FpField::sqr(z1);
    let z2_sq = FpField::sqr(z2);
    let u1 = FpField::mul(x1, z2_sq);
    let u2 = FpField::mul(x2, z1_sq);

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

    let r2 = FpField::sqr(r);
    let two_u1h2 = FpField::add(u1h2, u1h2);
    let x3 = FpField::sub(FpField::sub(r2, h3), two_u1h2);

    let u1h2_minus_x3 = FpField::sub(u1h2, x3);
    let s1h3 = FpField::mul(s1, h3);
    let y3 = FpField::sub(FpField::mul(r, u1h2_minus_x3), s1h3);

    let z3 = FpField::mul(h, FpField::mul(z1, z2));

    ProjectivePoint {
        x: x3,
        y: y3,
        z: z3,
        is_infinity: false,
    }
}

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

/// y² = x³ + 7
fn is_on_curve(p: AffinePoint) -> bool {
    let y2 = FpField::sqr(p.y);
    let x3 = FpField::mul(FpField::sqr(p.x), p.x);
    let rhs = FpField::add(x3, curve_b());
    FpField::eq(y2, rhs)
}

// -----------------------------------------------------------------------
// SEC1 エンコード/デコード
// -----------------------------------------------------------------------

fn encode_sec1_compressed(p: AffinePoint) -> [u8; 33] {
    let x_bytes = p.x.to_u256().to_be_bytes();
    let y_val = p.y.to_u256();
    let prefix = if y_val.l0 & 1 == 1 { 0x03u8 } else { 0x02u8 };
    let mut out = [0u8; 33];
    out[0] = prefix;
    out[1..].copy_from_slice(&x_bytes);
    out
}

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

    if !matches!(
        U256::cmp(x_u256, Secp256k1Field::MODULUS),
        core::cmp::Ordering::Less
    ) {
        return None;
    }

    let x = FpField::from_u256(x_u256);
    let b = curve_b();

    // y² = x³ + 7
    let x3 = FpField::mul(FpField::sqr(x), x);
    let rhs = FpField::add(x3, b);

    let y = FpField::sqrt(rhs, SQRT_EXP_SECP256K1)?;
    let y_u256 = y.to_u256();

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

fn normalize_s(s: U256, n: U256) -> U256 {
    let half_n = U256::shr1(n);
    if let core::cmp::Ordering::Greater = U256::cmp(s, half_n) {
        let (ns, _) = U256::sub(n, s);
        ns
    } else {
        s
    }
}

fn ecdsa_sign(secret_bytes: &[u8; 32], digest: &[u8; 32]) -> Option<[u8; 64]> {
    let n = order_u256();
    let g = generator();

    let d = U256::from_be_bytes(*secret_bytes);

    let k = generate_k(secret_bytes, digest, &n);

    let r_proj = scalar_mul(k, g);
    let r_affine = to_affine(r_proj)?;

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

    let e_raw = U256::from_be_bytes(*digest);
    let e = if let core::cmp::Ordering::Less = U256::cmp(e_raw, n) {
        e_raw
    } else {
        let (v, _) = U256::sub(e_raw, n);
        v
    };

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

    let s = normalize_s(s, n);

    let mut out = [0u8; 64];
    out[..32].copy_from_slice(&r.to_be_bytes());
    out[32..].copy_from_slice(&s.to_be_bytes());
    Some(out)
}

fn ecdsa_verify(pubkey_sec1: &[u8; 33], digest: &[u8; 32], sig_bytes: &[u8; 64]) -> bool {
    let n = order_u256();
    let g = generator();

    let r = U256::from_be_bytes(sig_bytes[..32].try_into().unwrap());
    let s = U256::from_be_bytes(sig_bytes[32..].try_into().unwrap());

    if U256::is_zero(r) || U256::is_zero(s) {
        return false;
    }
    if !matches!(U256::cmp(r, n), core::cmp::Ordering::Less) {
        return false;
    }
    if !matches!(U256::cmp(s, n), core::cmp::Ordering::Less) {
        return false;
    }

    let half_n = U256::shr1(n);
    if let core::cmp::Ordering::Greater = U256::cmp(s, half_n) {
        return false;
    }

    let pubkey = match decode_sec1_compressed(pubkey_sec1) {
        Some(p) => p,
        None => return false,
    };

    let e_raw = U256::from_be_bytes(*digest);
    let e = if let core::cmp::Ordering::Less = U256::cmp(e_raw, n) {
        e_raw
    } else {
        let (v, _) = U256::sub(e_raw, n);
        v
    };

    let s_scalar = Scalar::from_u256(s);
    let w_scalar = Scalar::inv(s_scalar);

    let e_scalar = Scalar::from_u256(e);
    let r_scalar = Scalar::from_u256(r);

    let u1 = Scalar::mul(e_scalar, w_scalar).to_u256();
    let u2 = Scalar::mul(r_scalar, w_scalar).to_u256();

    let u1g = scalar_mul(u1, g);
    let u2q = scalar_mul(u2, pubkey);
    let r_point = point_add(u1g, u2q);

    let r_affine = match to_affine(r_point) {
        Some(p) => p,
        None => return false,
    };

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
// BIP340 Schnorr
// -----------------------------------------------------------------------

fn tagged_hash(tag: &[u8], data: &[u8]) -> [u8; 32] {
    let tag_hash = Sha256::digest(tag);
    let mut h = Sha256::new();
    h.update(&tag_hash);
    h.update(&tag_hash);
    h.update(data);
    h.finalize()
}

/// x 座標から偶数 y の点を復元 (BIP340 lift_x)
fn lift_x(x_bytes: &[u8; 32]) -> Option<AffinePoint> {
    let x_u256 = U256::from_be_bytes(*x_bytes);
    if !matches!(
        U256::cmp(x_u256, Secp256k1Field::MODULUS),
        core::cmp::Ordering::Less
    ) {
        return None;
    }

    let x = FpField::from_u256(x_u256);
    let b = curve_b();
    let x3 = FpField::mul(FpField::sqr(x), x);
    let rhs = FpField::add(x3, b);

    let y = FpField::sqrt(rhs, SQRT_EXP_SECP256K1)?;
    let y_u256 = y.to_u256();

    // BIP340: 偶数 y を選択
    let y_final = if y_u256.l0 & 1 == 1 {
        FpField::neg(y)
    } else {
        y
    };

    Some(AffinePoint { x, y: y_final })
}

fn schnorr_sign(secret: &[u8; 32], digest: &[u8; 32]) -> Option<[u8; 64]> {
    // BIP340 recommends fresh randomness for fault attack resistance
    let mut aux = [0u8; 32];
    let _ = getrandom::getrandom(&mut aux);
    schnorr_sign_with_aux(secret, digest, &aux)
}

fn schnorr_sign_with_aux(secret: &[u8; 32], digest: &[u8; 32], aux: &[u8; 32]) -> Option<[u8; 64]> {
    let n = order_u256();
    let d0 = U256::from_be_bytes(*secret);
    if U256::is_zero(d0) || !matches!(U256::cmp(d0, n), core::cmp::Ordering::Less) {
        return None;
    }

    let g = generator();
    let p_proj = scalar_mul(d0, g);
    let p_affine = to_affine(p_proj)?;

    // BIP340: y が奇数なら d = n - d0
    let py = p_affine.y.to_u256();
    let d = if py.l0 & 1 == 1 {
        let (neg, _) = U256::sub(n, d0);
        neg
    } else {
        d0
    };
    let px_bytes = p_affine.x.to_u256().to_be_bytes();

    let t = tagged_hash(b"BIP0340/aux", aux);
    // t XOR d
    let d_bytes = d.to_be_bytes();
    let mut rand = [0u8; 32];
    for i in 0..32 {
        rand[i] = d_bytes[i] ^ t[i];
    }

    // nonce = tagged_hash("BIP0340/nonce", rand || px || digest)
    let mut nonce_input = [0u8; 96];
    nonce_input[..32].copy_from_slice(&rand);
    nonce_input[32..64].copy_from_slice(&px_bytes);
    nonce_input[64..].copy_from_slice(digest);
    let nonce_hash = tagged_hash(b"BIP0340/nonce", &nonce_input);

    let k0 = U256::from_be_bytes(nonce_hash);
    // k0 mod n
    let k0 = if let core::cmp::Ordering::Less = U256::cmp(k0, n) {
        k0
    } else {
        let (v, _) = U256::sub(k0, n);
        v
    };
    if U256::is_zero(k0) {
        return None;
    }

    let r_proj = scalar_mul(k0, g);
    let r_affine = to_affine(r_proj)?;

    let ry = r_affine.y.to_u256();
    let k = if ry.l0 & 1 == 1 {
        let (neg, _) = U256::sub(n, k0);
        neg
    } else {
        k0
    };

    let rx_bytes = r_affine.x.to_u256().to_be_bytes();

    // e = tagged_hash("BIP0340/challenge", rx || px || digest) mod n
    let mut challenge_input = [0u8; 96];
    challenge_input[..32].copy_from_slice(&rx_bytes);
    challenge_input[32..64].copy_from_slice(&px_bytes);
    challenge_input[64..].copy_from_slice(digest);
    let e_hash = tagged_hash(b"BIP0340/challenge", &challenge_input);
    let e_raw = U256::from_be_bytes(e_hash);
    let e = if let core::cmp::Ordering::Less = U256::cmp(e_raw, n) {
        e_raw
    } else {
        let (v, _) = U256::sub(e_raw, n);
        v
    };

    // sig = (rx, k + e*d mod n)
    let e_scalar = Scalar::from_u256(e);
    let d_scalar = Scalar::from_u256(d);
    let k_scalar = Scalar::from_u256(k);
    let s = Scalar::add(k_scalar, Scalar::mul(e_scalar, d_scalar)).to_u256();

    let mut sig = [0u8; 64];
    sig[..32].copy_from_slice(&rx_bytes);
    sig[32..].copy_from_slice(&s.to_be_bytes());
    Some(sig)
}

fn schnorr_verify(pubkey_x: &[u8; 32], digest: &[u8; 32], sig: &[u8; 64]) -> bool {
    let n = order_u256();

    let rx_bytes: [u8; 32] = sig[..32].try_into().unwrap();
    let s_bytes: [u8; 32] = sig[32..].try_into().unwrap();

    let s = U256::from_be_bytes(s_bytes);
    if !matches!(U256::cmp(s, n), core::cmp::Ordering::Less) {
        return false;
    }

    let p = match lift_x(pubkey_x) {
        Some(p) => p,
        None => return false,
    };

    // e = tagged_hash("BIP0340/challenge", rx || px || digest) mod n
    let mut challenge_input = [0u8; 96];
    challenge_input[..32].copy_from_slice(&rx_bytes);
    challenge_input[32..64].copy_from_slice(pubkey_x);
    challenge_input[64..].copy_from_slice(digest);
    let e_hash = tagged_hash(b"BIP0340/challenge", &challenge_input);
    let e_raw = U256::from_be_bytes(e_hash);
    let e = if let core::cmp::Ordering::Less = U256::cmp(e_raw, n) {
        e_raw
    } else {
        let (v, _) = U256::sub(e_raw, n);
        v
    };

    let g = generator();

    // R = s*G - e*P
    let sg = scalar_mul(s, g);
    // -e*P = (n-e)*P
    let (neg_e, _) = U256::sub(n, e);
    let neg_ep = scalar_mul(neg_e, p);
    let r_proj = point_add(sg, neg_ep);

    let r_affine = match to_affine(r_proj) {
        Some(p) => p,
        None => return false,
    };

    // R.y must be even
    if r_affine.y.to_u256().l0 & 1 == 1 {
        return false;
    }

    // R.x must equal rx
    let rx = U256::from_be_bytes(rx_bytes);
    r_affine.x.to_u256() == rx
}

// -----------------------------------------------------------------------
// ECDH
// -----------------------------------------------------------------------

#[cfg(any(feature = "nip04", feature = "nip44"))]
pub(crate) fn ecdh_raw(secret: &[u8; 32], pubkey: AffinePoint) -> Option<[u8; 32]> {
    let d = U256::from_be_bytes(*secret);
    let n = order_u256();
    if U256::is_zero(d) || !matches!(U256::cmp(d, n), core::cmp::Ordering::Less) {
        return None;
    }
    let q = scalar_mul(d, pubkey);
    let q_affine = to_affine(q)?;
    Some(q_affine.x.to_u256().to_be_bytes())
}

// -----------------------------------------------------------------------
// 秘密鍵バリデーション
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
    pub(crate) bytes: [u8; 32],
}

impl SecretKey {
    pub fn generate() -> Result<Self, SecpError> {
        let n = order_u256();
        loop {
            let mut buf = [0u8; 32];
            getrandom::getrandom(&mut buf).map_err(|_| SecpError::InvalidSecretKey)?;
            let k = U256::from_be_bytes(buf);
            if !U256::is_zero(k) {
                if let core::cmp::Ordering::Less = U256::cmp(k, n) {
                    return Ok(Self { bytes: buf });
                }
            }
        }
    }

    pub fn from_hex(hex: &str) -> Result<Self, SecpError> {
        let bytes = hex_decode(hex)?;
        if bytes.len() != 32 {
            return Err(SecpError::InvalidHex("expected 64 hex characters"));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Self::from_bytes(arr)
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, SecpError> {
        if !validate_secret_key(&bytes) {
            return Err(SecpError::InvalidSecretKey);
        }
        Ok(Self { bytes })
    }

    pub fn to_hex(&self) -> String {
        hex_encode(&self.bytes)
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.bytes
    }

    pub fn public_key(&self) -> Result<PublicKey, SecpError> {
        let d = U256::from_be_bytes(self.bytes);
        let g = generator();
        let q_proj = scalar_mul(d, g);
        let q = to_affine(q_proj).ok_or(SecpError::InvalidSecretKey)?;
        let sec1_bytes = encode_sec1_compressed(q);
        Ok(PublicKey { sec1_bytes })
    }

    pub fn xonly_public_key(&self) -> Result<XOnlyPublicKey, SecpError> {
        let d = U256::from_be_bytes(self.bytes);
        let g = generator();
        let q_proj = scalar_mul(d, g);
        let q = to_affine(q_proj).ok_or(SecpError::InvalidSecretKey)?;
        let x_bytes = q.x.to_u256().to_be_bytes();
        Ok(XOnlyPublicKey { bytes: x_bytes })
    }

    pub fn sign_schnorr_prehash(&self, digest32: [u8; 32]) -> Result<SchnorrSignature, SecpError> {
        let sig_bytes = schnorr_sign(&self.bytes, &digest32).ok_or(SecpError::InvalidSignature)?;
        Ok(SchnorrSignature { bytes: sig_bytes })
    }

    /// aux_rand をゼロ固定にした決定的 BIP-340 Schnorr 署名。
    ///
    /// 同じ秘密鍵・同じ digest に対して常に同じ署名バイト列を返す。
    /// BIP-340 はフォールトアタック耐性のためフレッシュな aux_rand を推奨しており、
    /// 本 API はテスト互換や外部ツールとの fixture 照合用途に限って使用すること。
    /// 通常の署名用途では [`sign_schnorr_prehash`] を使うこと。
    pub fn sign_schnorr_prehash_deterministic(
        &self,
        digest32: [u8; 32],
    ) -> Result<SchnorrSignature, SecpError> {
        let zero_aux = [0u8; 32];
        let sig_bytes = schnorr_sign_with_aux(&self.bytes, &digest32, &zero_aux)
            .ok_or(SecpError::InvalidSignature)?;
        Ok(SchnorrSignature { bytes: sig_bytes })
    }

    pub fn sign_ecdsa_prehash(&self, digest32: [u8; 32]) -> Result<EcdsaSignature, SecpError> {
        let sig_bytes = ecdsa_sign(&self.bytes, &digest32).ok_or(SecpError::InvalidSignature)?;
        Ok(EcdsaSignature { bytes: sig_bytes })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicKey {
    sec1_bytes: [u8; 33],
}

impl PublicKey {
    pub fn from_hex(hex: &str) -> Result<Self, SecpError> {
        let bytes = hex_decode(hex)?;
        if bytes.len() != 33 {
            return Err(SecpError::InvalidHex("expected 66 hex characters"));
        }
        Self::from_sec1_bytes(&bytes)
    }

    pub fn from_sec1_bytes(bytes: &[u8]) -> Result<Self, SecpError> {
        if bytes.len() != 33 {
            return Err(SecpError::InvalidPublicKey);
        }
        let point = decode_sec1_compressed(bytes).ok_or(SecpError::InvalidPublicKey)?;
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
    ) -> Result<(), SecpError> {
        if ecdsa_verify(&self.sec1_bytes, &digest32, &sig.bytes) {
            Ok(())
        } else {
            Err(SecpError::InvalidSignature)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XOnlyPublicKey {
    pub(crate) bytes: [u8; 32],
}

impl XOnlyPublicKey {
    pub fn from_hex(hex: &str) -> Result<Self, SecpError> {
        let bytes = hex_decode(hex)?;
        if bytes.len() != 32 {
            return Err(SecpError::InvalidHex("expected 64 hex characters"));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Self::from_bytes(arr)
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, SecpError> {
        let _ = lift_x(&bytes).ok_or(SecpError::InvalidPublicKey)?;
        Ok(Self { bytes })
    }

    pub fn to_hex(&self) -> String {
        hex_encode(&self.bytes)
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.bytes
    }

    pub fn verify_schnorr_prehash(
        &self,
        digest32: [u8; 32],
        sig: &SchnorrSignature,
    ) -> Result<(), SecpError> {
        if schnorr_verify(&self.bytes, &digest32, &sig.bytes) {
            Ok(())
        } else {
            Err(SecpError::InvalidSignature)
        }
    }
}

#[cfg(feature = "serde")]
impl Serialize for XOnlyPublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for XOnlyPublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;
        Self::from_hex(&hex).map_err(serde::de::Error::custom)
    }
}

#[cfg(any(feature = "nip04", feature = "nip44"))]
pub(crate) fn decode_xonly_pubkey(pubkey: &XOnlyPublicKey) -> Result<AffinePoint, SecpError> {
    lift_x(&pubkey.bytes).ok_or(SecpError::InvalidPublicKey)
}
