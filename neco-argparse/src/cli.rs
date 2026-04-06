use neco_json::JsonValue;

use crate::args::CommandMeta;
use crate::error::ArgParseError;

/// CLI パース結果
#[derive(Debug)]
pub struct CliParsed {
    /// サブコマンド名
    pub command: String,
    /// パラメータ（JsonValue::Object）
    pub params: JsonValue,
    /// `--help` / `-h` が指定されたかどうか
    pub help_requested: bool,
}

/// CLI 引数をパースし、サブコマンド名と JsonValue パラメータに分解する。
///
/// - `metas`: 登録済みコマンドの CommandMeta 一覧（短縮フラグの解決に使用）
///
/// パース規則:
/// - 最初の非フラグ引数がサブコマンド名
/// - `--key value` / `--key=value` → `{ "key": "value" }`
/// - `-k value` → CommandMeta の ArgDef.short から long name を解決して `{ "long_name": "value" }`
/// - 残りの位置引数 → `_positional` 配列
/// - `--help` / `-h` → `help_requested = true`
pub fn parse_cli_args(args: &[String], metas: &[CommandMeta]) -> Result<CliParsed, ArgParseError> {
    let mut iter = args.iter().peekable();

    // サブコマンド名を探す（最初の非フラグ引数）
    let command = loop {
        match iter.next() {
            None => {
                return Err(ArgParseError::InvalidParameter(
                    "サブコマンドが指定されていません".to_string(),
                ));
            }
            Some(arg) if !arg.starts_with('-') => break arg.clone(),
            Some(_) => {
                // サブコマンドより前のフラグは無視する（--help 等は後段で処理）
            }
        }
    };

    // サブコマンドに対応する CommandMeta を探す（短縮フラグ解決に使用）
    let meta = metas.iter().find(|m| m.name == command);

    let mut fields: Vec<(String, JsonValue)> = Vec::new();
    let mut positional: Vec<JsonValue> = Vec::new();
    let mut help_requested = false;

    while let Some(arg) = iter.next() {
        if arg == "--help" || arg == "-h" {
            help_requested = true;
            continue;
        }

        if let Some(rest) = arg.strip_prefix("--") {
            // --key=value または --key value
            if let Some(eq_pos) = rest.find('=') {
                let key = rest[..eq_pos].to_string();
                let val = rest[eq_pos + 1..].to_string();
                fields.push((key, JsonValue::String(val)));
            } else {
                let key = rest.to_string();
                // 次のトークンが値（フラグでなければ）
                let val = if iter.peek().map(|s| !s.starts_with('-')).unwrap_or(false) {
                    iter.next().unwrap().clone()
                } else {
                    // 値なし → boolean フラグとして true
                    fields.push((key, JsonValue::Bool(true)));
                    continue;
                };
                fields.push((key, JsonValue::String(val)));
            }
        } else if let Some(short_rest) = arg.strip_prefix('-') {
            // -k value（単一文字）
            let chars: Vec<char> = short_rest.chars().collect();
            if chars.len() == 1 {
                let short_char = chars[0];
                // CommandMeta から long name を解決
                let long_name = meta
                    .and_then(|m| {
                        m.args
                            .iter()
                            .find(|a| a.short == Some(short_char))
                            .map(|a| a.name.clone())
                    })
                    .unwrap_or_else(|| short_char.to_string());

                let val = if iter.peek().map(|s| !s.starts_with('-')).unwrap_or(false) {
                    iter.next().unwrap().clone()
                } else {
                    fields.push((long_name, JsonValue::Bool(true)));
                    continue;
                };
                fields.push((long_name, JsonValue::String(val)));
            } else {
                // 複数文字の短縮フラグ（-abc など）は位置引数として扱う
                positional.push(JsonValue::String(arg.clone()));
            }
        } else {
            // 位置引数
            positional.push(JsonValue::String(arg.clone()));
        }
    }

    if !positional.is_empty() {
        fields.push(("_positional".to_string(), JsonValue::Array(positional)));
    }

    Ok(CliParsed {
        command,
        params: JsonValue::Object(fields),
        help_requested,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{ArgDef, ArgType};

    fn s(v: &str) -> String {
        v.to_string()
    }

    fn no_metas() -> Vec<CommandMeta> {
        vec![]
    }

    fn meta_with_short(cmd: &str, long: &str, short: char) -> Vec<CommandMeta> {
        vec![CommandMeta {
            name: cmd.to_string(),
            description: String::new(),
            args: vec![ArgDef {
                name: long.to_string(),
                description: String::new(),
                required: false,
                arg_type: ArgType::String,
                short: Some(short),
                min: None,
                max: None,
                default_value: None,
                values: None,
                step: None,
            }],
        }]
    }

    #[test]
    fn test_basic_subcommand() {
        let args = vec![s("init")];
        let result = parse_cli_args(&args, &no_metas()).unwrap();
        assert_eq!(result.command, "init");
        assert!(!result.help_requested);
    }

    #[test]
    fn test_long_flag_with_space() {
        let args = vec![s("snapshot"), s("--message"), s("hello world")];
        let result = parse_cli_args(&args, &no_metas()).unwrap();
        assert_eq!(result.command, "snapshot");
        assert_eq!(
            result.params.get("message"),
            Some(&JsonValue::String("hello world".into()))
        );
    }

    #[test]
    fn test_long_flag_with_equals() {
        let args = vec![s("log"), s("--count=5")];
        let result = parse_cli_args(&args, &no_metas()).unwrap();
        assert_eq!(result.command, "log");
        assert_eq!(
            result.params.get("count"),
            Some(&JsonValue::String("5".into()))
        );
    }

    #[test]
    fn test_help_flag() {
        let args = vec![s("diff"), s("--help")];
        let result = parse_cli_args(&args, &no_metas()).unwrap();
        assert!(result.help_requested);
    }

    #[test]
    fn test_short_help_flag() {
        let args = vec![s("diff"), s("-h")];
        let result = parse_cli_args(&args, &no_metas()).unwrap();
        assert!(result.help_requested);
    }

    #[test]
    fn test_positional_args() {
        let args = vec![s("push"), s("origin"), s("main")];
        let result = parse_cli_args(&args, &no_metas()).unwrap();
        assert_eq!(result.command, "push");
        let pos = result
            .params
            .get("_positional")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(pos.len(), 2);
        assert_eq!(pos[0].as_str(), Some("origin"));
        assert_eq!(pos[1].as_str(), Some("main"));
    }

    #[test]
    fn test_short_flag_resolved_to_long() {
        let metas = meta_with_short("snapshot", "message", 'm');
        let args = vec![s("snapshot"), s("-m"), s("my commit message")];
        let result = parse_cli_args(&args, &metas).unwrap();
        assert_eq!(
            result.params.get("message"),
            Some(&JsonValue::String("my commit message".into()))
        );
    }

    #[test]
    fn test_boolean_flag_no_value() {
        let args = vec![s("log"), s("--verbose")];
        let result = parse_cli_args(&args, &no_metas()).unwrap();
        assert_eq!(result.params.get("verbose"), Some(&JsonValue::Bool(true)));
    }

    #[test]
    fn test_no_subcommand_error() {
        let args: Vec<String> = vec![];
        let result = parse_cli_args(&args, &no_metas());
        assert!(result.is_err());
    }

    #[test]
    fn test_mixed_flags_and_positional() {
        // --stat が bool フラグ（次トークンは別の -- フラグではなく値として解釈される）
        // positional は -- フラグの後ろにない引数
        let args = vec![s("diff"), s("abc123"), s("def456"), s("--stat")];
        let result = parse_cli_args(&args, &no_metas()).unwrap();
        assert_eq!(result.command, "diff");
        assert_eq!(result.params.get("stat"), Some(&JsonValue::Bool(true)));
        let pos = result
            .params
            .get("_positional")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(pos.len(), 2);
        assert_eq!(pos[0].as_str(), Some("abc123"));
        assert_eq!(pos[1].as_str(), Some("def456"));
    }
}
