/// UTF-8 文字列のスキャナー。行・列位置を追跡しながら char 単位で走査する。
pub(crate) struct Scanner<'a> {
    input: &'a str,
    chars: core::str::CharIndices<'a>,
    current: Option<(usize, char)>,
    line: usize,
    col: usize,
}

impl<'a> Scanner<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        let mut chars = input.char_indices();
        let current = chars.next();
        Self {
            input,
            chars,
            current,
            line: 1,
            col: 1,
        }
    }

    pub(crate) fn peek(&self) -> Option<char> {
        self.current.map(|(_, ch)| ch)
    }

    pub(crate) fn peek_next(&self) -> Option<char> {
        let mut clone = self.chars.clone();
        clone.next().map(|(_, ch)| ch)
    }

    pub(crate) fn advance(&mut self) -> Option<char> {
        let (_, ch) = self.current?;
        if is_newline(ch) {
            // CR+LF は consume_newline で 1 改行として処理する。
            // advance 単独で CR を踏んだ場合も行を進める。
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        self.current = self.chars.next();
        Some(ch)
    }

    pub(crate) fn expect(&mut self, expected: char) -> Result<(), crate::KdlError> {
        match self.peek() {
            Some(ch) if ch == expected => {
                self.advance();
                Ok(())
            }
            Some(ch) => Err(self.make_error(crate::KdlErrorKind::UnexpectedChar(ch))),
            None => Err(self.make_error(crate::KdlErrorKind::UnexpectedEof)),
        }
    }

    #[cfg(test)]
    pub(crate) fn position(&self) -> (usize, usize) {
        (self.line, self.col)
    }

    pub(crate) fn is_eof(&self) -> bool {
        self.current.is_none()
    }

    pub(crate) fn make_error(&self, kind: crate::KdlErrorKind) -> crate::KdlError {
        crate::KdlError {
            line: self.line,
            col: self.col,
            kind,
        }
    }

    pub(crate) fn byte_offset(&self) -> usize {
        match self.current {
            Some((idx, _)) => idx,
            None => self.input.len(),
        }
    }

    pub(crate) fn slice(&self, start: usize, end: usize) -> &'a str {
        &self.input[start..end]
    }

    pub(crate) fn skip_bom(&mut self) {
        if self.peek() == Some('\u{FEFF}') {
            self.current = self.chars.next();
            self.col = 1;
        }
    }

    /// 改行シーケンスを消費する。CRLF は単一の改行として扱う。
    pub(crate) fn consume_newline(&mut self) -> bool {
        match self.peek() {
            Some(ch) if is_newline(ch) => {
                let was_cr = ch == '\r';
                self.advance(); // 行が進む
                if was_cr && self.peek() == Some('\n') {
                    // CRLF: LF を消費するが行はもう進んでいるので col リセットのみ
                    self.current = self.chars.next();
                    self.col = 1;
                }
                true
            }
            _ => false,
        }
    }

    /// 現在位置を保存する。backtrack 用。
    pub(crate) fn save(&self) -> ScannerState<'a> {
        ScannerState {
            chars: self.chars.clone(),
            current: self.current,
            line: self.line,
            col: self.col,
        }
    }

    /// 保存した位置に戻る。
    pub(crate) fn restore(&mut self, state: ScannerState<'a>) {
        self.chars = state.chars;
        self.current = state.current;
        self.line = state.line;
        self.col = state.col;
    }
}

#[derive(Clone)]
pub(crate) struct ScannerState<'a> {
    chars: core::str::CharIndices<'a>,
    current: Option<(usize, char)>,
    line: usize,
    col: usize,
}

impl ScannerState<'_> {
    pub(crate) fn line(&self) -> usize {
        self.line
    }

    pub(crate) fn col(&self) -> usize {
        self.col
    }
}

