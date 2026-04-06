use std::fmt;

/// コマンド引数パース・バリデーションエラー
#[derive(Debug, PartialEq)]
pub enum ArgParseError {
    /// パラメータの値が不正、または型変換に失敗した
    InvalidParameter(String),
    /// 必須パラメータが指定されていない
    MissingRequired(String),
}

impl fmt::Display for ArgParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParameter(msg) => write!(f, "パラメータエラー: {}", msg),
            Self::MissingRequired(name) => write!(f, "必須パラメータがありません: {}", name),
        }
    }
}

impl std::error::Error for ArgParseError {}
