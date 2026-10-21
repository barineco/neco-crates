#![doc = "neco-json を利用する CLI 引数パーサーおよびバリデーターです。"]
#![doc = "[ArgDef] はコマンド引数を定義します。"]
#![doc = "[ArgType] は引数の型を表します。"]
#![doc = "[CommandMeta] はコマンドスキーマを表します。"]
#![doc = "[parse_and_validate] は JSON 値を検証します。"]
#![doc = "[parse_cli_args] は生の CLI 引数を検証済みの JSON 値に変換します。"]

mod args;
mod cli;
mod error;
mod parsed;
mod validate;

pub use args::{ArgDef, ArgType, CommandMeta};
pub use cli::{parse_cli_args, CliParsed};
pub use error::ArgParseError;
pub use parsed::ParsedArgs;
pub use validate::parse_and_validate;
