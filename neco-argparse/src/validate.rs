use std::collections::HashMap;

use neco_json::JsonValue;

use crate::args::{ArgDef, ArgType};
use crate::error::ArgParseError;
use crate::parsed::{value_to_f64, ParsedArgs};

/// ArgDef に基づいて params JsonValue をパース・バリデーションする。
///
/// - `params` は `JsonValue::Object` であることを想定する（Object 以外はキー無しとして扱う）
/// - `_positional` キーは位置引数リストとして特別処理する
pub fn parse_and_validate(
    params: &JsonValue,
    defs: &[ArgDef],
) -> Result<ParsedArgs, ArgParseError> {
    let mut inner: HashMap<String, JsonValue> = HashMap::new();

    // _positional を抽出
    let positional: Vec<String> = params
        .get("_positional")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // params が Object の場合、全キーを inner にコピー（_positional 除く）
    if let Some(fields) = params.as_object() {
        for (k, v) in fields {
            if k != "_positional" {
                inner.insert(k.clone(), v.clone());
            }
        }
    }

    // 各 ArgDef に基づくバリデーション
    for def in defs {
        let has_value = inner.contains_key(&def.name);

        if !has_value {
            if let Some(ref default) = def.default_value {
                inner.insert(def.name.clone(), default.clone());
                continue;
            }
            if def.required {
                return Err(ArgParseError::MissingRequired(def.name.clone()));
            }
            continue;
        }

        let value = &inner[&def.name];

        // Enum values チェック
        if matches!(def.arg_type, ArgType::Enum) {
            if let Some(ref allowed) = def.values {
                if let Some(s) = value.as_str() {
                    if !allowed.iter().any(|v| v == s) {
                        return Err(ArgParseError::InvalidParameter(format!(
                            "{} の値 '{}' は許可されていません。許可値: {:?}",
                            def.name, s, allowed
                        )));
                    }
                } else {
                    return Err(ArgParseError::InvalidParameter(format!(
                        "{} は文字列である必要があります",
                        def.name
                    )));
                }
            }
        }

        // min/max 範囲チェック（数値型のみ）
        if matches!(def.arg_type, ArgType::Int | ArgType::Float) {
            if let Ok(n) = value_to_f64(value, &def.name) {
                if let Some(min) = def.min {
                    if n < min {
                        return Err(ArgParseError::InvalidParameter(format!(
                            "{} の値 {} は最小値 {} 未満です",
                            def.name, n, min
                        )));
                    }
                }
                if let Some(max) = def.max {
                    if n > max {
                        return Err(ArgParseError::InvalidParameter(format!(
                            "{} の値 {} は最大値 {} を超えています",
                            def.name, n, max
                        )));
                    }
                }
            }
        }
    }

    Ok(ParsedArgs { inner, positional })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{ArgDef, ArgType};

    fn make_defs() -> Vec<ArgDef> {
        vec![
            ArgDef {
                name: "width".into(),
                description: "幅".into(),
                required: true,
                arg_type: ArgType::Int,
                short: None,
                min: Some(1.0),
                max: Some(10000.0),
                default_value: None,
                values: None,
                step: None,
            },
            ArgDef {
                name: "height".into(),
                description: "高さ".into(),
                required: false,
                arg_type: ArgType::Int,
                short: None,
                min: None,
                max: None,
                default_value: Some(JsonValue::Number(100.0)),
                values: None,
                step: None,
            },
            ArgDef {
                name: "mode".into(),
                description: "モード".into(),
                required: false,
                arg_type: ArgType::Enum,
                short: None,
                min: None,
                max: None,
                default_value: None,
                values: Some(vec!["fast".into(), "quality".into()]),
                step: None,
            },
        ]
    }

    fn obj(fields: Vec<(&str, JsonValue)>) -> JsonValue {
        JsonValue::Object(
            fields
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        )
    }

    // --- 12 件以上のテスト ---

    #[test]
    fn test_required_param_present() {
        let params = obj(vec![
            ("width", JsonValue::Number(200.0)),
            (
                "_positional",
                JsonValue::Array(vec![JsonValue::String("sub".into())]),
            ),
        ]);
        let result = parse_and_validate(&params, &make_defs()).unwrap();
        assert_eq!(result.get_u32("width").unwrap(), 200);
        assert_eq!(result.positional(), &["sub"]);
    }

    #[test]
    fn test_required_param_missing() {
        let params = obj(vec![]);
        let result = parse_and_validate(&params, &make_defs());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("width"));
    }

    #[test]
    fn test_default_value_applied() {
        let params = obj(vec![("width", JsonValue::Number(50.0))]);
        let result = parse_and_validate(&params, &make_defs()).unwrap();
        assert_eq!(result.get_u32("height").unwrap(), 100);
    }

    #[test]
    fn test_enum_valid() {
        let params = obj(vec![
            ("width", JsonValue::Number(50.0)),
            ("mode", JsonValue::String("fast".into())),
        ]);
        let result = parse_and_validate(&params, &make_defs()).unwrap();
        assert_eq!(result.get_str("mode").unwrap(), "fast");
    }

    #[test]
    fn test_enum_invalid() {
        let params = obj(vec![
            ("width", JsonValue::Number(50.0)),
            ("mode", JsonValue::String("invalid".into())),
        ]);
        let result = parse_and_validate(&params, &make_defs());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("mode"));
    }

    #[test]
    fn test_min_range_violation() {
        let params = obj(vec![("width", JsonValue::Number(0.0))]);
        let result = parse_and_validate(&params, &make_defs());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("最小値"));
    }

    #[test]
    fn test_max_range_violation() {
        let params = obj(vec![("width", JsonValue::Number(99999.0))]);
        let result = parse_and_validate(&params, &make_defs());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("最大値"));
    }

    #[test]
    fn test_string_to_number_conversion() {
        let params = obj(vec![("width", JsonValue::String("200".into()))]);
        let result = parse_and_validate(&params, &make_defs()).unwrap();
        assert_eq!(result.get_u32("width").unwrap(), 200);
    }

    #[test]
    fn test_unknown_params_preserved() {
        let params = obj(vec![
            ("width", JsonValue::Number(50.0)),
            ("extra", JsonValue::String("kept".into())),
        ]);
        let result = parse_and_validate(&params, &make_defs()).unwrap();
        assert_eq!(result.get_str("extra").unwrap(), "kept");
    }

    #[test]
    fn test_to_json_value_roundtrip() {
        let params = obj(vec![
            ("width", JsonValue::Number(50.0)),
            (
                "_positional",
                JsonValue::Array(vec![
                    JsonValue::String("a".into()),
                    JsonValue::String("b".into()),
                ]),
            ),
        ]);
        let result = parse_and_validate(&params, &make_defs()).unwrap();
        let v = result.to_json_value();
        assert_eq!(v.get("width"), Some(&JsonValue::Number(50.0)));
        // _positional は配列として格納されている
        let pos = v.get("_positional").unwrap().as_array().unwrap();
        assert_eq!(pos.len(), 2);
        assert_eq!(pos[0].as_str(), Some("a"));
        assert_eq!(pos[1].as_str(), Some("b"));
    }

    #[test]
    fn test_optional_accessors() {
        let params = obj(vec![("width", JsonValue::Number(50.0))]);
        let result = parse_and_validate(&params, &make_defs()).unwrap();
        assert!(result.get_opt_str("mode").is_none());
        assert_eq!(result.get_opt_u32("height"), Some(100)); // default applied
    }

    #[test]
    fn test_empty_defs_passthrough() {
        let params = obj(vec![
            ("x", JsonValue::Number(10.0)),
            ("y", JsonValue::Number(20.0)),
            (
                "_positional",
                JsonValue::Array(vec![JsonValue::String("sub".into())]),
            ),
        ]);
        let result = parse_and_validate(&params, &[]).unwrap();
        assert_eq!(result.get_u32("x").unwrap(), 10);
        assert_eq!(result.positional(), &["sub"]);
    }

    #[test]
    fn test_bool_param() {
        let defs = vec![ArgDef {
            name: "verbose".into(),
            description: "verbosity".into(),
            required: false,
            arg_type: ArgType::Bool,
            short: Some('v'),
            min: None,
            max: None,
            default_value: Some(JsonValue::Bool(false)),
            values: None,
            step: None,
        }];
        let params = obj(vec![("verbose", JsonValue::Bool(true))]);
        let result = parse_and_validate(&params, &defs).unwrap();
        assert!(result.get_bool("verbose").unwrap());
    }

    #[test]
    fn test_float_param() {
        let defs = vec![ArgDef {
            name: "scale".into(),
            description: "倍率".into(),
            required: true,
            arg_type: ArgType::Float,
            short: None,
            min: Some(0.0),
            max: Some(100.0),
            default_value: None,
            values: None,
            step: None,
        }];
        let params = obj(vec![("scale", JsonValue::Number(2.5))]);
        let result = parse_and_validate(&params, &defs).unwrap();
        let v = result.get_f64("scale").unwrap();
        assert!((v - 2.5).abs() < 1e-10);
    }

    #[test]
    fn test_missing_required_error_type() {
        let params = obj(vec![]);
        let err = parse_and_validate(&params, &make_defs()).unwrap_err();
        assert!(matches!(err, ArgParseError::MissingRequired(_)));
        if let ArgParseError::MissingRequired(name) = err {
            assert_eq!(name, "width");
        }
    }
}
