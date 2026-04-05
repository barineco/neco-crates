use crate::scan::Scanner;
use crate::{KdlError, KdlErrorKind, KdlNumber};

/// 数値リテラルをパースする。数値でなければ None を返す。
/// keyword-number (`#inf`, `#-inf`, `#nan`) は別関数。
pub(crate) fn parse_number(s: &mut Scanner<'_>) -> Result<Option<KdlNumber>, KdlError> {
    let ch = match s.peek() {
        Some(ch) => ch,
        None => return Ok(None),
    };

    // 数値は digit または sign + digit / sign + 0x/0o/0b で始まる
    let is_sign = ch == '+' || ch == '-';
    let is_digit = ch.is_ascii_digit();

    if !is_sign && !is_digit {
        return Ok(None);
    }

    let saved = s.save();
    let start = s.byte_offset();

    // sign を消費
    let has_sign = if is_sign {
        s.advance();
        true
    } else {
        false
    };

    // sign の後に digit が来なければ数値ではない（identifier かもしれない）
    match s.peek() {
        Some(ch2) if ch2.is_ascii_digit() => {}
        Some('.') => {
            // `.1` は数値ではない（dotted-ident に当たる）
            s.restore(saved);
            return Ok(None);
        }
        _ if has_sign => {
            s.restore(saved);
            return Ok(None);
        }
        _ => {
            s.restore(saved);
            return Ok(None);
        }
    }

    // 0x, 0o, 0b プレフィックスチェック
    if s.peek() == Some('0') {
        match s.peek_next() {
            Some('x') => return parse_prefixed(s, start, 'x', is_hex_digit),
            Some('o') => return parse_prefixed(s, start, 'o', is_octal_digit),
            Some('b') => return parse_prefixed(s, start, 'b', is_binary_digit),
            _ => {}
        }
    }

    // decimal: integer ('.' integer)? exponent?
    consume_integer(s);

    // 小数部
    let has_dot = if s.peek() == Some('.') {
        // '.' の後に digit が必要（'_' は不可 — underscore_at_start_of_fraction_fail）
        match s.peek_next() {
            Some(ch) if ch.is_ascii_digit() => {
                s.advance(); // '.'
                consume_integer(s);
                true
            }
            _ => {
                // `1.` or `1._7` — 小数点はあるが小数部がない → エラー
                s.advance(); // '.' を消費して報告
                return Err(s.make_error(KdlErrorKind::InvalidNumber));
            }
        }
    } else {
        false
    };

    // 指数部
    let has_exp = if matches!(s.peek(), Some('e') | Some('E')) {
        s.advance(); // 'e'/'E'
        if matches!(s.peek(), Some('+') | Some('-')) {
            s.advance();
        }
        if !matches!(s.peek(), Some(ch) if ch.is_ascii_digit()) {
            return Err(s.make_error(KdlErrorKind::InvalidNumber));
        }
        consume_integer(s);
        // 指数部の後に再び 'e'/'E' が来たらエラー (multiple_es_in_float_fail)
        if matches!(s.peek(), Some('e') | Some('E')) {
            return Err(s.make_error(KdlErrorKind::InvalidNumber));
        }
        true
    } else {
        false
    };

    // 数値の後に identifier-char が続くのは不正 (0n, +0n, 0x10g10 等)
    if let Some(ch) = s.peek() {
        if crate::scan::is_identifier_char(ch) && ch != '.' {
            return Err(s.make_error(KdlErrorKind::InvalidNumber));
        }
    }

    let end = s.byte_offset();
    let raw = s.slice(start, end).to_string();

    let is_float = has_dot || has_exp;
    let (as_i64, as_f64) = interpret_decimal(&raw, is_float);

    Ok(Some(KdlNumber {
        raw,
        as_i64,
        as_f64,
    }))
}

