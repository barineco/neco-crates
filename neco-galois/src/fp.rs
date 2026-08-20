use crate::bigint::U256;
use core::cmp::Ordering;
use core::marker::PhantomData;

/// Montgomery 形式の素体に必要なパラメータです。
pub trait PrimeField: Copy + Clone + PartialEq + Eq + 'static {
    /// 素数またはスカラー体の位数です。
    const MODULUS: U256;
    /// `R = 2^256` に対する `R² mod p` です。
    const R_SQUARED: U256;
    /// `-p⁻¹ mod 2^64` です。
    const INV: u64;
}

/// `R = 2^256` として `REDC(T) = T · R⁻¹ mod p` を返します。
/// 入力はリトルエンディアンの 8 limb で `T < p · R` を満たす必要があります。
/// 出力は CIOS 縮約と一回の条件付き減算による `[0, p)` の値です。
#[inline(always)]
pub fn redc<P: PrimeField>(t: [u64; 8]) -> U256 {
    let p = P::MODULUS;
    let inv = P::INV;
    let mut a = [0u64; 9];
    a[..8].copy_from_slice(&t);
    for i in 0..4usize {
        let q = a[i].wrapping_mul(inv);
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
        let (s, oc) = a[i + 4].overflowing_add(carry);
        a[i + 4] = s;
        if oc {
            let (s2, _) = a[i + 5].overflowing_add(1);
            a[i + 5] = s2;
        }
    }
    let result = U256 {
        l0: a[4],
        l1: a[5],
        l2: a[6],
        l3: a[7],
    };
    let overflow = a[8];
    let (sub_r, borrow) = U256::sub(result, p);
    let need_sub = (overflow > 0) | !borrow;
    U256::select(result, sub_r, need_sub)
}

/// Montgomery 形式で保持する素体の元です。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fp<P: PrimeField> {
    inner: U256,
    _p: PhantomData<P>,
}

impl<P: PrimeField> Fp<P> {
    /// 加法単位元です。
    pub const ZERO: Fp<P> = Fp {
        inner: U256::ZERO,
        _p: PhantomData,
    };

    /// 乗法単位元を Montgomery 形式で返します。
    pub fn one() -> Fp<P> {
        Self::from_u256(U256::ONE)
    }

    /// 縮約済みの Montgomery 形式の内部値から作成します。
    #[inline]
    pub fn from_montgomery(inner: U256) -> Fp<P> {
        Fp {
            inner,
            _p: PhantomData,
        }
    }

    /// 縮約済みの Montgomery 形式の内部値を返します。
    #[inline]
    pub fn to_montgomery_inner(self) -> U256 {
        self.inner
    }

    /// 通常の整数を Montgomery 形式へ変換します。
    /// 入力は `[0, p)` を想定しますが、現在は範囲外の入力も縮約した値を返します。
    pub fn from_u256(n: U256) -> Fp<P> {
        let t = U256::mul_wide(n, P::R_SQUARED);
        let inner = redc::<P>(t);
        Fp {
            inner,
            _p: PhantomData,
        }
    }

    /// Montgomery 形式の値を通常の整数へ変換します。
    pub fn to_u256(self) -> U256 {
        let mut t = [0u64; 8];
        t[0] = self.inner.l0;
        t[1] = self.inner.l1;
        t[2] = self.inner.l2;
        t[3] = self.inner.l3;
        redc::<P>(t)
    }

    /// 二つの Montgomery 形式の値を加算し、法による値を返します。
    #[inline]
    pub fn add(a: Fp<P>, b: Fp<P>) -> Fp<P> {
        let (sum, carry) = U256::add(a.inner, b.inner);
        let (sub, borrow) = U256::sub(sum, P::MODULUS);
        let need_sub = carry | !borrow;
        Fp {
            inner: U256::select(sum, sub, need_sub),
            _p: PhantomData,
        }
    }

    /// 加法逆元を返します。ゼロの逆元はゼロです。
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

    /// `a - b` を返します。
    #[inline]
    pub fn sub(a: Fp<P>, b: Fp<P>) -> Fp<P> {
        Self::add(a, Self::neg(b))
    }

    /// 二つの Montgomery 形式の値を乗算し、Montgomery 形式の積を返します。
    #[inline]
    pub fn mul(a: Fp<P>, b: Fp<P>) -> Fp<P> {
        let t = U256::mul_wide(a.inner, b.inner);
        Fp {
            inner: redc::<P>(t),
            _p: PhantomData,
        }
    }

    /// 自乗を返します。
    #[inline]
    pub fn sqr(a: Fp<P>) -> Fp<P> {
        Self::mul(a, a)
    }

    /// 非負の指数による乗冪を返します。
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

