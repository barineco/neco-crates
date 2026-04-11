use core::fmt::{self, Write};

use crate::scan::{is_disallowed, is_identifier_char, is_newline};
use crate::{KdlDocument, KdlEntry, KdlNode, KdlValue};

/// KDL v2 ドキュメントを文字列へシリアライズする。
pub fn serialize(doc: &KdlDocument) -> String {
    doc.to_string()
}

impl core::fmt::Display for KdlDocument {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for node in &self.nodes {
            write_node(f, node, 0)?;
        }
        Ok(())
    }
}

impl core::fmt::Display for KdlNode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write_node(f, self, 0)
    }
}

impl core::fmt::Display for KdlEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            KdlEntry::Argument { ty, value } => {
                write_type_annotation(f, ty.as_deref())?;
                write!(f, "{value}")
            }
            KdlEntry::Property { key, ty, value } => {
                write_identifier(f, key)?;
                write!(f, "=")?;
                write_type_annotation(f, ty.as_deref())?;
                write!(f, "{value}")
            }
        }
    }
}

impl core::fmt::Display for KdlValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            KdlValue::String(value) => write_quoted_string(f, value),
            KdlValue::Number(value) => write!(f, "{}", value.raw()),
            KdlValue::Bool(true) => write!(f, "#true"),
            KdlValue::Bool(false) => write!(f, "#false"),
            KdlValue::Null => write!(f, "#null"),
        }
    }
}

fn write_node(f: &mut fmt::Formatter<'_>, node: &KdlNode, depth: usize) -> fmt::Result {
    for _ in 0..depth {
        write!(f, "    ")?;
    }

    write_type_annotation(f, node.ty())?;
    write_identifier(f, node.name())?;

    for entry in node.entries() {
        write!(f, " {entry}")?;
    }

    match node.children() {
        Some(children) if !children.is_empty() => {
            writeln!(f, " {{")?;
            for child in children {
                write_node(f, child, depth + 1)?;
            }
            for _ in 0..depth {
                write!(f, "    ")?;
            }
            writeln!(f, "}}")
        }
        _ => writeln!(f),
    }
}

fn write_type_annotation(f: &mut fmt::Formatter<'_>, ty: Option<&str>) -> fmt::Result {
    if let Some(ty) = ty {
        write!(f, "(")?;
        write_identifier(f, ty)?;
        write!(f, ")")?;
    }
    Ok(())
}

fn write_identifier(f: &mut fmt::Formatter<'_>, value: &str) -> fmt::Result {
    if can_be_bare_identifier(value) {
        write!(f, "{value}")
    } else {
        write_quoted_string(f, value)
    }
}

fn write_quoted_string(f: &mut fmt::Formatter<'_>, value: &str) -> fmt::Result {
    write!(f, "\"")?;
    for ch in value.chars() {
        match ch {
            '"' => write!(f, "\\\"")?,
            '\\' => write!(f, "\\\\")?,
            '\n' => write!(f, "\\n")?,
            '\r' => write!(f, "\\r")?,
            '\t' => write!(f, "\\t")?,
            '\u{0008}' => write!(f, "\\b")?,
            '\u{000C}' => write!(f, "\\f")?,
            ch if is_newline(ch) || is_disallowed(ch) => write!(f, "\\u{{{:04X}}}", ch as u32)?,
            ch => f.write_char(ch)?,
        }
    }
    write!(f, "\"")
}

fn can_be_bare_identifier(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }

    let mut chars = value.chars();
    let first = chars.next().unwrap();

    if !is_identifier_char(first) || chars.clone().any(|ch| !is_identifier_char(ch)) {
        return false;
    }

    match value {
        "true" | "false" | "null" | "inf" | "-inf" | "nan" => return false,
        _ => {}
    }

    if first.is_ascii_digit() {
        return false;
    }

    if first == '+' || first == '-' {
        match value.chars().nth(1) {
            None => return true,
            Some(ch) if ch.is_ascii_digit() => return false,
            Some('.') => {
                if let Some(ch) = value.chars().nth(2) {
                    if ch.is_ascii_digit() {
                        return false;
                    }
                }
            }
            _ => {}
        }
    }

    if first == '.' {
        if let Some(ch) = value.chars().nth(1) {
            if ch.is_ascii_digit() {
                return false;
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::serialize;
    use crate::parse;

    fn assert_roundtrip(input: &str) {
        let parsed = parse(input).unwrap();
        let reparsed = parse(&serialize(&parsed)).unwrap();
        assert_eq!(reparsed, parsed);
    }

    #[test]
    fn roundtrip_basic_nodes() {
        assert_roundtrip("plain\nargs 1 2 3\nprops key=\"value\" enabled=#true");
    }

    #[test]
    fn roundtrip_type_annotation() {
        assert_roundtrip("(string)name \"value\"");
    }

    #[test]
    fn roundtrip_nested_children() {
        assert_roundtrip("root {\n    branch {\n        leaf \"value\"\n    }\n}");
    }

    #[test]
    fn roundtrip_mixed_document() {
        assert_roundtrip(
            "first 1 mode=\"fast\"\nsecond key=#null {\n    child (u8)1 prop=(i64)-2\n}\nthird \"tail\"",
        );
    }

    #[test]
    fn roundtrip_escaped_string() {
        assert_roundtrip("text \"line1\\nline2\\\\\\\"quoted\\\"\"");
    }
}
