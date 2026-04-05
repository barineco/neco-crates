mod comment;
pub mod normalize;
mod number;
mod parse;
mod scan;
mod string;
mod value;

pub use normalize::normalize;
pub use parse::parse;
pub use value::{KdlDocument, KdlEntry, KdlError, KdlErrorKind, KdlNode, KdlNumber, KdlValue};
