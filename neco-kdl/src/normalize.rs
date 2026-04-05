use core::fmt::Write;

use crate::scan::{is_disallowed, is_identifier_char, is_newline};
use crate::{KdlDocument, KdlEntry, KdlNode, KdlNumber, KdlValue};

/// KDL ドキュメントを正規化形式に変換する。
///
/// 正規化ルール（公式テストスイート準拠）:
/// - コメント除去（パース時に消える）
/// - プロパティはキーのアルファベット順
/// - 重複プロパティは右側のみ保持
/// - 全文字列を通常引用文字列に変換
/// - 識別子は可能ならアンクォート
/// - インデント 4 スペース
/// - 数値は正規化形式（hex/octal/binary → decimal、アンダースコア除去）
/// - 末尾改行
pub fn normalize(doc: &KdlDocument) -> String {
    let mut out = String::new();
    for node in &doc.nodes {
        write_node(&mut out, node, 0);
    }
    // 正規化出力は常に末尾改行で終わる
    if out.is_empty() {
        out.push('\n');
    }
    out
}

fn write_node(out: &mut String, node: &KdlNode, depth: usize) {
    let indent = "    ".repeat(depth);
    out.push_str(&indent);

    // type annotation
    if let Some(ty) = &node.ty {
        out.push('(');
        write_identifier_or_string(out, ty);
        out.push(')');
    }

    // ノード名
    write_identifier_or_string(out, &node.name);

    // entries: arguments (出現順) + properties (重複統合 + アルファベット順)
    let mut args: Vec<&KdlEntry> = Vec::new();
    let mut props: Vec<(&str, &KdlEntry)> = Vec::new();

    for entry in &node.entries {
        match entry {
            KdlEntry::Argument { .. } => args.push(entry),
            KdlEntry::Property { key, .. } => {
                // 重複統合: 同キーは後勝ち
                if let Some(existing) = props.iter_mut().find(|(k, _)| *k == key.as_str()) {
                    existing.1 = entry;
                } else {
                    props.push((key, entry));
                }
            }
        }
    }

    // プロパティをキー順でソート
    props.sort_by_key(|(k, _)| *k);

    // arguments 出力
    for entry in &args {
        out.push(' ');
        write_entry(out, entry);
    }

    // properties 出力
    for (_, entry) in &props {
        out.push(' ');
        write_entry(out, entry);
    }

    // children
    match &node.children {
        Some(children) if !children.is_empty() => {
            out.push_str(" {\n");
            for child in children {
                write_node(out, child, depth + 1);
            }
            out.push_str(&indent);
            out.push_str("}\n");
        }
        _ => {
            out.push('\n');
        }
    }
}

fn write_entry(out: &mut String, entry: &KdlEntry) {
    match entry {
        KdlEntry::Argument { ty, value } => {
            if let Some(ty) = ty {
                out.push('(');
                write_identifier_or_string(out, ty);
                out.push(')');
            }
            write_value(out, value);
        }
        KdlEntry::Property { key, ty, value } => {
            write_identifier_or_string(out, key);
            out.push('=');
            if let Some(ty) = ty {
                out.push('(');
                write_identifier_or_string(out, ty);
                out.push(')');
            }
            write_value(out, value);
        }
    }
}

fn write_value(out: &mut String, value: &KdlValue) {
    match value {
        KdlValue::String(s) => write_identifier_or_string(out, s),
        KdlValue::Number(n) => out.push_str(&normalize_number(n)),
        KdlValue::Bool(true) => out.push_str("#true"),
        KdlValue::Bool(false) => out.push_str("#false"),
        KdlValue::Null => out.push_str("#null"),
    }
}

