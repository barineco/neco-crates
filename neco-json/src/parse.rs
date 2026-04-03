use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{ParseError, ParseErrorKind};
use crate::value::JsonValue;

const MAX_DEPTH: usize = 128;

struct Parser<'a> {
    input: &'a [u8],
    position: usize,
    depth: usize,
}

/// Parses a JSON byte slice into a `JsonValue`.
pub fn parse(input: &[u8]) -> Result<JsonValue, ParseError> {
    let mut parser = Parser {
        input,
        position: 0,
        depth: 0,
    };
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    if parser.position < parser.input.len() {
        return Err(parser.error(ParseErrorKind::TrailingContent));
    }
    Ok(value)
}

impl<'a> Parser<'a> {
    fn error(&self, kind: ParseErrorKind) -> ParseError {
        ParseError {
            kind,
            position: self.position,
        }
    }

    fn error_at(&self, position: usize, kind: ParseErrorKind) -> ParseError {
        ParseError { kind, position }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.position).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let byte = self.input.get(self.position).copied();
        if byte.is_some() {
            self.position += 1;
        }
        byte
    }

    fn expect(&mut self, expected: u8) -> Result<(), ParseError> {
        match self.advance() {
            Some(b) if b == expected => Ok(()),
            Some(b) => {
                Err(self.error_at(self.position - 1, ParseErrorKind::UnexpectedCharacter(b)))
            }
            None => Err(self.error(ParseErrorKind::UnexpectedEnd)),
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(b) = self.peek() {
            match b {
                b' ' | b'\t' | b'\n' | b'\r' => {
                    self.position += 1;
                }
                _ => break,
            }
        }
    }

    fn parse_value(&mut self) -> Result<JsonValue, ParseError> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'n') => self.parse_null(),
            Some(b't') => self.parse_true(),
            Some(b'f') => self.parse_false(),
            Some(b'"') => self.parse_string().map(JsonValue::String),
            Some(b'[') => self.parse_array(),
            Some(b'{') => self.parse_object(),
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number(),
            Some(b) => Err(self.error(ParseErrorKind::UnexpectedCharacter(b))),
            None => Err(self.error(ParseErrorKind::UnexpectedEnd)),
        }
    }

    fn parse_null(&mut self) -> Result<JsonValue, ParseError> {
        self.expect_literal(b"null")?;
        Ok(JsonValue::Null)
    }

    fn parse_true(&mut self) -> Result<JsonValue, ParseError> {
        self.expect_literal(b"true")?;
        Ok(JsonValue::Bool(true))
    }

    fn parse_false(&mut self) -> Result<JsonValue, ParseError> {
        self.expect_literal(b"false")?;
        Ok(JsonValue::Bool(false))
    }

    fn expect_literal(&mut self, literal: &[u8]) -> Result<(), ParseError> {
        for &expected in literal {
            match self.advance() {
                Some(b) if b == expected => {}
                Some(b) => {
                    return Err(
                        self.error_at(self.position - 1, ParseErrorKind::UnexpectedCharacter(b))
                    )
                }
                None => return Err(self.error(ParseErrorKind::UnexpectedEnd)),
            }
        }
        Ok(())
    }

    fn parse_number(&mut self) -> Result<JsonValue, ParseError> {
        let start = self.position;

        // optional minus
        if self.peek() == Some(b'-') {
            self.position += 1;
        }

        // integer part
        match self.peek() {
            Some(b'0') => {
                self.position += 1;
                // Digits after a leading zero are not allowed.
                // The trailing-content check catches them.
            }
            Some(b'1'..=b'9') => {
                self.position += 1;
                while let Some(b'0'..=b'9') = self.peek() {
                    self.position += 1;
                }
            }
            _ => return Err(self.error(ParseErrorKind::InvalidNumber)),
        }

        // fraction
        if self.peek() == Some(b'.') {
            self.position += 1;
            let frac_start = self.position;
            while let Some(b'0'..=b'9') = self.peek() {
                self.position += 1;
            }
            if self.position == frac_start {
                return Err(self.error(ParseErrorKind::InvalidNumber));
            }
        }

        // exponent
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.position += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.position += 1;
            }
            let exp_start = self.position;
            while let Some(b'0'..=b'9') = self.peek() {
                self.position += 1;
            }
            if self.position == exp_start {
                return Err(self.error(ParseErrorKind::InvalidNumber));
            }
        }

        let slice = &self.input[start..self.position];
        // UTF-8 safe: number literals are ASCII only.
        let s = core::str::from_utf8(slice).map_err(|_| self.error(ParseErrorKind::InvalidUtf8))?;
        let value: f64 = s
            .parse()
            .map_err(|_| self.error(ParseErrorKind::InvalidNumber))?;
        Ok(JsonValue::Number(value))
    }

    fn parse_string(&mut self) -> Result<String, ParseError> {
        self.expect(b'"')?;
        let mut buf = String::new();
        loop {
            match self.advance() {
                Some(b'"') => return Ok(buf),
                Some(b'\\') => {
                    let ch = self.parse_escape()?;
                    buf.push(ch);
                }
                Some(b) if b < 0x20 => {
                    return Err(
                        self.error_at(self.position - 1, ParseErrorKind::UnexpectedCharacter(b))
                    );
                }
                Some(b) => {
                    let width = utf8_char_width(b);
                    if width == 0 {
                        return Err(self.error(ParseErrorKind::InvalidUtf8));
                    }
                    if width == 1 {
                        buf.push(b as char);
                    } else {
                        let start = self.position - 1;
                        let mut bytes = [0_u8; 4];
                        bytes[0] = b;

                        for slot in bytes.iter_mut().take(width).skip(1) {
                            *slot = match self.advance() {
                                Some(cont) => cont,
                                None => return Err(self.error(ParseErrorKind::InvalidUtf8)),
                            };
                        }

                        if !is_valid_utf8_sequence(&bytes[..width]) {
                            return Err(self.error(ParseErrorKind::InvalidUtf8));
                        }

                        let s = core::str::from_utf8(&self.input[start..self.position])
                            .map_err(|_| self.error(ParseErrorKind::InvalidUtf8))?;
                        buf.push_str(s);
                    }
                }
                None => return Err(self.error(ParseErrorKind::UnexpectedEnd)),
            }
        }
    }

    fn parse_escape(&mut self) -> Result<char, ParseError> {
        match self.advance() {
            Some(b'"') => Ok('"'),
            Some(b'\\') => Ok('\\'),
            Some(b'/') => Ok('/'),
            Some(b'b') => Ok('\u{0008}'),
            Some(b'f') => Ok('\u{000C}'),
            Some(b'n') => Ok('\n'),
            Some(b'r') => Ok('\r'),
            Some(b't') => Ok('\t'),
            Some(b'u') => self.parse_unicode_escape(),
            Some(_) => Err(self.error_at(self.position - 1, ParseErrorKind::InvalidEscape)),
            None => Err(self.error(ParseErrorKind::UnexpectedEnd)),
        }
    }

    fn parse_unicode_escape(&mut self) -> Result<char, ParseError> {
        let cp = self.parse_hex4()?;

        // High surrogate
        if (0xD800..=0xDBFF).contains(&cp) {
            // Verify that another \u follows immediately.
            self.expect(b'\\')
                .map_err(|_| self.error(ParseErrorKind::InvalidUnicodeEscape))?;
            self.expect(b'u')
                .map_err(|_| self.error(ParseErrorKind::InvalidUnicodeEscape))?;
            let low = self.parse_hex4()?;
            if !(0xDC00..=0xDFFF).contains(&low) {
                return Err(self.error(ParseErrorKind::InvalidUnicodeEscape));
            }
            let combined = (cp - 0xD800) * 0x400 + (low - 0xDC00) + 0x10000;
            char::from_u32(combined).ok_or_else(|| self.error(ParseErrorKind::InvalidUnicodeEscape))
        } else if (0xDC00..=0xDFFF).contains(&cp) {
            // Lone low surrogate
            Err(self.error(ParseErrorKind::InvalidUnicodeEscape))
        } else {
            char::from_u32(cp).ok_or_else(|| self.error(ParseErrorKind::InvalidUnicodeEscape))
        }
    }

    fn parse_hex4(&mut self) -> Result<u32, ParseError> {
        let mut value: u32 = 0;
        for _ in 0..4 {
            let digit = match self.advance() {
                Some(b @ b'0'..=b'9') => (b - b'0') as u32,
                Some(b @ b'a'..=b'f') => (b - b'a' + 10) as u32,
                Some(b @ b'A'..=b'F') => (b - b'A' + 10) as u32,
                Some(_) => {
                    return Err(
                        self.error_at(self.position - 1, ParseErrorKind::InvalidUnicodeEscape)
                    )
                }
                None => return Err(self.error(ParseErrorKind::InvalidUnicodeEscape)),
            };
            value = value * 16 + digit;
        }
        Ok(value)
    }

    fn parse_array(&mut self) -> Result<JsonValue, ParseError> {
        self.expect(b'[')?;
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.error(ParseErrorKind::NestingTooDeep));
        }

        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.position += 1;
            self.depth -= 1;
            return Ok(JsonValue::Array(Vec::new()));
        }

        let mut items = Vec::new();
        loop {
            let value = self.parse_value()?;
            items.push(value);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.position += 1;
                }
                Some(b']') => {
                    self.position += 1;
                    self.depth -= 1;
                    return Ok(JsonValue::Array(items));
                }
                Some(b) => return Err(self.error(ParseErrorKind::UnexpectedCharacter(b))),
                None => return Err(self.error(ParseErrorKind::UnexpectedEnd)),
            }
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, ParseError> {
        self.expect(b'{')?;
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.error(ParseErrorKind::NestingTooDeep));
        }

        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.position += 1;
            self.depth -= 1;
            return Ok(JsonValue::Object(Vec::new()));
        }

        let mut fields = Vec::new();
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(match self.peek() {
                    Some(b) => self.error(ParseErrorKind::UnexpectedCharacter(b)),
                    None => self.error(ParseErrorKind::UnexpectedEnd),
                });
            }
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect(b':')?;
            let value = self.parse_value()?;
            fields.push((key, value));
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.position += 1;
                }
                Some(b'}') => {
                    self.position += 1;
                    self.depth -= 1;
                    return Ok(JsonValue::Object(fields));
                }
                Some(b) => return Err(self.error(ParseErrorKind::UnexpectedCharacter(b))),
                None => return Err(self.error(ParseErrorKind::UnexpectedEnd)),
            }
        }
    }
}

