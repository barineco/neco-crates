//! CLI argument parser and validator backed by neco-json.
//!
//! Provides [ArgDef], [ArgType], [CommandMeta] for defining command schemas,
//! [parse_and_validate] for validating [JsonValue] parameters against those schemas,
//! and [parse_cli_args] for converting raw CLI arguments into validated [JsonValue].

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
