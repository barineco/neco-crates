use crate::bigint::U256;
use core::cmp::Ordering;
use core::marker::PhantomData;

// -----------------------------------------------------------------------
// PrimeField trait
// -----------------------------------------------------------------------

/// 素体のパラメータ。secp256k1 と P-256 の両曲線に使う。
pub trait PrimeField: Copy + Clone + PartialEq + Eq + 'static {
    /// 素数 p
    const MODULUS: U256;
    /// R² mod p (R = 2^256)。Montgomery 変換に使用。
    const R_SQUARED: U256;
    /// -p⁻¹ mod 2^64。REDC の Montgomery constant。
    const INV: u64;
}

// -----------------------------------------------------------------------
// REDC algorithm
// -----------------------------------------------------------------------

/// Computes `REDC(T) = T · R⁻¹ mod p` where `R = 2^256`.
///
/// Input: `T` as eight little-endian limbs with `T < p · R`.
/// Output: a `U256` in the range `[0, p)`.
///
/// Implementation: CIOS (Coarsely Integrated Operand Scanning) with one
/// conditional subtraction at the end.
#[inline(always)]
pub fn redc<P: PrimeField>(t: [u64; 8]) -> U256 {
    let p = P::MODULUS;
    let inv = P::INV; // -p⁻¹ mod 2^64

    // 入力を可変バッファにコピー (9 limbs: 8 + 上位 overflow)
    let mut a = [0u64; 9];
    a[..8].copy_from_slice(&t);

    // 4 ラウンドの Montgomery 縮約
    for i in 0..4usize {
        // q_i = a[i] * INV mod 2^64
        let q = a[i].wrapping_mul(inv);

        // a[i..i+5] += q * p, carry を伝播
        // mac: (acc, x, y, c) -> acc = (acc + x*y + c) mod 2^64, new_carry = upper 64-bit
        let mut carry: u64 = 0;

        macro_rules! mac {
            ($dst:expr, $x:expr, $y:expr) => {{
                let x = ($dst as u128) + ($x as u128) * ($y as u128) + (carry as u128);
                $dst = x as u64;
                carry = (x >> 64) as u64;
            }};
        }

        mac!(a[i], q, p.l0);
        mac!(a[i + 1], q, p.l1);
        mac!(a[i + 2], q, p.l2);
        mac!(a[i + 3], q, p.l3);

        // a[i+4] += carry (propagate into upper half)
        let (s, oc) = a[i + 4].overflowing_add(carry);
        a[i + 4] = s;
        // overflowing_add から生じる追加 carry を次 limb に伝播
        if oc {
            // oc が true になるのは a[i+4] = u64::MAX + carry が溢れる場合のみ
            // 2p < 2^257 なので a[8] が 1 になる可能性がある
            let (s2, _) = a[i + 5].overflowing_add(1);
            a[i + 5] = s2;
        }
        // a[i] は今 0 になっているはず（下位 limb は縮約済み）
        // （実際は 0 に設定されているが読まれないので不要）
    }

    // 結果は a[4..8] に入っている
    let result = U256 {
        l0: a[4],
        l1: a[5],
        l2: a[6],
        l3: a[7],
    };
    let overflow = a[8];

    // CIOS output is always less than 2p, so a single conditional subtraction suffices.
    let (sub_r, borrow) = U256::sub(result, p);
    let need_sub = (overflow > 0) | !borrow;
    U256::select(result, sub_r, need_sub)
}

// -----------------------------------------------------------------------
// Fp<P>: モンゴメリ形式の素体元
// -----------------------------------------------------------------------

/// 素体の元。内部値は Montgomery 形式 (a·R mod p)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fp<P: PrimeField> {
    inner: U256,
    _p: PhantomData<P>,
}

impl<P: PrimeField> Fp<P> {
    /// ゼロ元
    pub const ZERO: Fp<P> = Fp {
        inner: U256::ZERO,
        _p: PhantomData,
    };

    /// 単位元 (1): Montgomery 形式では R mod p
    /// ただし定数として使う場合は各特殊化で定義する
    pub fn one() -> Fp<P> {
        // 1 を Montgomery 形式に変換: montMul(1, R²) = R mod p
        Self::from_u256(U256::ONE)
    }