/// 文字列を引用文字列として出力する。
fn write_quoted_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if is_newline(c) || is_disallowed(c) => {
                // VT, NEL, LS, PS 等の改行文字 + BOM 等の disallowed code points
                write!(out, "\\u{{{:04X}}}", c as u32).unwrap();
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// identifier として有効なら裸で、そうでなければ引用文字列で出力する。
fn write_identifier_or_string(out: &mut String, s: &str) {
    if can_be_bare_identifier(s) {
        out.push_str(s);
    } else {
        write_quoted_string(out, s);
    }
}

/// KDL v2 の identifier-string として有効かどうかを判定する。
fn can_be_bare_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let mut chars = s.chars();
    let first = chars.next().unwrap();

    // 全文字が identifier-char であることが前提
    if !is_identifier_char(first) {
        return false;
    }
    for ch in chars {
        if !is_identifier_char(ch) {
            return false;
        }
    }

    // 禁止キーワード
    match s {
        "true" | "false" | "null" | "inf" | "-inf" | "nan" => return false,
        _ => {}
    }

    let is_sign = first == '+' || first == '-';
    let is_dot = first == '.';
    let is_digit = first.is_ascii_digit();

    // 先頭が digit → 数値と紛らわしい
    if is_digit {
        return false;
    }

    if is_sign {
        let second = s.chars().nth(1);
        match second {
            None => return true,                           // "+" or "-" は valid identifier
            Some(c) if c.is_ascii_digit() => return false, // "+1", "-2"
            Some('.') => {
                // "+.", "-." は OK
                // "+.1" は dotted-ident の数値紛らわしいケース
                let third = s.chars().nth(2);
                if let Some(c) = third {
                    if c.is_ascii_digit() {
                        return false;
                    }
                }
            }
            _ => {}
        }
    }

    if is_dot {
        let second = s.chars().nth(1);
        if let Some(c) = second {
            if c.is_ascii_digit() {
                return false;
            }
        }
    }

    true
}

/// 数値を正規化形式に変換する（raw ベース）。
fn normalize_number(n: &KdlNumber) -> String {
    let raw = &n.raw;

    // keyword number: #inf, #-inf, #nan
    if raw.starts_with('#') {
        return raw.clone();
    }

    // sign を分離
    let (sign, rest) = if let Some(r) = raw.strip_prefix('-') {
        ("-", r)
    } else if let Some(r) = raw.strip_prefix('+') {
        ("", r)
    } else {
        ("", raw.as_str())
    };

    // hex/octal/binary
    if let Some(hex_part) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        let clean: String = hex_part.chars().filter(|&c| c != '_').collect();
        return format_prefixed_number(sign, &clean, 16);
    }
    if let Some(oct_part) = rest.strip_prefix("0o").or_else(|| rest.strip_prefix("0O")) {
        let clean: String = oct_part.chars().filter(|&c| c != '_').collect();
        return format_prefixed_number(sign, &clean, 8);
    }
    if let Some(bin_part) = rest.strip_prefix("0b").or_else(|| rest.strip_prefix("0B")) {
        let clean: String = bin_part.chars().filter(|&c| c != '_').collect();
        return format_prefixed_number(sign, &clean, 2);
    }

    // decimal: アンダースコア除去
    let clean: String = rest.chars().filter(|&c| c != '_').collect();

    // 指数部の有無で分岐
    if let Some(e_pos) = clean.find(['e', 'E']) {
        // float with exponent
        let mantissa = &clean[..e_pos];
        let exp_part = &clean[e_pos + 1..];
        let normalized_exp = normalize_exponent(exp_part);
        let normalized_mantissa = normalize_decimal_mantissa(mantissa);

        if sign == "-" {
            format!("-{}E{}", normalized_mantissa, normalized_exp)
        } else {
            format!("{}E{}", normalized_mantissa, normalized_exp)
        }
    } else if clean.contains('.') {
        // float without exponent
        let normalized = normalize_decimal_mantissa(&clean);
        if sign == "-" {
            format!("-{normalized}")
        } else {
            normalized
        }
    } else {
        // integer
        let normalized = normalize_decimal_integer(&clean);
        if sign == "-" && normalized != "0" {
            format!("-{normalized}")
        } else {
            normalized
        }
    }
}

/// hex/octal/binary の digits を u128 経由で 10 進文字列に変換する。
fn format_prefixed_number(sign: &str, digits: &str, radix: u32) -> String {
    // u128 で試す
    if let Ok(val) = u128::from_str_radix(digits, radix) {
        if sign == "-" && val != 0 {
            format!("-{val}")
        } else {
            format!("{val}")
        }
    } else {
        // u128 を超える場合: 文字列ベースの変換
        let decimal = big_radix_to_decimal(digits, radix);
        if sign == "-" && decimal != "0" {
            format!("-{decimal}")
        } else {
            decimal
        }
    }
}