/// KDL v2 の Unicode whitespace（改行を含まない）。18 文字。
pub(crate) fn is_unicode_space(ch: char) -> bool {
    matches!(
        ch,
        '\u{0009}' | '\u{0020}' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

/// KDL v2 の改行文字。
pub(crate) fn is_newline(ch: char) -> bool {
    matches!(
        ch,
        '\u{000A}' | '\u{000D}' | '\u{0085}' | '\u{000B}' | '\u{000C}' | '\u{2028}' | '\u{2029}'
    )
}

/// 禁止リテラルコードポイント。
/// BOM (U+FEFF) はここでは常に true。先頭の BOM は skip_bom() で処理。
pub(crate) fn is_disallowed(ch: char) -> bool {
    matches!(
        ch,
        '\u{0000}'..='\u{0008}'
            | '\u{000E}'..='\u{001F}'
            | '\u{007F}'
            | '\u{200E}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}'
    )
}

/// KDL v2 の identifier-char。
pub(crate) fn is_identifier_char(ch: char) -> bool {
    !is_unicode_space(ch)
        && !is_newline(ch)
        && !is_disallowed(ch)
        && !matches!(
            ch,
            '\\' | '/' | '(' | ')' | '{' | '}' | ';' | '[' | ']' | '"' | '#' | '='
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_space_table() {
        assert!(is_unicode_space('\t'));
        assert!(is_unicode_space(' '));
        assert!(is_unicode_space('\u{00A0}'));
        assert!(is_unicode_space('\u{3000}'));
        assert!(is_unicode_space('\u{2009}'));
        assert!(!is_unicode_space('a'));
        assert!(!is_unicode_space('\n'));
    }

    #[test]
    fn newline_chars() {
        assert!(is_newline('\n'));
        assert!(is_newline('\r'));
        assert!(is_newline('\u{0085}'));
        assert!(is_newline('\u{000B}'));
        assert!(is_newline('\u{000C}'));
        assert!(is_newline('\u{2028}'));
        assert!(is_newline('\u{2029}'));
        assert!(!is_newline(' '));
    }

    #[test]
    fn disallowed_code_points() {
        assert!(is_disallowed('\u{0000}'));
        assert!(is_disallowed('\u{0008}'));
        assert!(is_disallowed('\u{000E}'));
        assert!(is_disallowed('\u{001F}'));
        assert!(is_disallowed('\u{007F}'));
        assert!(is_disallowed('\u{200E}'));
        assert!(is_disallowed('\u{FEFF}'));
        assert!(!is_disallowed('a'));
        assert!(!is_disallowed('\n'));
    }

    #[test]
    fn identifier_char_rules() {
        assert!(is_identifier_char('a'));
        assert!(is_identifier_char('-'));
        assert!(is_identifier_char('.'));
        assert!(is_identifier_char(','));
        assert!(is_identifier_char('<'));
        assert!(!is_identifier_char('\\'));
        assert!(!is_identifier_char('/'));
        assert!(!is_identifier_char('"'));
        assert!(!is_identifier_char('#'));
        assert!(!is_identifier_char('='));
        assert!(!is_identifier_char(' '));
        assert!(!is_identifier_char('\n'));
    }

    #[test]
    fn scanner_basic_tracking() {
        let mut s = Scanner::new("ab\ncd");
        assert_eq!(s.position(), (1, 1));
        assert_eq!(s.advance(), Some('a'));
        assert_eq!(s.position(), (1, 2));
        assert_eq!(s.advance(), Some('b'));
        assert_eq!(s.position(), (1, 3));
        assert_eq!(s.advance(), Some('\n'));
        assert_eq!(s.position(), (2, 1));
        assert_eq!(s.advance(), Some('c'));
        assert_eq!(s.position(), (2, 2));
    }

    #[test]
    fn scanner_crlf_as_single_newline() {
        let mut s = Scanner::new("a\r\nb");
        assert_eq!(s.advance(), Some('a'));
        assert!(s.consume_newline());
        assert_eq!(s.position(), (2, 1));
        assert_eq!(s.peek(), Some('b'));
    }

    #[test]
    fn scanner_bom_skip() {
        let mut s = Scanner::new("\u{FEFF}hello");
        s.skip_bom();
        assert_eq!(s.peek(), Some('h'));
        assert_eq!(s.position(), (1, 1));
    }

    #[test]
    fn scanner_save_restore() {
        let mut s = Scanner::new("abc");
        s.advance();
        let saved = s.save();
        s.advance();
        s.advance();
        assert!(s.is_eof());
        s.restore(saved);
        assert_eq!(s.peek(), Some('b'));
    }
}