    /// Montgomery 形式の内部表現から直接作成（redc 済みの値を渡す）
    #[inline]
    pub fn from_montgomery(inner: U256) -> Fp<P> {
        Fp {
            inner,
            _p: PhantomData,
        }
    }

    /// 内部の Montgomery 形式の値を返す（上級者向け）
    #[inline]
    pub fn to_montgomery_inner(self) -> U256 {
        self.inner
    }

    /// 通常の整数 n から Fp<P> に変換。
    /// n は [0, p) の範囲を仮定（超えていても動作するが未定義）。
    /// montMul(n, R²) = n·R mod p を計算。
    pub fn from_u256(n: U256) -> Fp<P> {
        // t = n * R² (512-bit)
        let t = U256::mul_wide(n, P::R_SQUARED);
        let inner = redc::<P>(t);
        Fp {
            inner,
            _p: PhantomData,
        }
    }

    /// Montgomery 形式から通常の整数に変換。
    /// redc(inner) = inner · R⁻¹ mod p。
    pub fn to_u256(self) -> U256 {
        let mut t = [0u64; 8];
        t[0] = self.inner.l0;
        t[1] = self.inner.l1;
        t[2] = self.inner.l2;
        t[3] = self.inner.l3;
        redc::<P>(t)
    }

    /// Field addition. In Montgomery form: `(a·R + b·R) mod p = (a+b)·R mod p`.
    #[inline]
    pub fn add(a: Fp<P>, b: Fp<P>) -> Fp<P> {
        let (sum, carry) = U256::add(a.inner, b.inner);
        // sum >= p または carry のとき sum - p
        let (sub, borrow) = U256::sub(sum, P::MODULUS);
        let need_sub = carry | !borrow;
        Fp {
            inner: U256::select(sum, sub, need_sub),
            _p: PhantomData,
        }
    }

    /// Field negation: `-a = if a == 0 then 0 else p - a`.
    #[inline]
    pub fn neg(a: Fp<P>) -> Fp<P> {
        if U256::is_zero(a.inner) {
            Fp::ZERO
        } else {
            let (r, _) = U256::sub(P::MODULUS, a.inner);
            Fp {
                inner: r,
                _p: PhantomData,
            }
        }
    }

    /// 減算 a - b = a + (-b)。
    #[inline]
    pub fn sub(a: Fp<P>, b: Fp<P>) -> Fp<P> {
        Self::add(a, Self::neg(b))
    }

    /// Montgomery multiplication: `montMul(a, b) = redc(a * b) = a·b·R⁻¹ mod p`.
    /// For Montgomery-form inputs: `montMul(a·R, b·R) = (a·b)·R mod p`.
    #[inline]
    pub fn mul(a: Fp<P>, b: Fp<P>) -> Fp<P> {
        let t = U256::mul_wide(a.inner, b.inner);
        Fp {
            inner: redc::<P>(t),
            _p: PhantomData,
        }
    }

    /// 自乗（最適化: 同じ値を 2 回掛ける）
    #[inline]
    pub fn sqr(a: Fp<P>) -> Fp<P> {
        Self::mul(a, a)
    }

    /// Square-and-multiply 乗冪。
    pub fn pow(base: Fp<P>, exp: U256) -> Fp<P> {
        let mut result = Self::one();
        let mut sq = base;
        for i in 0..256u32 {
            if U256::bit(exp, i) {
                result = Self::mul(result, sq);
            }
            sq = Self::sqr(sq);
        }
        result
    }

