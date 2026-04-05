use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Base58Error {
    InvalidCharacter(char),
}

impl fmt::Display for Base58Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Base58Error::InvalidCharacter(ch) => {
                write!(f, "invalid Base58 character: {:?}", ch)
            }
        }
    }
}

impl std::error::Error for Base58Error {}