/// u128 に収まらない任意精度の基数変換。
/// digits は有効な hex/octal/binary 文字列。
fn big_radix_to_decimal(digits: &str, radix: u32) -> String {
    // 10 進の桁を Vec<u8> で保持（最下位が先頭）
    let mut result: Vec<u8> = vec![0];

    for ch in digits.chars() {
        // パーサーが検証済みの有効な基数文字列のみ到達する
        let digit_val = ch.to_digit(radix).expect("parser-validated digit") as u8;

        // result *= radix
        let mut carry: u16 = 0;
        for d in result.iter_mut() {
            let prod = (*d as u16) * (radix as u16) + carry;
            *d = (prod % 10) as u8;
            carry = prod / 10;
        }
        while carry > 0 {
            result.push((carry % 10) as u8);
            carry /= 10;
        }

        // result += digit_val
        let mut carry: u16 = digit_val as u16;
        for d in result.iter_mut() {
            let sum = (*d as u16) + carry;
            *d = (sum % 10) as u8;
            carry = sum / 10;
            if carry == 0 {
                break;
            }
        }
        while carry > 0 {
            result.push((carry % 10) as u8);
            carry /= 10;
        }
    }

    // 先行ゼロ除去 + 逆順
    while result.len() > 1 && *result.last().unwrap() == 0 {
        result.pop();
    }
    result.iter().rev().map(|d| (b'0' + d) as char).collect()
}