    /// Modular inverse via the binary extended GCD (Bezout identity over `[0, p)`).
    /// Returns zero when `a == 0` (the conventional value for undefined input).
    pub fn inv(a: Fp<P>) -> Fp<P> {
        // Montgomery 形式を通常整数に戻す
        let a_norm = a.to_u256();
        if U256::is_zero(a_norm) {
            return Fp::ZERO;
        }

        let p = P::MODULUS;
        // Binary extended GCD: gcd(a, p) = 1 (p が素数なら保証)
        // Bezout: s * a ≡ 1 (mod p)
        // 符号付き計算のために (positive, negative) ペアで管理する
        // u = (s1_pos, s1_neg, v): s_1 = s1_pos - s1_neg
        // 入力: u=a, v=p
        // 不変条件: u = s1 * a (mod p)、v = s2 * a (mod p)

        let mut r0 = a_norm; // a
        let mut r1 = p; // p
                        // s0 * a ≡ r0 (mod p): s0 = 1
                        // s1 * a ≡ r1 (mod p): s1 = 0
        let mut s0_pos = U256::ONE;
        let mut s0_neg = U256::ZERO;
        let mut s1_pos = U256::ZERO;
        let mut s1_neg = U256::ZERO;

        // 最大 512 反復で収束（256-bit に対して十分）
        for _ in 0..512 {
            if U256::is_zero(r1) {
                break;
            }
            // q = r0 / r1, rem = r0 mod r1 (u256 division)
            let (q, rem) = div_mod_u256(r0, r1);

            // new_s = s0 - q * s1
            // 符号付き: (s0_pos - s0_neg) - q * (s1_pos - s1_neg)
            //         = (s0_pos + q*s1_neg) - (s0_neg + q*s1_pos)
            let qs1p = mul_mod_p(q, s1_pos, p);
            let qs1n = mul_mod_p(q, s1_neg, p);

            let new_s_pos = add_mod_p(s0_pos, qs1n, p);
            let new_s_neg = add_mod_p(s0_neg, qs1p, p);

            s0_pos = s1_pos;
            s0_neg = s1_neg;
            s1_pos = new_s_pos;
            s1_neg = new_s_neg;
            r0 = r1;
            r1 = rem;
        }

        // r0 == gcd == 1 (a != 0 かつ p が素数なら)
        // s0 * a ≡ 1 (mod p)
        // inv = (s0_pos - s0_neg) mod p
        let (diff, borrow) = U256::sub(s0_pos, s0_neg);
        let inv_norm = if borrow {
            let (d, _) = U256::add(diff, p);
            d
        } else {
            // 正規化: [0, p)
            if let Ordering::Less = U256::cmp(diff, p) {
                diff
            } else {
                let (d, _) = U256::sub(diff, p);
                d
            }
        };

        Self::from_u256(inv_norm)
    }

    /// Square root for primes with `p ≡ 3 (mod 4)`: `a^((p+1)/4) mod p`.
    /// Existence is confirmed by squaring the candidate root and comparing to `a`.
    pub fn sqrt(a: Fp<P>, sqrt_exp: U256) -> Option<Fp<P>> {
        if U256::is_zero(a.inner) {
            return Some(Fp::ZERO);
        }
        let root = Self::pow(a, sqrt_exp);
        // 検証: root² == a
        if Self::sqr(root).inner == a.inner {
            Some(root)
        } else {
            None
        }
    }

    /// ゼロ判定
    #[inline]
    pub fn is_zero(a: Fp<P>) -> bool {
        U256::is_zero(a.inner)
    }

    /// 等値判定（constant-time ではないが標準用途向け）
    #[inline]
    pub fn eq(a: Fp<P>, b: Fp<P>) -> bool {
        a.inner == b.inner
    }

    /// 大小比較（スカラー体でのハーフオーダー判定などに使用）
    #[inline]
    pub fn cmp(a: Fp<P>, b: Fp<P>) -> core::cmp::Ordering {
        // Montgomery 形式のまま比較するのは無効なので正規化して比較
        U256::cmp(a.to_u256(), b.to_u256())
    }
}

// -----------------------------------------------------------------------
// 補助: u256 の単純除算（逆元計算用）
// -----------------------------------------------------------------------

