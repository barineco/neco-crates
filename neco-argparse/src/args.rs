use neco_json::JsonValue;

/// 引数の型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgType {
    Int,
    Float,
    String,
    Bool,
    Enum,
}

/// コマンド引数の定義
#[derive(Debug, Clone)]
pub struct ArgDef {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub arg_type: ArgType,
    /// CLI の短縮フラグ（例: `Some('m')` → `-m`）
    pub short: Option<char>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub default_value: Option<JsonValue>,
    pub values: Option<Vec<String>>,
    pub step: Option<f64>,
}

/// コマンドメタデータ（GUI 非依存）
#[derive(Debug, Clone)]
pub struct CommandMeta {
    pub name: String,
    pub description: String,
    pub args: Vec<ArgDef>,
}
