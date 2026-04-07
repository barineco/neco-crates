/// Format-agnostic value type for KDL conversion.
///
/// This enum serves as an intermediate representation between KDL documents
/// and other data formats (JSON, CBOR, etc.) without requiring external
/// dependencies in the neco-kdl crate.
///
/// Integer(i64) and Float(f64) are kept distinct, unlike JSON's single Number(f64).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    /// Order-preserving map of key-value pairs.
    Object(Vec<(String, Value)>),
}

use crate::{KdlDocument, KdlEntry, KdlError, KdlErrorKind, KdlNode, KdlNumber, KdlValue};

/// Converts a `Value` into a `KdlDocument`.
///
/// Only `Value::Object` can be represented as a KDL document (since KDL is a
/// collection of named nodes). Each key in the object becomes a node name.
///
/// Conversion rules:
/// - Primitive values (Bool/Integer/Float/String/Null) → single positional argument
/// - Array values → multiple positional arguments
/// - Nested Object values → children block
///
/// # Errors
///
/// Returns an error if `value` is not a `Value::Object`.
pub fn value_to_kdl_document(value: &Value) -> Result<KdlDocument, KdlError> {
    match value {
        Value::Object(fields) => {
            let nodes = fields
                .iter()
                .map(|(key, val)| value_to_kdl_node(key, val))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(KdlDocument { nodes })
        }
        _ => Err(make_conv_error("top-level value must be an Object")),
    }
}

/// Converts a `KdlDocument` back into a `Value`.
///
/// The document is always decoded as a `Value::Object` where each node name
/// becomes a key.
///
/// Decoding rules for node arguments:
/// - No positional arguments and no children → `Value::Null`
/// - Exactly one positional argument, no children → scalar value
/// - Two or more positional arguments, no children → `Value::Array`
/// - Children block (no positional arguments) → nested `Value::Object`
///
/// # Errors
///
/// Returns an error if a node cannot be decoded (e.g., mixed arguments and
/// children, or an unrecognised value type).
pub fn kdl_document_to_value(doc: &KdlDocument) -> Result<Value, KdlError> {
    let mut fields = Vec::with_capacity(doc.nodes.len());
    for node in &doc.nodes {
        let val = kdl_node_to_value(node)?;
        fields.push((node.name.clone(), val));
    }
    Ok(Value::Object(fields))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// A sentinel type-annotation string used to distinguish `Value::Float` from
/// `Value::Integer` when both are stored as KDL numbers.
///
/// KDL's `KdlNumber` stores a raw string; we use a `(f64)` type annotation on
/// the argument to mark that the original value was a `Value::Float`.
const FLOAT_TYPE: &str = "f64";

/// Converts a single key-value pair into a `KdlNode`.
fn value_to_kdl_node(key: &str, value: &Value) -> Result<KdlNode, KdlError> {
    match value {
        // Nested object → children block
        Value::Object(fields) => {
            let children = fields
                .iter()
                .map(|(k, v)| value_to_kdl_node(k, v))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(KdlNode {
                ty: None,
                name: key.to_string(),
                entries: Vec::new(),
                children: Some(children),
            })
        }

        // Array → multiple positional arguments
        Value::Array(items) => {
            let entries = items
                .iter()
                .map(primitive_to_argument)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(KdlNode {
                ty: None,
                name: key.to_string(),
                entries,
                children: None,
            })
        }

        // Primitive → single positional argument
        _ => {
            let entry = primitive_to_argument(value)?;
            Ok(KdlNode {
                ty: None,
                name: key.to_string(),
                entries: vec![entry],
                children: None,
            })
        }
    }
}

/// Converts a primitive `Value` (non-Array, non-Object) into a `KdlEntry::Argument`.
///
/// `Value::Float` gets a `(f64)` type annotation so that round-trip decoding
/// can distinguish it from `Value::Integer`.
fn primitive_to_argument(value: &Value) -> Result<KdlEntry, KdlError> {
    match value {
        Value::Null => Ok(KdlEntry::Argument {
            ty: None,
            value: KdlValue::Null,
        }),
        Value::Bool(b) => Ok(KdlEntry::Argument {
            ty: None,
            value: KdlValue::Bool(*b),
        }),
        Value::Integer(i) => Ok(KdlEntry::Argument {
            ty: None,
            value: KdlValue::Number(i64_to_kdl_number(*i)),
        }),
        Value::Float(f) => Ok(KdlEntry::Argument {
            // Use a (f64) type annotation to distinguish from Integer on decode.
            ty: Some(FLOAT_TYPE.to_string()),
            value: KdlValue::Number(f64_to_kdl_number(*f)),
        }),
        Value::String(s) => Ok(KdlEntry::Argument {
            ty: None,
            value: KdlValue::String(s.clone()),
        }),
        Value::Array(_) | Value::Object(_) => Err(make_conv_error(
            "nested Array/Object inside Array is not supported",
        )),
    }
}

/// Decodes a `KdlNode` into a `Value`.
fn kdl_node_to_value(node: &KdlNode) -> Result<Value, KdlError> {
    // Collect positional arguments only (properties are ignored for now).
    let args: Vec<&KdlEntry> = node
        .entries
        .iter()
        .filter(|e| matches!(e, KdlEntry::Argument { .. }))
        .collect();

    let has_children = node
        .children
        .as_ref()
        .map(|c| !c.is_empty())
        .unwrap_or(false);

    match (args.len(), has_children) {
        // No arguments and no children → null
        (0, false) => Ok(Value::Null),

        // Children block only → nested Object
        (0, true) => {
            let children = node.children.as_ref().unwrap();
            let mut fields = Vec::with_capacity(children.len());
            for child in children {
                let v = kdl_node_to_value(child)?;
                fields.push((child.name.clone(), v));
            }
            Ok(Value::Object(fields))
        }

        // Exactly one argument → scalar
        (1, false) => kdl_entry_to_value(args[0]),

        // Multiple arguments → Array
        (_, false) => {
            let items = args
                .iter()
                .map(|e| kdl_entry_to_value(e))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Array(items))
        }

        // Both arguments and children → ambiguous, treat as error
        (_, true) => Err(make_conv_error(
            "node has both positional arguments and children, which is not supported",
        )),
    }
}