/// keyword-number: `#inf`, `#-inf`, `#nan` をパースする。
/// `#` は未消費の前提。マッチしなければ None。
pub(crate) fn parse_keyword_number(s: &mut Scanner<'_>) -> Result<Option<KdlNumber>, KdlError> {
    if s.peek() != Some('#') {
        return Ok(None);
    }

    let saved = s.save();
    let start = s.byte_offset();
    s.advance(); // '#'

    // "#inf", "#-inf", "#nan"
    match s.peek() {
        Some('i') => {
            // #inf
            s.advance();
            if s.peek() == Some('n') {
                s.advance();
                if s.peek() == Some('f') {
                    s.advance();
                    let end = s.byte_offset();
                    let raw = s.slice(start, end).to_string();
                    return Ok(Some(KdlNumber {
                        raw,
                        as_i64: None,
                        as_f64: Some(f64::INFINITY),
                    }));
                }
            }
            s.restore(saved);
            Ok(None)
        }
        Some('-') => {
            // #-inf
            s.advance();
            if s.peek() == Some('i') {
                s.advance();
                if s.peek() == Some('n') {
                    s.advance();
                    if s.peek() == Some('f') {
                        s.advance();
                        let end = s.byte_offset();
                        let raw = s.slice(start, end).to_string();
                        return Ok(Some(KdlNumber {
                            raw,
                            as_i64: None,
                            as_f64: Some(f64::NEG_INFINITY),
                        }));
                    }
                }
            }
            s.restore(saved);
            Ok(None)
        }
        Some('n') => {
            // #nan
            s.advance();
            if s.peek() == Some('a') {
                s.advance();
                if s.peek() == Some('n') {
                    s.advance();
                    let end = s.byte_offset();
                    let raw = s.slice(start, end).to_string();
                    return Ok(Some(KdlNumber {
                        raw,
                        as_i64: None,
                        as_f64: Some(f64::NAN),
                    }));
                }
            }
            s.restore(saved);
            Ok(None)
        }
        _ => {
            s.restore(saved);
            Ok(None)
        }
    }
}

/// プレフィックス付き数値 (0x, 0o, 0b) をパースする。
fn parse_prefixed(
    s: &mut Scanner<'_>,
    start: usize,
    prefix: char,
    is_valid_digit: fn(char) -> bool,
) -> Result<Option<KdlNumber>, KdlError> {
    s.advance(); // '0'
    s.advance(); // 'x'/'o'/'b'

    // プレフィックスの直後に有効な digit が必要
    match s.peek() {
        Some(ch) if is_valid_digit(ch) => {
            s.advance();
        }
        _ => return Err(s.make_error(KdlErrorKind::InvalidNumber)),
    }

    // 残りの digit と underscore
    while let Some(ch) = s.peek() {
        if is_valid_digit(ch) || ch == '_' {
            s.advance();
        } else {
            break;
        }
    }

    // 数値の後に identifier-char が続くのは不正 (0x10g10 等)
    if let Some(ch) = s.peek() {
        if crate::scan::is_identifier_char(ch) {
            return Err(s.make_error(KdlErrorKind::InvalidNumber));
        }
    }

    let end = s.byte_offset();
    let raw = s.slice(start, end).to_string();

    // underscore を除去して解釈
    let clean: String = raw.chars().filter(|&c| c != '_').collect();
    let (as_i64, as_f64) = match prefix {
        'x' => interpret_hex(&clean),
        'o' => interpret_octal(&clean),
        'b' => interpret_binary(&clean),
        _ => unreachable!(),
    };

    Ok(Some(KdlNumber {
        raw,
        as_i64,
        as_f64,
    }))
}

/// integer := digit (digit | '_')* を消費する。
fn consume_integer(s: &mut Scanner<'_>) {
    while let Some(ch) = s.peek() {
        if ch.is_ascii_digit() || ch == '_' {
            s.advance();
        } else {
            break;
        }
    }
}

fn is_hex_digit(ch: char) -> bool {
    ch.is_ascii_hexdigit()
}

fn is_octal_digit(ch: char) -> bool {
    matches!(ch, '0'..='7')
}

fn is_binary_digit(ch: char) -> bool {
    ch == '0' || ch == '1'
}

/// decimal 文字列を i64/f64 に変換する。
fn interpret_decimal(raw: &str, is_float: bool) -> (Option<i64>, Option<f64>) {
    let clean: String = raw.chars().filter(|&c| c != '_').collect();
    if is_float {
        let f = clean.parse::<f64>().ok();
        (None, f)
    } else {
        let i = clean.parse::<i64>().ok();
        let f = clean.parse::<f64>().ok();
        (i, f)
    }
}

/// hex 文字列 ("+0x1a" 等) を i64/f64 に変換する。
fn interpret_hex(clean: &str) -> (Option<i64>, Option<f64>) {
    let (neg, hex_part) = strip_sign_and_prefix(clean, "0x");
    let val = u64::from_str_radix(hex_part, 16).ok();
    let i = val.and_then(|v| {
        if neg {
            (v as i128)
                .checked_neg()
                .and_then(|n| i64::try_from(n).ok())
        } else {
            i64::try_from(v).ok()
        }
    });
    let f = i.map(|v| v as f64);
    (i, f)
}