/// Returns the UTF-8 character width from the first byte.
fn utf8_char_width(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if (0xC2..=0xDF).contains(&b) {
        2
    } else if (0xE0..=0xEF).contains(&b) {
        3
    } else if (0xF0..=0xF4).contains(&b) {
        4
    } else {
        0
    }
}

fn is_valid_utf8_sequence(bytes: &[u8]) -> bool {
    match bytes {
        [first, second] => (0xC2..=0xDF).contains(first) && is_continuation_byte(*second),
        [first, second, third] => {
            (match *first {
                0xE0 => (0xA0..=0xBF).contains(second),
                0xE1..=0xEC | 0xEE..=0xEF => is_continuation_byte(*second),
                0xED => (0x80..=0x9F).contains(second),
                _ => false,
            }) && is_continuation_byte(*third)
        }
        [first, second, third, fourth] => {
            (match *first {
                0xF0 => (0x90..=0xBF).contains(second),
                0xF1..=0xF3 => is_continuation_byte(*second),
                0xF4 => (0x80..=0x8F).contains(second),
                _ => false,
            }) && is_continuation_byte(*third)
                && is_continuation_byte(*fourth)
        }
        _ => false,
    }
}

fn is_continuation_byte(b: u8) -> bool {
    (0x80..=0xBF).contains(&b)
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::vec;

    use super::parse;
    use crate::{JsonValue, ParseErrorKind};

    #[test]
    fn parses_basic_types() {
        assert_eq!(parse(b"null"), Ok(JsonValue::Null));
        assert_eq!(parse(b"true"), Ok(JsonValue::Bool(true)));
        assert_eq!(parse(b"false"), Ok(JsonValue::Bool(false)));
        assert_eq!(parse(b"0"), Ok(JsonValue::Number(0.0)));
        assert_eq!(parse(b"-12"), Ok(JsonValue::Number(-12.0)));
        assert_eq!(parse(b"3.5"), Ok(JsonValue::Number(3.5)));
        assert_eq!(parse(b"1.25e2"), Ok(JsonValue::Number(125.0)));
        assert_eq!(
            parse(br#""hello""#),
            Ok(JsonValue::String(String::from("hello")))
        );
    }

    #[test]
    fn parses_string_escapes() {
        assert_eq!(
            parse(br#""\"\\\/\b\f\n\r\t""#),
            Ok(JsonValue::String(String::from(
                "\"\\/\u{0008}\u{000C}\n\r\t"
            )))
        );
    }

    #[test]
    fn parses_unicode_escapes_and_surrogate_pairs() {
        assert_eq!(
            parse(br#""\u0041""#),
            Ok(JsonValue::String(String::from("A")))
        );
        assert_eq!(
            parse(br#""\u3042""#),
            Ok(JsonValue::String(String::from("あ")))
        );
        assert_eq!(
            parse(br#""\uD83D\uDE00""#),
            Ok(JsonValue::String(String::from("😀")))
        );
    }

    #[test]
    fn parses_nested_values() {
        assert_eq!(
            parse(br#"{"items":[{"flag":true},[null,{"n":2}]],"name":"ok"}"#),
            Ok(JsonValue::Object(vec![
                (
                    String::from("items"),
                    JsonValue::Array(vec![
                        JsonValue::Object(vec![(String::from("flag"), JsonValue::Bool(true))]),
                        JsonValue::Array(vec![
                            JsonValue::Null,
                            JsonValue::Object(vec![(String::from("n"), JsonValue::Number(2.0))]),
                        ]),
                    ]),
                ),
                (String::from("name"), JsonValue::String(String::from("ok"))),
            ]))
        );
    }

    #[test]
    fn parses_deep_nesting_at_limit() {
        let mut input = vec![b'['; 128];
        input.extend(vec![b']'; 128]);
        // The innermost value is an empty array. 128 levels is allowed.
        assert!(parse(&input).is_ok());
    }

    #[test]
    fn skips_whitespace() {
        assert_eq!(
            parse(b" \n\t\r {\"a\" : [ true , null ] } \n"),
            Ok(JsonValue::Object(vec![(
                String::from("a"),
                JsonValue::Array(vec![JsonValue::Bool(true), JsonValue::Null]),
            )]))
        );
    }

    #[test]
    fn reports_trailing_content() {
        let error = parse(b"true false").unwrap_err();
        assert_eq!(error.kind, ParseErrorKind::TrailingContent);
    }

    #[test]
    fn reports_lone_surrogates() {
        assert_eq!(
            parse(br#""\uD83D""#).unwrap_err().kind,
            ParseErrorKind::InvalidUnicodeEscape
        );
        assert_eq!(
            parse(br#""\uDE00""#).unwrap_err().kind,
            ParseErrorKind::InvalidUnicodeEscape
        );
    }

    #[test]
    fn reports_leading_zero_numbers() {
        // If digits follow 0, the parser consumes only 0 and leaves trailing content.
        assert_eq!(
            parse(b"01").unwrap_err().kind,
            ParseErrorKind::TrailingContent
        );
        assert_eq!(
            parse(b"-01").unwrap_err().kind,
            ParseErrorKind::TrailingContent
        );
    }

    #[test]
    fn parses_all_supported_number_forms() {
        assert_eq!(parse(b"0"), Ok(JsonValue::Number(0.0)));
        assert_eq!(parse(b"-0"), Ok(JsonValue::Number(-0.0)));
        assert_eq!(parse(b"0.5"), Ok(JsonValue::Number(0.5)));
        assert_eq!(parse(b"-0.5"), Ok(JsonValue::Number(-0.5)));
        assert_eq!(parse(b"1e10"), Ok(JsonValue::Number(1e10)));
        assert_eq!(parse(b"1E+10"), Ok(JsonValue::Number(1e10)));
        assert_eq!(parse(b"1e-10"), Ok(JsonValue::Number(1e-10)));
        assert_eq!(parse(b"1.5e2"), Ok(JsonValue::Number(150.0)));
        assert_eq!(parse(b"-1.5E-3"), Ok(JsonValue::Number(-1.5e-3)));
    }

    #[test]
    fn rejects_additional_leading_zero_forms() {
        assert_eq!(
            parse(b"00").unwrap_err().kind,
            ParseErrorKind::TrailingContent
        );
        assert_eq!(
            parse(b"-00").unwrap_err().kind,
            ParseErrorKind::TrailingContent
        );
        assert_eq!(parse(b"-0"), Ok(JsonValue::Number(-0.0)));
    }

    #[test]
    fn parses_solidus_escape_explicitly() {
        assert_eq!(parse(br#""\/""#), Ok(JsonValue::String(String::from("/"))));
    }

    #[test]
    fn accepts_all_json_whitespace_around_values() {
        assert_eq!(parse(b"\t\r\n 42 \r\n\t"), Ok(JsonValue::Number(42.0)));
        assert_eq!(
            parse(b"\r\n[\t1,\n2,\r3 \t]\n"),
            Ok(JsonValue::Array(vec![
                JsonValue::Number(1.0),
                JsonValue::Number(2.0),
                JsonValue::Number(3.0),
            ]))
        );
        assert_eq!(
            parse(b"\t{\n\"a\"\r:\ttrue \n}\r"),
            Ok(JsonValue::Object(vec![(
                String::from("a"),
                JsonValue::Bool(true)
            )]))
        );
    }

    #[test]
    fn parses_empty_object_array_and_empty_string_key() {
        assert_eq!(parse(b"{}"), Ok(JsonValue::Object(vec![])));
        assert_eq!(parse(b"[]"), Ok(JsonValue::Array(vec![])));
        assert_eq!(
            parse(br#"{"":1}"#),
            Ok(JsonValue::Object(vec![(
                String::from(""),
                JsonValue::Number(1.0)
            )]))
        );
    }

    #[test]
    fn parses_unicode_escape_boundary_values() {
        assert_eq!(
            parse(br#""\u0000""#),
            Ok(JsonValue::String(String::from("\u{0000}")))
        );
        assert_eq!(
            parse(br#""\uFFFF""#),
            Ok(JsonValue::String(String::from("\u{FFFF}")))
        );
    }

    #[test]
    fn rejects_utf8_bom_prefix() {
        assert_eq!(
            parse(b"\xEF\xBB\xBFnull").unwrap_err().kind,
            ParseErrorKind::UnexpectedCharacter(0xEF)
        );
    }

    #[test]
    fn rejects_invalid_single_token_and_trailing_comma_inputs() {
        for input in [b"]".as_slice(), b"}", b",", b":"] {
            assert!(matches!(
                parse(input).unwrap_err().kind,
                ParseErrorKind::UnexpectedCharacter(_)
            ));
        }

        assert_eq!(
            parse(b"[,]").unwrap_err().kind,
            ParseErrorKind::UnexpectedCharacter(b',')
        );
        assert_eq!(
            parse(b"{,}").unwrap_err().kind,
            ParseErrorKind::UnexpectedCharacter(b',')
        );
        assert_eq!(
            parse(br#"{"key"}"#).unwrap_err().kind,
            ParseErrorKind::UnexpectedCharacter(b'}')
        );
        assert_eq!(
            parse(b"[1,]").unwrap_err().kind,
            ParseErrorKind::UnexpectedCharacter(b']')
        );
    }

    #[test]
    fn reports_max_depth_exceeded() {
        let mut input = vec![b'['; 129];
        input.extend(vec![b']'; 129]);
        assert_eq!(
            parse(&input).unwrap_err().kind,
            ParseErrorKind::NestingTooDeep
        );
    }

    #[test]
    fn reports_invalid_escape_sequences() {
        assert_eq!(
            parse(br#""\x""#).unwrap_err().kind,
            ParseErrorKind::InvalidEscape
        );
    }

    #[test]
    fn reports_exact_error_positions_for_post_advance_failures() {
        let invalid_escape = parse(br#""\x""#).unwrap_err();
        assert_eq!(invalid_escape.kind, ParseErrorKind::InvalidEscape);
        assert_eq!(invalid_escape.position, 2);

        let invalid_literal = parse(b"tXue").unwrap_err();
        assert_eq!(
            invalid_literal.kind,
            ParseErrorKind::UnexpectedCharacter(b'X')
        );
        assert_eq!(invalid_literal.position, 1);

        let control = parse(b"\"hello\nworld\"").unwrap_err();
        assert_eq!(control.kind, ParseErrorKind::UnexpectedCharacter(b'\n'));
        assert_eq!(control.position, 6);

        let invalid_unicode = parse(br#""\u12G4""#).unwrap_err();
        assert_eq!(invalid_unicode.kind, ParseErrorKind::InvalidUnicodeEscape);
        assert_eq!(invalid_unicode.position, 5);
    }

    #[test]
    fn reports_empty_input() {
        assert_eq!(parse(b"").unwrap_err().kind, ParseErrorKind::UnexpectedEnd);
    }

    #[test]
    fn parses_empty_array_and_object() {
        assert_eq!(parse(b"[]"), Ok(JsonValue::Array(vec![])));
        assert_eq!(parse(b"{}"), Ok(JsonValue::Object(vec![])));
    }

    #[test]
    fn parses_negative_exponent() {
        assert_eq!(parse(b"1e-2"), Ok(JsonValue::Number(0.01)));
        assert_eq!(parse(b"5E+3"), Ok(JsonValue::Number(5000.0)));
    }

    #[test]
    fn parses_multibyte_utf8_in_string() {
        // A string containing raw UTF-8 bytes.
        assert_eq!(
            parse("\"日本語\"".as_bytes()),
            Ok(JsonValue::String(String::from("日本語")))
        );
    }

    #[test]
    fn reports_control_character_in_string() {
        // 0x0A (newline) appears in the string without escaping.
        assert!(parse(b"\"hello\nworld\"").is_err());
    }

    #[test]
    fn reports_invalid_utf8_sequences_in_string() {
        assert_eq!(
            parse(b"\"\x80\"").unwrap_err().kind,
            ParseErrorKind::InvalidUtf8
        );
        assert_eq!(
            parse(b"\"\xC0\xAF\"").unwrap_err().kind,
            ParseErrorKind::InvalidUtf8
        );
        assert_eq!(
            parse(b"\"\xE3\x81\"").unwrap_err().kind,
            ParseErrorKind::InvalidUtf8
        );
    }
}
