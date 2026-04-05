/// KDL v2 ドキュメント。ゼロ個以上のノードで構成される。
#[derive(Debug, Clone, PartialEq)]
pub struct KdlDocument {
    pub(crate) nodes: Vec<KdlNode>,
}

impl KdlDocument {
    /// ドキュメント内のノード一覧を返す。
    pub fn nodes(&self) -> &[KdlNode] {
        &self.nodes
    }
}

/// KDL v2 ノード。名前、エントリ群（argument + property）、子ノードを持つ。
#[derive(Debug, Clone, PartialEq)]
pub struct KdlNode {
    /// type annotation `(type)name`
    pub(crate) ty: Option<String>,
    /// ノード名
    pub(crate) name: String,
    /// argument と property を出現順で保持
    pub(crate) entries: Vec<KdlEntry>,
    /// children block `{ ... }`
    pub(crate) children: Option<Vec<KdlNode>>,
}

impl KdlNode {
    /// type annotation を返す。
    pub fn ty(&self) -> Option<&str> {
        self.ty.as_deref()
    }

    /// ノード名を返す。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// エントリ一覧を返す。
    pub fn entries(&self) -> &[KdlEntry] {
        &self.entries
    }

    /// 子ノードを返す。
    pub fn children(&self) -> Option<&[KdlNode]> {
        self.children.as_deref()
    }
}

/// ノードのエントリ。argument（位置引数）または property（名前付き引数）。
#[derive(Debug, Clone, PartialEq)]
pub enum KdlEntry {
    Argument {
        ty: Option<String>,
        value: KdlValue,
    },
    Property {
        key: String,
        ty: Option<String>,
        value: KdlValue,
    },
}

/// KDL v2 の値。
#[derive(Debug, Clone, PartialEq)]
pub enum KdlValue {
    String(String),
    Number(KdlNumber),
    Bool(bool),
    Null,
}

/// 数値の raw 文字列を保持しつつ、可能な場合は解釈済み値も提供する。
///
/// KDL v2 は数値サイズに制限を置かないため、i64/f64 に収まらない値も受理する。
#[derive(Debug, Clone)]
pub struct KdlNumber {
    /// 原文（アンダースコア・プレフィックス含む）
    pub(crate) raw: String,
    /// 整数として解釈可能な場合
    pub(crate) as_i64: Option<i64>,
    /// 浮動小数点として解釈可能な場合（#inf, #-inf, #nan 含む）
    pub(crate) as_f64: Option<f64>,
}

impl KdlNumber {
    /// 原文を返す。
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// 整数として解釈可能な場合の値を返す。
    pub fn as_i64(&self) -> Option<i64> {
        self.as_i64
    }

    /// 浮動小数点として解釈可能な場合の値を返す。
    pub fn as_f64(&self) -> Option<f64> {
        self.as_f64
    }
}

impl PartialEq for KdlNumber {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

/// パースエラー。
#[derive(Debug, Clone, PartialEq)]
pub struct KdlError {
    /// 1-based 行番号
    pub(crate) line: usize,
    /// 1-based 列番号（Unicode scalar value 単位）
    pub(crate) col: usize,
    /// エラー種別
    pub(crate) kind: KdlErrorKind,
}

impl KdlError {
    /// 行番号（1-based）を返す。
    pub fn line(&self) -> usize {
        self.line
    }

    /// 列番号（1-based、Unicode scalar value 単位）を返す。
    pub fn col(&self) -> usize {
        self.col
    }

    /// エラー種別を返す。
    pub fn kind(&self) -> &KdlErrorKind {
        &self.kind
    }
}

impl core::fmt::Display for KdlError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.kind)
    }
}

/// エラー種別。
#[derive(Debug, Clone, PartialEq)]
pub enum KdlErrorKind {
    /// 予期しない文字
    UnexpectedChar(char),
    /// 予期しない EOF
    UnexpectedEof,
    /// 不正な文字列エスケープ
    InvalidEscape,
    /// 不正な Unicode エスケープ
    InvalidUnicodeEscape,
    /// 不正な数値リテラル
    InvalidNumber,
    /// 禁止コードポイント
    DisallowedCodePoint(char),
    /// 裸キーワード（true, false, null, inf, -inf, nan）
    BareKeyword,
    /// ネストされていないブロックコメント終端
    UnmatchedBlockCommentEnd,
    /// 閉じられていないブロックコメント
    UnclosedBlockComment,
    /// 閉じられていない文字列
    UnclosedString,
    /// multiline string のインデント不一致
    InconsistentIndentation,
    /// slashdash の不正な位置
    InvalidSlashdash,
}

impl core::fmt::Display for KdlErrorKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnexpectedChar(c) => write!(f, "unexpected character: {:?}", c),
            Self::UnexpectedEof => write!(f, "unexpected end of input"),
            Self::InvalidEscape => write!(f, "invalid escape sequence"),
            Self::InvalidUnicodeEscape => write!(f, "invalid unicode escape"),
            Self::InvalidNumber => write!(f, "invalid number literal"),
            Self::DisallowedCodePoint(c) => {
                write!(f, "disallowed code point: U+{:04X}", *c as u32)
            }
            Self::BareKeyword => write!(f, "bare keyword (use #true, #false, #null, etc.)"),
            Self::UnmatchedBlockCommentEnd => write!(f, "unmatched */"),
            Self::UnclosedBlockComment => write!(f, "unclosed block comment"),
            Self::UnclosedString => write!(f, "unclosed string"),
            Self::InconsistentIndentation => write!(f, "inconsistent multiline string indentation"),
            Self::InvalidSlashdash => write!(f, "slashdash in invalid position"),
        }
    }
}