fn interpret_octal(clean: &str) -> (Option<i64>, Option<f64>) {
    let (neg, oct_part) = strip_sign_and_prefix(clean, "0o");
    let val = u64::from_str_radix(oct_part, 8).ok();
    let i = val.and_then(|v| {
        if neg {
            (v as i128)
                .checked_neg()
                .and_then(|n| i64::try_from(n).ok())
        } else {
            i64::try_from(v).ok()
        }
    });
    let f = i.map(|v| v as f64);
    (i, f)
}

fn interpret_binary(clean: &str) -> (Option<i64>, Option<f64>) {
    let (neg, bin_part) = strip_sign_and_prefix(clean, "0b");
    let val = u64::from_str_radix(bin_part, 2).ok();
    let i = val.and_then(|v| {
        if neg {
            (v as i128)
                .checked_neg()
                .and_then(|n| i64::try_from(n).ok())
        } else {
            i64::try_from(v).ok()
        }
    });
    let f = i.map(|v| v as f64);
    (i, f)
}

/// sign とプレフィックスを分離する。
fn strip_sign_and_prefix<'a>(s: &'a str, prefix: &str) -> (bool, &'a str) {
    let (neg, rest) = if let Some(r) = s.strip_prefix('-') {
        (true, r)
    } else if let Some(r) = s.strip_prefix('+') {
        (false, r)
    } else {
        (false, s)
    };
    let rest = rest.strip_prefix(prefix).unwrap_or(rest);
    (neg, rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(input: &str) -> KdlNumber {
        let mut s = Scanner::new(input);
        parse_number(&mut s).unwrap().unwrap()
    }

    fn kw_num(input: &str) -> KdlNumber {
        let mut s = Scanner::new(input);
        parse_keyword_number(&mut s).unwrap().unwrap()
    }

    #[test]
    fn decimal_integer() {
        let n = num("42 ");
        assert_eq!(n.raw, "42");
        assert_eq!(n.as_i64, Some(42));
    }

    #[test]
    fn decimal_with_underscore() {
        let n = num("1_000 ");
        assert_eq!(n.raw, "1_000");
        assert_eq!(n.as_i64, Some(1000));
    }

    #[test]
    fn decimal_float() {
        let n = num("1.23 ");
        assert_eq!(n.raw, "1.23");
        assert_eq!(n.as_i64, None);
        assert!((n.as_f64.unwrap() - 1.23_f64).abs() < f64::EPSILON);
    }

    #[test]
    fn decimal_exponent() {
        let n = num("1.5e2 ");
        assert_eq!(n.as_f64, Some(150.0));
    }

    #[test]
    fn decimal_exponent_no_dot() {
        let n = num("1e10 ");
        assert_eq!(n.as_f64, Some(1e10));
    }

    #[test]
    fn hex_number() {
        let n = num("0xff ");
        assert_eq!(n.as_i64, Some(255));
    }

    #[test]
    fn octal_number() {
        let n = num("0o77 ");
        assert_eq!(n.as_i64, Some(63));
    }

    #[test]
    fn binary_number() {
        let n = num("0b1010 ");
        assert_eq!(n.as_i64, Some(10));
    }

    #[test]
    fn signed_number() {
        let n = num("-42 ");
        assert_eq!(n.as_i64, Some(-42));

        let n = num("+42 ");
        assert_eq!(n.as_i64, Some(42));
    }

    #[test]
    fn signed_hex() {
        let n = num("-0xff ");
        assert_eq!(n.as_i64, Some(-255));
    }

    #[test]
    fn keyword_inf() {
        let n = kw_num("#inf");
        assert_eq!(n.raw, "#inf");
        assert_eq!(n.as_f64, Some(f64::INFINITY));
    }

    #[test]
    fn keyword_neg_inf() {
        let n = kw_num("#-inf");
        assert_eq!(n.raw, "#-inf");
        assert_eq!(n.as_f64, Some(f64::NEG_INFINITY));
    }

    #[test]
    fn keyword_nan() {
        let n = kw_num("#nan");
        assert_eq!(n.raw, "#nan");
        assert!(n.as_f64.unwrap().is_nan());
    }

    #[test]
    fn overflow_preserves_raw() {
        let big = "99999999999999999999 ";
        let n = num(big);
        assert_eq!(n.raw, "99999999999999999999");
        assert_eq!(n.as_i64, None);
        // f64 は精度落ちるが値は持てる
        assert!(n.as_f64.is_some());
    }

    #[test]
    fn not_a_number() {
        let mut s = Scanner::new("foo ");
        assert!(parse_number(&mut s).unwrap().is_none());
    }

    #[test]
    fn sign_only_not_a_number() {
        let mut s = Scanner::new("+ ");
        assert!(parse_number(&mut s).unwrap().is_none());
    }
}
