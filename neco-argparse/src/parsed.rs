use std::collections::HashMap;

use neco_json::JsonValue;

use crate::error::ArgParseError;

/// パース・バリデーション済みのコマンド引数
#[derive(Debug)]
pub struct ParsedArgs {
    pub(crate) inner: HashMap<String, JsonValue>,
    pub(crate) positional: Vec<String>,
}

impl ParsedArgs {
    // --- required アクセサ ---

    pub fn get_u32(&self, name: &str) -> Result<u32, ArgParseError> {
        let v = self
            .inner
            .get(name)
            .ok_or_else(|| ArgParseError::InvalidParameter(format!("{} が必要です", name)))?;
        value_to_u32(v, name)
    }

    pub fn get_f64(&self, name: &str) -> Result<f64, ArgParseError> {
        let v = self
            .inner
            .get(name)
            .ok_or_else(|| ArgParseError::InvalidParameter(format!("{} が必要です", name)))?;
        value_to_f64(v, name)
    }

    pub fn get_f32(&self, name: &str) -> Result<f32, ArgParseError> {
        self.get_f64(name).map(|v| v as f32)
    }

    pub fn get_bool(&self, name: &str) -> Result<bool, ArgParseError> {
        let v = self
            .inner
            .get(name)
            .ok_or_else(|| ArgParseError::InvalidParameter(format!("{} が必要です", name)))?;
        value_to_bool(v, name)
    }

    pub fn get_str(&self, name: &str) -> Result<&str, ArgParseError> {
        let v = self
            .inner
            .get(name)
            .ok_or_else(|| ArgParseError::InvalidParameter(format!("{} が必要です", name)))?;
        v.as_str().ok_or_else(|| {
            ArgParseError::InvalidParameter(format!("{} は文字列である必要があります", name))
        })
    }

    // --- optional アクセサ ---

    pub fn get_opt_u32(&self, name: &str) -> Option<u32> {
        self.inner
            .get(name)
            .and_then(|v| value_to_u32(v, name).ok())
    }

    pub fn get_opt_f64(&self, name: &str) -> Option<f64> {
        self.inner
            .get(name)
            .and_then(|v| value_to_f64(v, name).ok())
    }

    pub fn get_opt_f32(&self, name: &str) -> Option<f32> {
        self.get_opt_f64(name).map(|v| v as f32)
    }

    pub fn get_opt_bool(&self, name: &str) -> Option<bool> {
        self.inner
            .get(name)
            .and_then(|v| value_to_bool(v, name).ok())
    }

    pub fn get_opt_str(&self, name: &str) -> Option<&str> {
        self.inner.get(name).and_then(|v| v.as_str())
    }

    pub fn get_json(&self, name: &str) -> Option<&JsonValue> {
        self.inner.get(name)
    }

    pub fn positional(&self) -> &[String] {
        &self.positional
    }

    /// inner を JsonValue::Object に再構築する（デバッグ・シリアライズ用）
    pub fn to_json_value(&self) -> JsonValue {
        let mut fields: Vec<(String, JsonValue)> = self
            .inner
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let pos = JsonValue::Array(
            self.positional
                .iter()
                .map(|s| JsonValue::String(s.clone()))
                .collect(),
        );
        fields.push(("_positional".to_string(), pos));
        JsonValue::Object(fields)
    }
}

// --- 型変換ヘルパー ---

pub(crate) fn value_to_u32(v: &JsonValue, name: &str) -> Result<u32, ArgParseError> {
    if let Some(n) = v.as_f64() {
        if n.fract() == 0.0 && n >= 0.0 && n <= u32::MAX as f64 {
            Ok(n as u32)
        } else {
            Err(ArgParseError::InvalidParameter(format!(
                "{} が u32 の範囲外です: {}",
                name, n
            )))
        }
    } else if let Some(s) = v.as_str() {
        s.parse::<u32>().map_err(|_| {
            ArgParseError::InvalidParameter(format!("{} を整数に変換できません: {}", name, s))
        })
    } else {
        Err(ArgParseError::InvalidParameter(format!(
            "{} を整数に変換できません",
            name
        )))
    }
}

pub(crate) fn value_to_f64(v: &JsonValue, name: &str) -> Result<f64, ArgParseError> {
    if let Some(n) = v.as_f64() {
        Ok(n)
    } else if let Some(s) = v.as_str() {
        s.parse::<f64>().map_err(|_| {
            ArgParseError::InvalidParameter(format!("{} を数値に変換できません: {}", name, s))
        })
    } else {
        Err(ArgParseError::InvalidParameter(format!(
            "{} を数値に変換できません",
            name
        )))
    }
}

pub(crate) fn value_to_bool(v: &JsonValue, name: &str) -> Result<bool, ArgParseError> {
    if let Some(b) = v.as_bool() {
        Ok(b)
    } else if let Some(s) = v.as_str() {
        match s {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(ArgParseError::InvalidParameter(format!(
                "{} を真偽値に変換できません: {}",
                name, s
            ))),
        }
    } else {
        Err(ArgParseError::InvalidParameter(format!(
            "{} を真偽値に変換できません",
            name
        )))
    }
}