/// 10 進整数文字列を正規化する（先行ゼロ除去）。
fn normalize_decimal_integer(s: &str) -> String {
    let trimmed = s.trim_start_matches('0');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// 10 進仮数部を正規化する（整数部の先行ゼロ除去）。
fn normalize_decimal_mantissa(s: &str) -> String {
    if let Some(dot_pos) = s.find('.') {
        let int_part = &s[..dot_pos];
        let frac_part = &s[dot_pos + 1..];
        let int_normalized = normalize_decimal_integer(int_part);
        format!("{int_normalized}.{frac_part}")
    } else {
        normalize_decimal_integer(s)
    }
}

/// 指数部を正規化する（明示的な符号）。
fn normalize_exponent(s: &str) -> String {
    if let Some(rest) = s.strip_prefix('-') {
        format!("-{}", normalize_decimal_integer(rest))
    } else if let Some(rest) = s.strip_prefix('+') {
        format!("+{}", normalize_decimal_integer(rest))
    } else {
        format!("+{}", normalize_decimal_integer(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KdlEntry, KdlNode, KdlNumber, KdlValue};

    fn make_doc(nodes: Vec<KdlNode>) -> KdlDocument {
        KdlDocument { nodes }
    }

    fn make_node(name: &str) -> KdlNode {
        KdlNode {
            ty: None,
            name: name.to_string(),
            entries: Vec::new(),
            children: None,
        }
    }

    #[test]
    fn empty_document() {
        let doc = make_doc(vec![]);
        assert_eq!(normalize(&doc), "\n");
    }

    #[test]
    fn basic_node() {
        let mut node = make_node("node");
        node.entries.push(KdlEntry::Argument {
            ty: None,
            value: KdlValue::String("hello".to_string()),
        });
        let doc = make_doc(vec![node]);
        // "hello" は有効な identifier なので裸で出力される
        assert_eq!(normalize(&doc), "node hello\n");
    }

    #[test]
    fn property_sort_and_dedup() {
        let mut node = make_node("node");
        node.entries.push(KdlEntry::Property {
            key: "b".to_string(),
            ty: None,
            value: KdlValue::String("1".to_string()),
        });
        node.entries.push(KdlEntry::Property {
            key: "a".to_string(),
            ty: None,
            value: KdlValue::String("2".to_string()),
        });
        node.entries.push(KdlEntry::Property {
            key: "b".to_string(),
            ty: None,
            value: KdlValue::String("3".to_string()),
        });
        let doc = make_doc(vec![node]);
        assert_eq!(normalize(&doc), "node a=\"2\" b=\"3\"\n");
    }

    #[test]
    fn number_normalization() {
        assert_eq!(
            normalize_number(&KdlNumber {
                raw: "0xff".to_string(),
                as_i64: Some(255),
                as_f64: Some(255.0),
            }),
            "255"
        );
        assert_eq!(
            normalize_number(&KdlNumber {
                raw: "0b1010".to_string(),
                as_i64: Some(10),
                as_f64: Some(10.0),
            }),
            "10"
        );
        assert_eq!(
            normalize_number(&KdlNumber {
                raw: "1.0e+10".to_string(),
                as_i64: None,
                as_f64: Some(1e10),
            }),
            "1.0E+10"
        );
        assert_eq!(
            normalize_number(&KdlNumber {
                raw: "1.23E+1000".to_string(),
                as_i64: None,
                as_f64: None,
            }),
            "1.23E+1000"
        );
        assert_eq!(
            normalize_number(&KdlNumber {
                raw: "#inf".to_string(),
                as_i64: None,
                as_f64: Some(f64::INFINITY),
            }),
            "#inf"
        );
        assert_eq!(
            normalize_number(&KdlNumber {
                raw: "+10".to_string(),
                as_i64: Some(10),
                as_f64: Some(10.0),
            }),
            "10"
        );
        assert_eq!(
            normalize_number(&KdlNumber {
                raw: "-0".to_string(),
                as_i64: Some(0),
                as_f64: Some(0.0),
            }),
            "0"
        );
    }

    #[test]
    fn type_annotation() {
        let mut node = make_node("node");
        node.ty = Some("mytype".to_string());
        node.entries.push(KdlEntry::Argument {
            ty: Some("u8".to_string()),
            value: KdlValue::Number(KdlNumber {
                raw: "42".to_string(),
                as_i64: Some(42),
                as_f64: Some(42.0),
            }),
        });
        let doc = make_doc(vec![node]);
        assert_eq!(normalize(&doc), "(mytype)node (u8)42\n");
    }

    #[test]
    fn empty_children_omitted() {
        let mut node = make_node("node");
        node.children = Some(vec![]);
        let doc = make_doc(vec![node]);
        assert_eq!(normalize(&doc), "node\n");
    }

    #[test]
    fn nested_children() {
        let inner = make_node("child");
        let mut node = make_node("parent");
        node.children = Some(vec![inner]);
        let doc = make_doc(vec![node]);
        assert_eq!(normalize(&doc), "parent {\n    child\n}\n");
    }

    #[test]
    fn identifier_judgment() {
        assert!(can_be_bare_identifier("foo"));
        assert!(can_be_bare_identifier("foo-bar"));
        assert!(can_be_bare_identifier("+"));
        assert!(can_be_bare_identifier("-"));
        assert!(can_be_bare_identifier(".md"));

        assert!(!can_be_bare_identifier("")); // 空
        assert!(!can_be_bare_identifier("true")); // keyword
        assert!(!can_be_bare_identifier("false"));
        assert!(!can_be_bare_identifier("null"));
        assert!(!can_be_bare_identifier("inf"));
        assert!(!can_be_bare_identifier("-inf"));
        assert!(!can_be_bare_identifier("nan"));
        assert!(!can_be_bare_identifier("0node")); // digit 先頭
        assert!(!can_be_bare_identifier("hello world")); // space
    }

    #[test]
    fn big_hex_conversion() {
        // 0xABCDEF0123456789abcdef → 88 bits, u128 で OK
        assert_eq!(
            normalize_number(&KdlNumber {
                raw: "0xABCDEF0123456789abcdef".to_string(),
                as_i64: None,
                as_f64: None,
            }),
            "207698809136909011942886895"
        );
    }

    #[test]
    fn keywords_and_null() {
        let mut node = make_node("node");
        node.entries.push(KdlEntry::Argument {
            ty: None,
            value: KdlValue::Bool(true),
        });
        node.entries.push(KdlEntry::Argument {
            ty: None,
            value: KdlValue::Bool(false),
        });
        node.entries.push(KdlEntry::Argument {
            ty: None,
            value: KdlValue::Null,
        });
        let doc = make_doc(vec![node]);
        assert_eq!(normalize(&doc), "node #true #false #null\n");
    }
}