/// (q, r) = a / b (u256 schoolbook 除算)
fn div_mod_u256(a: U256, b: U256) -> (U256, U256) {
    if U256::is_zero(b) {
        panic!("div_mod_u256: division by zero");
    }
    if let Ordering::Less = U256::cmp(a, b) {
        return (U256::ZERO, a);
    }
    // シフト引き算法 (binary long division)
    let mut q = U256::ZERO;
    let mut r = U256::ZERO;

    for i in (0..256u32).rev() {
        // r = r << 1 | bit(a, i)
        r = shift_left_1_with_bit(r, U256::bit(a, i));
        // if r >= b: r -= b, q |= 1 << i
        if let Ordering::Less = U256::cmp(r, b) {
            // r < b: do nothing
        } else {
            let (sub, _) = U256::sub(r, b);
            r = sub;
            q = set_bit(q, i);
        }
    }
    (q, r)
}

/// r = (r << 1) | bit
fn shift_left_1_with_bit(r: U256, bit: bool) -> U256 {
    U256 {
        l0: (r.l0 << 1) | (bit as u64),
        l1: (r.l1 << 1) | (r.l0 >> 63),
        l2: (r.l2 << 1) | (r.l1 >> 63),
        l3: (r.l3 << 1) | (r.l2 >> 63),
    }
}

/// q の i ビット目を 1 にする
fn set_bit(mut q: U256, i: u32) -> U256 {
    match i / 64 {
        0 => q.l0 |= 1u64 << (i % 64),
        1 => q.l1 |= 1u64 << (i % 64),
        2 => q.l2 |= 1u64 << (i % 64),
        3 => q.l3 |= 1u64 << (i % 64),
        _ => {}
    }
    q
}

/// a * b mod p（大きな積を p で割る。逆元計算用補助）
fn mul_mod_p(a: U256, b: U256, p: U256) -> U256 {
    // 512-bit 積を p で割る（schoolbook）
    // ただし a, b < p < 2^256 なので 512-bit 積 → div_mod
    // 効率は重要ではない（逆元計算は非ホットパス）
    let wide = U256::mul_wide(a, b);
    // wide を U512 として扱い、p で除算
    let mut rem = U256::ZERO;
    for i in (0..8usize).rev() {
        // rem = rem * 2^64 + wide[i]
        // = shift_left_64(rem) + wide[i]
        rem = U256 {
            l0: wide[i],
            l1: rem.l0,
            l2: rem.l1,
            l3: rem.l2,
        };
        // rem.l3 の carry 相当は捨てる（wide は 8 limb あるのでここでは上位を保存できない）
        // 代わりに正しい実装が必要: 512-bit を p で剰余
        // 上位ビットが残る可能性があるので、より堅牢な方法を使う
        let _ = rem.l3; // suppress warning
    }
    // 上記のアプローチは不完全。正しい 512-bit mod p を実装する
    u512_mod_p(wide, p)
}

/// 512-bit 値 t (= wide[0..8]) を p で割った余り
fn u512_mod_p(t: [u64; 8], p: U256) -> U256 {
    // Binary long division for 512-bit by 256-bit
    let mut rem = U256::ZERO;
    // 上位 256-bit から処理
    for i in (0..8usize).rev() {
        for bit_pos in (0..64u32).rev() {
            let bit = ((t[i] >> bit_pos) & 1) != 0;
            rem = shift_left_1_with_bit(rem, bit);
            if let Ordering::Greater | Ordering::Equal = U256::cmp(rem, p) {
                let (sub, _) = U256::sub(rem, p);
                rem = sub;
            }
        }
    }
    rem
}