    /// 拡張ユークリッド互除法で乗法逆元を返します。ゼロにはゼロを返します。
    /// 係数は正負の組で保持し、最大 512 回で反復を停止します。
    pub fn inv(a: Fp<P>) -> Fp<P> {
        let a_norm = a.to_u256();
        if U256::is_zero(a_norm) {
            return Fp::ZERO;
        }

        let p = P::MODULUS;

        let mut r0 = a_norm;
        let mut r1 = p;
        let mut s0_pos = U256::ONE;
        let mut s0_neg = U256::ZERO;
        let mut s1_pos = U256::ZERO;
        let mut s1_neg = U256::ZERO;
        for _ in 0..512 {
            if U256::is_zero(r1) {
                break;
            }
            let (q, rem) = div_mod_u256(r0, r1);
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
        let (diff, borrow) = U256::sub(s0_pos, s0_neg);
        let inv_norm = if borrow {
            let (d, _) = U256::add(diff, p);
            d
        } else if let Ordering::Less = U256::cmp(diff, p) {
            diff
        } else {
            let (d, _) = U256::sub(diff, p);
            d
        };

        Self::from_u256(inv_norm)
    }

    /// `p ≡ 3 (mod 4)` の素数について平方根を返します。
    /// 候補を自乗して入力と比較し、平方根がないときは `None` を返します。
    pub fn sqrt(a: Fp<P>, sqrt_exp: U256) -> Option<Fp<P>> {
        if U256::is_zero(a.inner) {
            return Some(Fp::ZERO);
        }
        let root = Self::pow(a, sqrt_exp);
        if Self::sqr(root).inner == a.inner {
            Some(root)
        } else {
            None
        }
    }

    /// ゼロか判定します。
    #[inline]
    pub fn is_zero(a: Fp<P>) -> bool {
        U256::is_zero(a.inner)
    }

    /// 二つの Montgomery 形式の内部値が等しいか判定します。
    #[inline]
    pub fn eq(a: Fp<P>, b: Fp<P>) -> bool {
        a.inner == b.inner
    }

    /// 通常の整数へ正規化して大小を比較します。
    #[inline]
    pub fn cmp(a: Fp<P>, b: Fp<P>) -> core::cmp::Ordering {
        U256::cmp(a.to_u256(), b.to_u256())
    }
}

/// 商と余りを返します。除数がゼロの場合は停止します。
fn div_mod_u256(a: U256, b: U256) -> (U256, U256) {
    if U256::is_zero(b) {
        panic!("div_mod_u256: division by zero");
    }
    if let Ordering::Less = U256::cmp(a, b) {
        return (U256::ZERO, a);
    }
    let mut q = U256::ZERO;
    let mut r = U256::ZERO;

    for i in (0..256u32).rev() {
        r = shift_left_1_with_bit(r, U256::bit(a, i));
        if let Ordering::Less = U256::cmp(r, b) {
        } else {
            let (sub, _) = U256::sub(r, b);
            r = sub;
            q = set_bit(q, i);
        }
    }
    (q, r)
}

/// 最下位ビットを加えて左へ一ビットシフトします。
fn shift_left_1_with_bit(r: U256, bit: bool) -> U256 {
    U256 {
        l0: (r.l0 << 1) | (bit as u64),
        l1: (r.l1 << 1) | (r.l0 >> 63),
        l2: (r.l2 << 1) | (r.l1 >> 63),
        l3: (r.l3 << 1) | (r.l2 >> 63),
    }
}

/// 指定したビットを設定します。
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

/// `a * b mod p` を返します。途中の limb ごとの計算は結果に用いず、最終的に `u512_mod_p` を使用します。
fn mul_mod_p(a: U256, b: U256, p: U256) -> U256 {
    let wide = U256::mul_wide(a, b);
    let mut rem = U256::ZERO;
    for i in (0..8usize).rev() {
        rem = U256 {
            l0: wide[i],
            l1: rem.l0,
            l2: rem.l1,
            l3: rem.l2,
        };
        let _ = rem.l3;
    }
    u512_mod_p(wide, p)
}

/// 512 ビット値を法で縮約します。
fn u512_mod_p(t: [u64; 8], p: U256) -> U256 {
    let mut rem = U256::ZERO;
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

/// `a + b mod p` を返します。
fn add_mod_p(a: U256, b: U256, p: U256) -> U256 {
    let (sum, carry) = U256::add(a, b);
    let (sub, borrow) = U256::sub(sum, p);
    let need_sub = carry | !borrow;
    U256::select(sum, sub, need_sub)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p256::{P256Field, P256Order};
    use crate::secp256k1::{Secp256k1Field, Secp256k1Order};

    type Fq = Fp<Secp256k1Field>;

    fn fe(n: u64) -> Fq {
        Fq::from_u256(U256::from_u64(n))
    }

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

    #[test]
    fn sqrt_known_qr_secp() {
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
        let p_minus_1 = Fq::from_u256({
            let (v, _) = U256::sub(Secp256k1Field::MODULUS, U256::ONE);
            v
        });
        let sqrt_exp = crate::secp256k1::SQRT_EXP_SECP256K1;
        let result = Fq::sqrt(p_minus_1, sqrt_exp);
        assert!(result.is_none(), "p-1 should not be a QR for secp256k1");
    }
}
