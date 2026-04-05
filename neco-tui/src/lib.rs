//! 外部依存ゼロの最小 ANSI ターミナルヘルパー。
//!
//! SGR (Select Graphic Rendition) エスケープシーケンスによるテキスト装飾と、
//! 複数スタイルの合成を提供する。
//!
//! # 使い方
//!
//! ```
//! use neco_tui::{style, Color, Style};
//!
//! // 単一スタイル
//! let text = style("hello").fg(Color::Cyan).bold().to_string();
//! assert!(text.starts_with("\x1b["));
//! assert!(text.ends_with("\x1b[0m"));
//!
//! // リセットのみ
//! assert_eq!(Style::RESET, "\x1b[0m");
//! ```

use std::fmt;

/// SGR 前景色。
///
/// 標準 8 色を提供する。拡張色（256 色・RGB）はスコープ外。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

impl Color {
    /// SGR 前景色コードを返す。
    fn fg_code(self) -> u8 {
        match self {
            Self::Black => 30,
            Self::Red => 31,
            Self::Green => 32,
            Self::Yellow => 33,
            Self::Blue => 34,
            Self::Magenta => 35,
            Self::Cyan => 36,
            Self::White => 37,
        }
    }
}

/// テキスト装飾の定数。
pub struct Style;

impl Style {
    /// 全属性リセット。
    pub const RESET: &str = "\x1b[0m";
}

/// スタイル付きテキストのビルダー。
///
/// [`style`] 関数で生成し、メソッドチェーンで装飾を追加する。
/// [`Display`](fmt::Display) で ANSI エスケープ付き文字列を出力する。
///
/// # 例
///
/// ```
/// use neco_tui::{style, Color};
///
/// let s = style("error").fg(Color::Red).bold();
/// // Display で "\x1b[31;1merror\x1b[0m" を出力
/// let output = s.to_string();
/// assert!(output.contains("error"));
/// assert!(output.ends_with("\x1b[0m"));
/// ```
pub struct Styled<'a> {
    text: &'a str,
    codes: Vec<u8>,
}

impl<'a> Styled<'a> {
    /// 前景色を設定する。
    pub fn fg(mut self, color: Color) -> Self {
        self.codes.push(color.fg_code());
        self
    }

    /// 太字にする。
    pub fn bold(mut self) -> Self {
        self.codes.push(1);
        self
    }

    /// 暗くする（dim）。
    pub fn dim(mut self) -> Self {
        self.codes.push(2);
        self
    }
}

impl fmt::Display for Styled<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.codes.is_empty() {
            return f.write_str(self.text);
        }
        f.write_str("\x1b[")?;
        for (i, code) in self.codes.iter().enumerate() {
            if i > 0 {
                f.write_str(";")?;
            }
            write!(f, "{code}")?;
        }
        f.write_str("m")?;
        f.write_str(self.text)?;
        f.write_str(Style::RESET)
    }
}

/// テキストにスタイルを適用するビルダーを返す。
///
/// # 例
///
/// ```
/// use neco_tui::{style, Color};
///
/// println!("{}", style("ok").fg(Color::Green));
/// println!("{}", style("warning").fg(Color::Yellow).bold());
/// ```
pub fn style(text: &str) -> Styled<'_> {
    Styled {
        text,
        codes: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_without_codes_returns_plain_text() {
        assert_eq!(style("hello").to_string(), "hello");
    }

    #[test]
    fn single_fg_color_wraps_with_sgr() {
        let output = style("ok").fg(Color::Green).to_string();
        assert_eq!(output, "\x1b[32mok\x1b[0m");
    }

    #[test]
    fn bold_produces_code_1() {
        let output = style("title").bold().to_string();
        assert_eq!(output, "\x1b[1mtitle\x1b[0m");
    }

    #[test]
    fn combined_styles_join_with_semicolon() {
        let output = style("err").fg(Color::Red).bold().to_string();
        assert_eq!(output, "\x1b[31;1merr\x1b[0m");
    }

    #[test]
    fn dim_produces_code_2() {
        let output = style("faint").dim().to_string();
        assert_eq!(output, "\x1b[2mfaint\x1b[0m");
    }

    #[test]
    fn all_colors_produce_distinct_codes() {
        let colors = [
            (Color::Black, 30),
            (Color::Red, 31),
            (Color::Green, 32),
            (Color::Yellow, 33),
            (Color::Blue, 34),
            (Color::Magenta, 35),
            (Color::Cyan, 36),
            (Color::White, 37),
        ];
        for (color, expected) in colors {
            let output = style("x").fg(color).to_string();
            assert_eq!(output, format!("\x1b[{expected}mx\x1b[0m"));
        }
    }

    #[test]
    fn reset_constant_is_sgr_0() {
        assert_eq!(Style::RESET, "\x1b[0m");
    }
}
