use crate::scan::{is_newline, is_unicode_space, Scanner};
use crate::{KdlError, KdlErrorKind};

/// `//` から改行または EOF までスキップする。`//` は既に消費済みの前提。
pub(crate) fn skip_single_line_comment(s: &mut Scanner<'_>) -> Result<(), KdlError> {
    while let Some(ch) = s.peek() {
        if is_newline(ch) {
            break;
        }
        s.advance();
    }
    Ok(())
}

/// `/* */` のネスト対応ブロックコメントをスキップする。`/*` は既に消費済みの前提。
pub(crate) fn skip_block_comment(s: &mut Scanner<'_>) -> Result<(), KdlError> {
    let mut depth: usize = 1;
    while depth > 0 {
        match s.advance() {
            Some('/') => {
                if s.peek() == Some('*') {
                    s.advance();
                    depth += 1;
                }
            }
            Some('*') => {
                if s.peek() == Some('/') {
                    s.advance();
                    depth -= 1;
                }
            }
            Some(_) => {}
            None => return Err(s.make_error(KdlErrorKind::UnclosedBlockComment)),
        }
    }
    Ok(())
}

/// ws をスキップする。ws := unicode-space | multi-line-comment
pub(crate) fn skip_ws(s: &mut Scanner<'_>) -> Result<(), KdlError> {
    loop {
        match s.peek() {
            Some(ch) if is_unicode_space(ch) => {
                s.advance();
            }
            Some('/') if s.peek_next() == Some('*') => {
                s.advance(); // /
                s.advance(); // *
                skip_block_comment(s)?;
            }
            _ => break,
        }
    }
    Ok(())
}

/// line-space := node-space | newline | single-line-comment
pub(crate) fn skip_line_space(s: &mut Scanner<'_>) -> Result<(), KdlError> {
    loop {
        match s.peek() {
            Some(ch) if is_unicode_space(ch) => {
                s.advance();
            }
            Some(ch) if is_newline(ch) => {
                s.consume_newline();
            }
            Some('/') => match s.peek_next() {
                Some('/') => {
                    s.advance(); // /
                    s.advance(); // /
                    skip_single_line_comment(s)?;
                }
                Some('*') => {
                    s.advance(); // /
                    s.advance(); // *
                    skip_block_comment(s)?;
                }
                _ => break,
            },
            Some('\\') => {
                // escline の可能性: \ ws* (single-line-comment | newline | eof)
                if try_skip_escline(s)? {
                    continue;
                }
                break;
            }
            _ => break,
        }
    }
    Ok(())
}

/// node-space := ws* (escline ws*)+ | ws+
/// 最低 1 つの ws または escline を消費する必要がある。
/// 呼び出し元がノード内のスペース区切りを期待する場合に使う。
/// 何も消費できなかった場合は false を返す。
pub(crate) fn skip_node_space(s: &mut Scanner<'_>) -> Result<bool, KdlError> {
    let start = s.byte_offset();
    skip_ws(s)?;
    let had_ws = s.byte_offset() != start;

    let mut had_escline = false;
    while try_skip_escline(s)? {
        had_escline = true;
        skip_ws(s)?;
    }

    Ok(had_ws || had_escline)
}

/// escline := '\' ws* (single-line-comment | newline | eof)
/// 成功時は true を返し、消費する。失敗時は位置を復元して false を返す。
fn try_skip_escline(s: &mut Scanner<'_>) -> Result<bool, KdlError> {
    if s.peek() != Some('\\') {
        return Ok(false);
    }

    let saved = s.save();
    s.advance(); // consume '\'
    skip_ws(s)?;

    match s.peek() {
        Some('/') if s.peek_next() == Some('/') => {
            s.advance();
            s.advance();
            skip_single_line_comment(s)?;
            // single-line comment はコメント後に改行/EOF を含む
            // consume_newline は skip_single_line_comment が行わないので呼ぶ
            s.consume_newline();
            Ok(true)
        }
        Some(ch) if is_newline(ch) => {
            s.consume_newline();
            Ok(true)
        }
        None => {
            // EOF — escline は EOF で終了可能
            Ok(true)
        }
        _ => {
            s.restore(saved);
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_comment() {
        let mut s = Scanner::new("hello\nworld");
        skip_single_line_comment(&mut s).unwrap();
        assert_eq!(s.peek(), Some('\n'));
    }

    #[test]
    fn block_comment_simple() {
        let mut s = Scanner::new(" comment */ rest");
        skip_block_comment(&mut s).unwrap();
        assert_eq!(s.peek(), Some(' '));
    }

    #[test]
    fn block_comment_nested() {
        let mut s = Scanner::new(" outer /* inner */ still */ rest");
        skip_block_comment(&mut s).unwrap();
        assert_eq!(s.peek(), Some(' '));
        // " rest" が残る
    }

    #[test]
    fn block_comment_deeply_nested() {
        // skip_block_comment は `/*` 消費済みの前提で呼ぶ
        // ネスト深度 3: /* a /* b /* c */ */ */
        let mut s = Scanner::new(" a /* b /* c */ */ */x");
        skip_block_comment(&mut s).unwrap();
        assert_eq!(s.peek(), Some('x'));
    }

    #[test]
    fn unclosed_block_comment() {
        let mut s = Scanner::new(" unclosed");
        let err = skip_block_comment(&mut s).unwrap_err();
        assert_eq!(err.kind, KdlErrorKind::UnclosedBlockComment);
    }

    #[test]
    fn ws_skips_space_and_block_comments() {
        let mut s = Scanner::new("  /* comment */  x");
        skip_ws(&mut s).unwrap();
        assert_eq!(s.peek(), Some('x'));
    }

    #[test]
    fn escline_backslash_newline() {
        let mut s = Scanner::new("\\\nrest");
        let consumed = skip_node_space(&mut s).unwrap();
        assert!(consumed);
        assert_eq!(s.peek(), Some('r'));
    }

    #[test]
    fn escline_backslash_comment_newline() {
        let mut s = Scanner::new("\\ // comment\nrest");
        let consumed = skip_node_space(&mut s).unwrap();
        assert!(consumed);
        assert_eq!(s.peek(), Some('r'));
    }
}