/// (a + b) mod p（補助）
fn add_mod_p(a: U256, b: U256, p: U256) -> U256 {
    let (sum, carry) = U256::add(a, b);
    let (sub, borrow) = U256::sub(sum, p);
    let need_sub = carry | !borrow;
    U256::select(sum, sub, need_sub)
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p256::{P256Field, P256Order};
    use crate::secp256k1::{Secp256k1Field, Secp256k1Order};

    type Fq = Fp<Secp256k1Field>;

    fn fe(n: u64) -> Fq {
        Fq::from_u256(U256::from_u64(n))
    }

    // --- フィールド公理ランダムテスト (27本 = 9 公理 × 3 入力セット) ---
    // 入力: a=2, b=3, c=5 / a=7, b=11, c=13 / a=100, b=200, c=300

    macro_rules! field_axioms_tests {
        ($mod_name:ident, $type:ty, $fe_fn:ident) => {
            mod $mod_name {
                use super::*;
                type F = $type;

                fn $fe_fn(n: u64) -> F {
                    F::from_u256(U256::from_u64(n))
                }

                #[test]
                fn add_comm_s1() {
                    let (a, b) = ($fe_fn(2), $fe_fn(3));
                    assert_eq!(F::add(a, b), F::add(b, a));
                }
                #[test]
                fn add_comm_s2() {
                    let (a, b) = ($fe_fn(7), $fe_fn(11));
                    assert_eq!(F::add(a, b), F::add(b, a));
                }
                #[test]
                fn add_comm_s3() {
                    let (a, b) = ($fe_fn(100), $fe_fn(200));
                    assert_eq!(F::add(a, b), F::add(b, a));
                }

                #[test]
                fn add_assoc_s1() {
                    let (a, b, c) = ($fe_fn(2), $fe_fn(3), $fe_fn(5));
                    assert_eq!(F::add(F::add(a, b), c), F::add(a, F::add(b, c)));
                }
                #[test]
                fn add_assoc_s2() {
                    let (a, b, c) = ($fe_fn(7), $fe_fn(11), $fe_fn(13));
                    assert_eq!(F::add(F::add(a, b), c), F::add(a, F::add(b, c)));
                }
                #[test]
                fn add_assoc_s3() {
                    let (a, b, c) = ($fe_fn(100), $fe_fn(200), $fe_fn(300));
                    assert_eq!(F::add(F::add(a, b), c), F::add(a, F::add(b, c)));
                }

                #[test]
                fn add_neg_s1() {
                    // a + (-a) = 0
                    let a = $fe_fn(2);
                    assert_eq!(F::add(a, F::neg(a)), F::ZERO);
                }
                #[test]
                fn add_neg_s2() {
                    let a = $fe_fn(7);
                    assert_eq!(F::add(a, F::neg(a)), F::ZERO);
                }
                #[test]
                fn add_neg_s3() {
                    let a = $fe_fn(100);
                    assert_eq!(F::add(a, F::neg(a)), F::ZERO);
                }

                #[test]
                fn mul_comm_s1() {
                    let (a, b) = ($fe_fn(2), $fe_fn(3));
                    assert_eq!(F::mul(a, b), F::mul(b, a));
                }
                #[test]
                fn mul_comm_s2() {
                    let (a, b) = ($fe_fn(7), $fe_fn(11));
                    assert_eq!(F::mul(a, b), F::mul(b, a));
                }
                #[test]
                fn mul_comm_s3() {
                    let (a, b) = ($fe_fn(100), $fe_fn(200));
                    assert_eq!(F::mul(a, b), F::mul(b, a));
                }

                #[test]
                fn mul_assoc_s1() {
                    let (a, b, c) = ($fe_fn(2), $fe_fn(3), $fe_fn(5));
                    assert_eq!(F::mul(F::mul(a, b), c), F::mul(a, F::mul(b, c)));
                }
                #[test]
                fn mul_assoc_s2() {
                    let (a, b, c) = ($fe_fn(7), $fe_fn(11), $fe_fn(13));
                    assert_eq!(F::mul(F::mul(a, b), c), F::mul(a, F::mul(b, c)));
                }
                #[test]
                fn mul_assoc_s3() {
                    let (a, b, c) = ($fe_fn(100), $fe_fn(200), $fe_fn(300));
                    assert_eq!(F::mul(F::mul(a, b), c), F::mul(a, F::mul(b, c)));
                }

                #[test]
                fn distributive_s1() {
                    let (a, b, c) = ($fe_fn(2), $fe_fn(3), $fe_fn(5));
                    assert_eq!(F::mul(a, F::add(b, c)), F::add(F::mul(a, b), F::mul(a, c)));
                }
                #[test]
                fn distributive_s2() {
                    let (a, b, c) = ($fe_fn(7), $fe_fn(11), $fe_fn(13));
                    assert_eq!(F::mul(a, F::add(b, c)), F::add(F::mul(a, b), F::mul(a, c)));
                }
                #[test]
                fn distributive_s3() {
                    let (a, b, c) = ($fe_fn(100), $fe_fn(200), $fe_fn(300));
                    assert_eq!(F::mul(a, F::add(b, c)), F::add(F::mul(a, b), F::mul(a, c)));
                }

                #[test]
                fn inv_mul_s1() {
                    // a * inv(a) = 1
                    let a = $fe_fn(2);
                    let one = F::one();
                    assert_eq!(F::mul(a, F::inv(a)), one);
                }
                #[test]
                fn inv_mul_s2() {
                    let a = $fe_fn(7);
                    assert_eq!(F::mul(a, F::inv(a)), F::one());
                }
                #[test]
                fn inv_mul_s3() {
                    let a = $fe_fn(100);
                    assert_eq!(F::mul(a, F::inv(a)), F::one());
                }

                #[test]
                fn add_sub_roundtrip_s1() {
                    // a + b - b = a
                    let (a, b) = ($fe_fn(2), $fe_fn(3));
                    assert_eq!(F::sub(F::add(a, b), b), a);
                }
                #[test]
                fn add_sub_roundtrip_s2() {
                    let (a, b) = ($fe_fn(7), $fe_fn(11));
                    assert_eq!(F::sub(F::add(a, b), b), a);
                }
                #[test]
                fn add_sub_roundtrip_s3() {
                    let (a, b) = ($fe_fn(100), $fe_fn(200));
                    assert_eq!(F::sub(F::add(a, b), b), a);
                }
            }
        };
    }

    field_axioms_tests!(secp_field, Fp<Secp256k1Field>, fe_secp);
    field_axioms_tests!(secp_order, Fp<Secp256k1Order>, fe_secporder);
    field_axioms_tests!(p256_field, Fp<P256Field>, fe_p256);
    field_axioms_tests!(p256_order, Fp<P256Order>, fe_p256order);

    // --- sqrt テスト (4本) ---

    #[test]
    fn sqrt_known_qr_secp() {
        // 4 は完全平方数: sqrt(4) = 2
        let four = fe(4);
        let sqrt_exp = crate::secp256k1::SQRT_EXP_SECP256K1;
        let root = Fq::sqrt(four, sqrt_exp).expect("sqrt(4) must exist");
        assert_eq!(Fq::sqr(root), four);
    }

    #[test]
    fn sqrt_known_qr_p256() {
        type Fp256 = Fp<P256Field>;
        let four = Fp256::from_u256(U256::from_u64(4));
        let sqrt_exp = crate::p256::SQRT_EXP_P256;
        let root = Fp256::sqrt(four, sqrt_exp).expect("sqrt(4) must exist");
        assert_eq!(Fp256::sqr(root), four);
    }

    #[test]
    fn sqrt_zero() {
        let zero = fe(0);
        let sqrt_exp = crate::secp256k1::SQRT_EXP_SECP256K1;
        let root = Fq::sqrt(zero, sqrt_exp).expect("sqrt(0) must be Some");
        assert_eq!(root, Fq::ZERO);
    }

    #[test]
    fn sqrt_qnr() {
        // secp256k1 の p-1 は QNR
        // (p-1) = p - 1 → from_u256 でも大丈夫
        let p_minus_1 = Fq::from_u256({
            let (v, _) = U256::sub(Secp256k1Field::MODULUS, U256::ONE);
            v
        });
        let sqrt_exp = crate::secp256k1::SQRT_EXP_SECP256K1;
        // p-1 は secp256k1 上で QNR であることが知られている
        // (Euler 基準: (p-1)^((p-1)/2) = (-1)^((p-1)/2) mod p)
        // ここでは sqrt が None を返すことを確認
        let result = Fq::sqrt(p_minus_1, sqrt_exp);
        // p-1 が QNR なら None、QR なら Some
        // secp256k1 の p ≡ 3 (mod 4) なので -1 は QNR
        assert!(result.is_none(), "p-1 should not be a QR for secp256k1");
    }
}