/// Decodes a single `KdlEntry::Argument` into a scalar `Value`.
fn kdl_entry_to_value(entry: &KdlEntry) -> Result<Value, KdlError> {
    let (ty, kdl_val) = match entry {
        KdlEntry::Argument { ty, value } => (ty.as_deref(), value),
        KdlEntry::Property { .. } => {
            return Err(make_conv_error(
                "expected positional argument, got property",
            ));
        }
    };

    match kdl_val {
        KdlValue::Null => Ok(Value::Null),
        KdlValue::Bool(b) => Ok(Value::Bool(*b)),
        KdlValue::String(s) => Ok(Value::String(s.clone())),
        KdlValue::Number(n) => {
            // A (f64) type annotation marks an explicitly floating-point value.
            if ty == Some(FLOAT_TYPE) {
                match n.as_f64 {
                    Some(f) => Ok(Value::Float(f)),
                    None => Err(make_conv_error("(f64)-annotated number has no f64 value")),
                }
            } else {
                // No annotation: prefer integer interpretation, fall back to float.
                match n.as_i64 {
                    Some(i) => Ok(Value::Integer(i)),
                    None => match n.as_f64 {
                        Some(f) => Ok(Value::Float(f)),
                        None => Err(make_conv_error("number has neither i64 nor f64 value")),
                    },
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Number construction helpers
// ---------------------------------------------------------------------------

fn i64_to_kdl_number(i: i64) -> KdlNumber {
    KdlNumber {
        raw: i.to_string(),
        as_i64: Some(i),
        as_f64: Some(i as f64),
    }
}

fn f64_to_kdl_number(f: f64) -> KdlNumber {
    // Produce a raw string that round-trips through the KDL parser.
    // We always emit at least one decimal digit so the parser recognises it as
    // a float (e.g. "2.5", "1.0").
    let raw = format_f64(f);
    KdlNumber {
        raw,
        as_i64: None, // intentionally None to distinguish from Integer
        as_f64: Some(f),
    }
}

/// Formats an f64 as a KDL-parseable decimal string with an explicit dot.
fn format_f64(f: f64) -> String {
    if f.is_nan() {
        return "#nan".to_string();
    }
    if f.is_infinite() {
        return if f > 0.0 {
            "#inf".to_string()
        } else {
            "#-inf".to_string()
        };
    }

    // Use Rust's default Display which includes the decimal point when
    // needed.  For whole numbers like 1.0, Display produces "1" — we
    // append ".0" explicitly.
    let s = format!("{f}");
    if s.contains('.') || s.contains('e') || s.contains('E') || s.starts_with('#') {
        s
    } else {
        format!("{s}.0")
    }
}

/// Creates a conversion error.  KdlError requires line/col even for
/// non-parse errors; we use 0:0 as a sentinel for "not from parsing".
fn make_conv_error(msg: &str) -> KdlError {
    // We repurpose `UnexpectedChar` with a NUL sentinel and encode the message
    // in a way callers can detect. Since `KdlErrorKind` has no `Custom`
    // variant, we use `UnexpectedChar('\0')` as the closest available carrier.
    // The public API exposes `kind()` and `Display`, so callers should use
    // those. We cannot add a Custom variant without modifying value.rs, which
    // is out of scope for T2.
    //
    // For now use UnexpectedEof as a placeholder — it is easily distinguished
    // from parse errors by line == 0.
    // TODO: Add a Custom(String) variant to KdlErrorKind to carry the message.
    let _ = msg;
    KdlError {
        line: 0,
        col: 0,
        kind: KdlErrorKind::UnexpectedEof,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: round-trip a Value through KdlDocument and back.
    fn roundtrip(v: &Value) -> Value {
        let doc = value_to_kdl_document(v).expect("encode failed");
        kdl_document_to_value(&doc).expect("decode failed")
    }

    // -----------------------------------------------------------------------
    // Single scalar node: text "hello"
    // -----------------------------------------------------------------------

    #[test]
    fn roundtrip_single_string_node() {
        let input = Value::Object(vec![(
            "text".to_string(),
            Value::String("hello".to_string()),
        )]);
        let output = roundtrip(&input);
        assert_eq!(input, output);
    }

    // -----------------------------------------------------------------------
    // Multiple-value node: langs "en" "ja"  →  Array
    // -----------------------------------------------------------------------

    #[test]
    fn roundtrip_array_node() {
        let input = Value::Object(vec![(
            "langs".to_string(),
            Value::Array(vec![
                Value::String("en".to_string()),
                Value::String("ja".to_string()),
            ]),
        )]);
        let output = roundtrip(&input);
        assert_eq!(input, output);
    }

    // -----------------------------------------------------------------------
    // Nested Object → children block
    // -----------------------------------------------------------------------

    #[test]
    fn roundtrip_nested_object() {
        let root_ref = Value::Object(vec![
            ("cid".to_string(), Value::String("bafyCID".to_string())),
            ("uri".to_string(), Value::String("at://x".to_string())),
        ]);
        let input = Value::Object(vec![(
            "reply".to_string(),
            Value::Object(vec![("root".to_string(), root_ref)]),
        )]);
        let output = roundtrip(&input);
        assert_eq!(input, output);
    }

    // -----------------------------------------------------------------------
    // Integer vs Float are preserved
    // -----------------------------------------------------------------------

    #[test]
    fn roundtrip_integer() {
        let input = Value::Object(vec![("count".to_string(), Value::Integer(42))]);
        let output = roundtrip(&input);
        assert_eq!(input, output);
        // Verify the decoded value is still Integer, not Float.
        match &output {
            Value::Object(fields) => {
                assert_eq!(fields[0].1, Value::Integer(42));
            }
            _ => panic!("expected Object"),
        }
    }

    #[test]
    fn roundtrip_float() {
        let input = Value::Object(vec![("ratio".to_string(), Value::Float(2.5))]);
        let output = roundtrip(&input);
        match &output {
            Value::Object(fields) => {
                if let Value::Float(f) = fields[0].1 {
                    assert!((f - 2.5_f64).abs() < 1e-10);
                } else {
                    panic!("expected Float, got {:?}", fields[0].1);
                }
            }
            _ => panic!("expected Object"),
        }
    }

    #[test]
    fn integer_and_float_are_distinct() {
        let int_input = Value::Object(vec![("n".to_string(), Value::Integer(1))]);
        let flt_input = Value::Object(vec![("n".to_string(), Value::Float(1.0))]);

        let int_out = roundtrip(&int_input);
        let flt_out = roundtrip(&flt_input);

        // After round-trip they must remain distinct.
        assert_ne!(int_out, flt_out);

        match &int_out {
            Value::Object(f) => assert!(matches!(f[0].1, Value::Integer(_))),
            _ => panic!(),
        }
        match &flt_out {
            Value::Object(f) => assert!(matches!(f[0].1, Value::Float(_))),
            _ => panic!(),
        }
    }

    // -----------------------------------------------------------------------
    // Null
    // -----------------------------------------------------------------------

    #[test]
    fn roundtrip_null() {
        let input = Value::Object(vec![("deleted".to_string(), Value::Null)]);
        let output = roundtrip(&input);
        assert_eq!(input, output);
    }

    // -----------------------------------------------------------------------
    // Bool
    // -----------------------------------------------------------------------

    #[test]
    fn roundtrip_bool() {
        let input = Value::Object(vec![
            ("active".to_string(), Value::Bool(true)),
            ("deleted".to_string(), Value::Bool(false)),
        ]);
        let output = roundtrip(&input);
        assert_eq!(input, output);
    }

    // -----------------------------------------------------------------------
    // Negative integer
    // -----------------------------------------------------------------------

    #[test]
    fn roundtrip_negative_integer() {
        let input = Value::Object(vec![("offset".to_string(), Value::Integer(-7))]);
        let output = roundtrip(&input);
        assert_eq!(input, output);
    }

    // -----------------------------------------------------------------------
    // Error: non-Object at top level
    // -----------------------------------------------------------------------

    #[test]
    fn top_level_non_object_is_error() {
        let result = value_to_kdl_document(&Value::String("oops".to_string()));
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Round-trip via normalize() + parse() (full text serialization)
    // -----------------------------------------------------------------------

    #[test]
    fn roundtrip_via_text() {
        use crate::{normalize, parse};

        let input = Value::Object(vec![
            ("name".to_string(), Value::String("Alice".to_string())),
            ("age".to_string(), Value::Integer(30)),
            ("score".to_string(), Value::Float(9.5)),
            ("active".to_string(), Value::Bool(true)),
            (
                "meta".to_string(),
                Value::Object(vec![(
                    "role".to_string(),
                    Value::String("admin".to_string()),
                )]),
            ),
        ]);

        let doc = value_to_kdl_document(&input).unwrap();
        let text = normalize(&doc);
        let doc2 = parse(&text).unwrap();
        let output = kdl_document_to_value(&doc2).unwrap();

        // Check field by field (Float comparison needs epsilon).
        match (&input, &output) {
            (Value::Object(a), Value::Object(b)) => {
                assert_eq!(a.len(), b.len());
                assert_eq!(a[0], b[0]); // name
                assert_eq!(a[1], b[1]); // age
                                        // score: float comparison
                if let (Value::Float(fa), Value::Float(fb)) = (&a[2].1, &b[2].1) {
                    assert!((fa - fb).abs() < 1e-10);
                } else {
                    panic!("expected Float for score");
                }
                assert_eq!(a[3], b[3]); // active
                assert_eq!(a[4], b[4]); // meta
            }
            _ => panic!("expected Object"),
        }
    }

    // -----------------------------------------------------------------------
    // Node with no arguments decodes as Null
    // -----------------------------------------------------------------------

    #[test]
    fn empty_node_decoded_as_null() {
        // Build a document with a node that has no arguments.
        let doc = KdlDocument {
            nodes: vec![KdlNode {
                ty: None,
                name: "empty".to_string(),
                entries: vec![],
                children: None,
            }],
        };
        let val = kdl_document_to_value(&doc).unwrap();
        assert_eq!(val, Value::Object(vec![("empty".to_string(), Value::Null)]));
    }
}
