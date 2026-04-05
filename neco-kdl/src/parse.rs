use crate::comment::{skip_line_space, skip_node_space, skip_ws};
use crate::number::{parse_keyword_number, parse_number};
use crate::scan::{is_newline, is_unicode_space, Scanner};
use crate::string::{parse_identifier_string, parse_quoted_string, parse_raw_string, parse_string};
use crate::{KdlDocument, KdlEntry, KdlError, KdlErrorKind, KdlNode, KdlValue};

/// KDL v2 ドキュメントをパースする。
pub fn parse(input: &str) -> Result<KdlDocument, KdlError> {
    let mut s = Scanner::new(input);
    s.skip_bom();
    skip_version_marker(&mut s)?;
    let nodes = parse_nodes(&mut s, false)?;
    skip_line_space(&mut s)?;
    if !s.is_eof() {
        return Err(s.make_error(KdlErrorKind::UnexpectedChar(s.peek().unwrap())));
    }
    Ok(KdlDocument { nodes })
}

/// version marker: `/-` unicode-space* `kdl-version` unicode-space+ (`1` | `2`) unicode-space* newline
fn skip_version_marker(s: &mut Scanner<'_>) -> Result<(), KdlError> {
    let saved = s.save();
    if s.peek() != Some('/') {
        return Ok(());
    }
    s.advance();
    if s.peek() != Some('-') {
        s.restore(saved);
        return Ok(());
    }
    s.advance();

    // unicode-space*
    while let Some(ch) = s.peek() {
        if is_unicode_space(ch) {
            s.advance();
        } else {
            break;
        }
    }

    // "kdl-version"
    let marker = "kdl-version";
    for expected in marker.chars() {
        match s.peek() {
            Some(ch) if ch == expected => {
                s.advance();
            }
            _ => {
                s.restore(saved);
                return Ok(());
            }
        }
    }

    // unicode-space+
    match s.peek() {
        Some(ch) if is_unicode_space(ch) => {
            s.advance();
        }
        _ => {
            s.restore(saved);
            return Ok(());
        }
    }
    while let Some(ch) = s.peek() {
        if is_unicode_space(ch) {
            s.advance();
        } else {
            break;
        }
    }

    // '1' | '2'
    match s.peek() {
        Some('1') | Some('2') => {
            s.advance();
        }
        _ => {
            s.restore(saved);
            return Ok(());
        }
    }

    // unicode-space*
    while let Some(ch) = s.peek() {
        if is_unicode_space(ch) {
            s.advance();
        } else {
            break;
        }
    }

    // newline
    if !s.consume_newline() && !s.is_eof() {
        s.restore(saved);
        return Ok(());
    }

    Ok(())
}

/// nodes := (line-space* node)* line-space*
/// `in_children` が true の場合、`}` で終了する。
fn parse_nodes(s: &mut Scanner<'_>, in_children: bool) -> Result<Vec<KdlNode>, KdlError> {
    let mut nodes = Vec::new();

    loop {
        skip_line_space(s)?;

        if in_children && s.peek() == Some('}') {
            break;
        }
        if s.is_eof() {
            break;
        }

        // slashdash でノード全体をスキップする可能性
        let slashdash = try_consume_slashdash(s)?;

        skip_line_space(s)?;

        if in_children && s.peek() == Some('}') {
            if slashdash {
                // slashdash の後に } は不正
                return Err(s.make_error(KdlErrorKind::InvalidSlashdash));
            }
            break;
        }
        if s.is_eof() {
            if slashdash {
                return Err(s.make_error(KdlErrorKind::InvalidSlashdash));
            }
            break;
        }

        // ノードをパース
        let before = s.byte_offset();
        let node = parse_base_node(s, in_children)?;

        if let Some(node) = node {
            if !slashdash {
                nodes.push(node);
            }
        } else if s.byte_offset() == before {
            // 何も消費されなかった — 不正な文字
            return Err(s.make_error(match s.peek() {
                Some(ch) => KdlErrorKind::UnexpectedChar(ch),
                None => KdlErrorKind::UnexpectedEof,
            }));
        }

        // node terminator
        if !in_children || s.peek() != Some('}') {
            skip_node_terminator(s, in_children)?;
        }
    }

    Ok(nodes)
}

/// `/-` line-space* を試みる。成功したら true。
fn try_consume_slashdash(s: &mut Scanner<'_>) -> Result<bool, KdlError> {
    if s.peek() != Some('/') {
        return Ok(false);
    }
    let saved = s.save();
    s.advance();
    if s.peek() == Some('-') {
        s.advance();
        skip_line_space(s)?;
        Ok(true)
    } else {
        s.restore(saved);
        Ok(false)
    }
}

/// node-terminator := single-line-comment | newline | ';' | eof
fn skip_node_terminator(s: &mut Scanner<'_>, in_children: bool) -> Result<(), KdlError> {
    // ノードの後のホワイトスペースは既に skip 済みのはず
    match s.peek() {
        Some(';') => {
            s.advance();
            Ok(())
        }
        Some('/') => {
            let saved = s.save();
            s.advance();
            if s.peek() == Some('/') {
                s.advance();
                crate::comment::skip_single_line_comment(s)?;
                s.consume_newline();
                Ok(())
            } else {
                s.restore(saved);
                Ok(())
            }
        }
        Some(ch) if is_newline(ch) => {
            s.consume_newline();
            Ok(())
        }
        Some('}') if in_children => {
            // final-node: ターミネーター省略可
            Ok(())
        }
        None => Ok(()), // EOF
        _ => {
            // terminator が来るべき位置に不正な文字
            Err(s.make_error(KdlErrorKind::UnexpectedChar(s.peek().unwrap())))
        }
    }
}

/// base-node をパースする。
fn parse_base_node(s: &mut Scanner<'_>, _in_children: bool) -> Result<Option<KdlNode>, KdlError> {
    // type annotation?
    let ty = try_parse_type_annotation(s)?;

    if ty.is_some() {
        skip_node_space_optional(s)?;
    }

    // ノード名 (string)
    let name = match parse_string_if_available(s)? {
        Some(name) => name,
        None => {
            if ty.is_some() {
                return Err(s.make_error(KdlErrorKind::UnexpectedEof));
            }
            return Ok(None);
        }
    };

    let mut entries = Vec::new();
    let mut children = None;

    // entries と children をパース
    loop {
        let had_space = skip_node_space_optional(s)?;

        // スペースなしの場合
        if !had_space {
            match s.peek() {
                Some('{') => {}                               // children block — スペースなしでも合法
                Some('/') if s.peek_next() == Some('-') => {} // slashdash — スペースなしでも合法
                _ => break,                                   // terminator or EOF — ノード終了
            }
        }

        // slashdash
        let entry_slashdash = try_consume_entry_slashdash(s)?;

        // children block?
        if s.peek() == Some('{') {
            if entry_slashdash {
                // slashdash された children block
                s.advance(); // '{'
                let _discarded = parse_nodes(s, true)?;
                s.expect('}')?;
                // slashdash children の後は children block か slashdash か terminator のみ
                // entry は来てはいけない (slashdash_child_block_before_entry_err_fail)
                skip_node_space_optional(s)?;
                match s.peek() {
                    Some('{') | Some(';') | None => {} // children block or terminator
                    Some('/') => {}                    // slashdash or comment — 次のループで判定
                    Some(ch) if is_newline(ch) => {}   // newline terminator
                    Some('}') => {}                    // children end
                    _ => return Err(s.make_error(KdlErrorKind::InvalidSlashdash)),
                }
                continue;
            }
            s.advance(); // '{'
            let child_nodes = parse_nodes(s, true)?;
            s.expect('}')?;
            children = Some(child_nodes);

            // children の後に slashdash された children block が来る可能性
            // スペースなしでも `/-` は合法
            loop {
                skip_node_space_optional(s)?;
                if try_consume_slashdash_children(s)? {
                    continue;
                }
                break;
            }
            break;
        }

        // entry (argument or property)
        if let Some(entry) = try_parse_entry(s)? {
            if !entry_slashdash {
                entries.push(entry);
            }
        } else {
            break;
        }
    }

    Ok(Some(KdlNode {
        ty,
        name,
        entries,
        children,
    }))
}

/// `/-` を entry/children 用に試みる。
fn try_consume_entry_slashdash(s: &mut Scanner<'_>) -> Result<bool, KdlError> {
    if s.peek() != Some('/') {
        return Ok(false);
    }
    let saved = s.save();
    s.advance();
    if s.peek() == Some('-') {
        s.advance();
        // slashdash の後に line-space*
        skip_line_space(s)?;
        // slashdash の後に }, ;, EOF が来たらエラー
        match s.peek() {
            Some('}') => return Err(s.make_error(KdlErrorKind::InvalidSlashdash)),
            Some(';') => return Err(s.make_error(KdlErrorKind::InvalidSlashdash)),
            None => return Err(s.make_error(KdlErrorKind::InvalidSlashdash)),
            _ => {}
        }
        Ok(true)
    } else {
        s.restore(saved);
        Ok(false)
    }
}

/// slashdash された children block を試みる: `/-` line-space* `{` nodes `}`
fn try_consume_slashdash_children(s: &mut Scanner<'_>) -> Result<bool, KdlError> {
    if s.peek() != Some('/') {
        return Ok(false);
    }
    let saved = s.save();
    s.advance();
    if s.peek() != Some('-') {
        s.restore(saved);
        return Ok(false);
    }
    s.advance();
    skip_line_space(s)?;

    if s.peek() != Some('{') {
        s.restore(saved);
        return Ok(false);
    }
    s.advance();
    let _discarded = parse_nodes(s, true)?;
    s.expect('}')?;
    Ok(true)
}

/// type annotation: `(` node-space* string node-space* `)`
fn try_parse_type_annotation(s: &mut Scanner<'_>) -> Result<Option<String>, KdlError> {
    if s.peek() != Some('(') {
        return Ok(None);
    }
    s.advance(); // '('
    skip_ws(s)?;
    let ty = parse_string(s)?;
    skip_ws(s)?;
    s.expect(')')?;
    Ok(Some(ty))
}

/// entry をパースする: prop | value
fn try_parse_entry(s: &mut Scanner<'_>) -> Result<Option<KdlEntry>, KdlError> {
    // まず type annotation をチェック
    let ty = try_parse_type_annotation(s)?;

    if ty.is_some() {
        skip_node_space_optional(s)?;

        // type annotation の後に slashdash は不可
        if s.peek() == Some('/') && s.peek_next() == Some('-') {
            return Err(s.make_error(KdlErrorKind::InvalidSlashdash));
        }

        // type annotation の後に値が必要。terminator/}/EOF/newline なら不正
        match s.peek() {
            None | Some(';') | Some('}') => {
                return Err(s.make_error(KdlErrorKind::UnexpectedChar(s.peek().unwrap_or(' '))));
            }
            Some(ch) if is_newline(ch) => {
                return Err(s.make_error(KdlErrorKind::UnexpectedChar(ch)));
            }
            Some('/') if s.peek_next() == Some('/') => {
                // single-line comment = terminator
                return Err(s.make_error(KdlErrorKind::UnexpectedChar('/')));
            }
            _ => {}
        }

        // type annotation の後に `=` が来たら type_before_prop_key_fail
        // → property key に type annotation はつけられない
        // ここでは string をパースした後に `=` チェックで対応
    }

    // `#` で始まる場合: raw string / keyword-number / keyword
    if s.peek() == Some('#') {
        // raw string を先に試す
        if let Some(r) = parse_raw_string(s)? {
            let str_val = KdlValue::String(r);
            // raw string は property key にもなれる: `=` チェック
            let saved_after = s.save();
            skip_ws(s)?;
            if s.peek() == Some('=') {
                if ty.is_some() {
                    return Err(s.make_error(KdlErrorKind::UnexpectedChar('=')));
                }
                s.advance();
                skip_ws(s)?;
                let key = match str_val {
                    KdlValue::String(k) => k,
                    _ => unreachable!(),
                };
                let val_ty = try_parse_type_annotation(s)?;
                if val_ty.is_some() {
                    skip_node_space_optional(s)?;
                }
                let value = parse_value(s)?;
                return Ok(Some(KdlEntry::Property {
                    key,
                    ty: val_ty,
                    value,
                }));
            }
            s.restore(saved_after);
            return Ok(Some(KdlEntry::Argument { ty, value: str_val }));
        }
        // keyword-number (#inf, #-inf, #nan)
        if let Some(kn) = parse_keyword_number(s)? {
            return Ok(Some(KdlEntry::Argument {
                ty,
                value: KdlValue::Number(kn),
            }));
        }
        // keyword (#true, #false, #null)
        if let Some(kw) = try_parse_keyword(s)? {
            return Ok(Some(KdlEntry::Argument { ty, value: kw }));
        }
        return Ok(None);
    }

    // number を試す
    if let Some(num) = parse_number(s)? {
        return Ok(Some(KdlEntry::Argument {
            ty,
            value: KdlValue::Number(num),
        }));
    }

    // string を試す (quoted, identifier)
    if let Some(str_val) = try_parse_string_value(s)? {
        // `=` が来たら property
        let saved_after = s.save();
        skip_ws(s)?;
        if s.peek() == Some('=') {
            // type annotation が entry に付いている場合、property key には付けられない
            if ty.is_some() {
                return Err(s.make_error(KdlErrorKind::UnexpectedChar('=')));
            }
            s.advance(); // '='
            skip_ws(s)?;
            // property: key = value
            let key = match str_val {
                KdlValue::String(k) => k,
                _ => unreachable!(),
            };
            // value の type annotation
            let val_ty = try_parse_type_annotation(s)?;
            if val_ty.is_some() {
                skip_node_space_optional(s)?;
            }
            let value = parse_value(s)?;
            return Ok(Some(KdlEntry::Property {
                key,
                ty: val_ty,
                value,
            }));
        }
        s.restore(saved_after);

        // argument
        return Ok(Some(KdlEntry::Argument { ty, value: str_val }));
    }

    Ok(None)
}

/// 文字列を KdlValue::String として試す。
fn try_parse_string_value(s: &mut Scanner<'_>) -> Result<Option<KdlValue>, KdlError> {
    if let Some(q) = parse_quoted_string(s)? {
        return Ok(Some(KdlValue::String(q)));
    }
    if let Some(r) = parse_raw_string(s)? {
        return Ok(Some(KdlValue::String(r)));
    }
    if let Some(id) = parse_identifier_string(s)? {
        return Ok(Some(KdlValue::String(id)));
    }
    Ok(None)
}

/// value: string | number | keyword
fn parse_value(s: &mut Scanner<'_>) -> Result<KdlValue, KdlError> {
    // keyword-number
    if let Some(kn) = parse_keyword_number(s)? {
        return Ok(KdlValue::Number(kn));
    }
    // keyword
    if let Some(kw) = try_parse_keyword(s)? {
        return Ok(kw);
    }
    // number
    if let Some(num) = parse_number(s)? {
        return Ok(KdlValue::Number(num));
    }
    // string
    let str_val = parse_string(s)?;
    Ok(KdlValue::String(str_val))
}

/// keyword: #true | #false | #null
fn try_parse_keyword(s: &mut Scanner<'_>) -> Result<Option<KdlValue>, KdlError> {
    if s.peek() != Some('#') {
        return Ok(None);
    }
    let saved = s.save();
    s.advance(); // '#'

    // #true
    if try_consume_literal(s, "true") {
        return Ok(Some(KdlValue::Bool(true)));
    }
    // #false
    if try_consume_literal(s, "false") {
        return Ok(Some(KdlValue::Bool(false)));
    }
    // #null
    if try_consume_literal(s, "null") {
        return Ok(Some(KdlValue::Null));
    }

    s.restore(saved);
    Ok(None)
}

/// リテラル文字列を消費する。マッチしたら true。
fn try_consume_literal(s: &mut Scanner<'_>, literal: &str) -> bool {
    let saved = s.save();
    for expected in literal.chars() {
        match s.peek() {
            Some(ch) if ch == expected => {
                s.advance();
            }
            _ => {
                s.restore(saved);
                return false;
            }
        }
    }
    true
}

/// node-space を可能な限りスキップする。何か消費したら true。
fn skip_node_space_optional(s: &mut Scanner<'_>) -> Result<bool, KdlError> {
    skip_node_space(s)
}

/// 文字列が利用可能な場合にパースする。
fn parse_string_if_available(s: &mut Scanner<'_>) -> Result<Option<String>, KdlError> {
    if let Some(q) = parse_quoted_string(s)? {
        return Ok(Some(q));
    }
    if let Some(r) = parse_raw_string(s)? {
        return Ok(Some(r));
    }
    if let Some(id) = parse_identifier_string(s)? {
        return Ok(Some(id));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(input: &str) -> KdlDocument {
        parse(input).unwrap()
    }

    fn p_err(input: &str) -> KdlError {
        parse(input).unwrap_err()
    }

    #[test]
    fn empty_document() {
        let doc = p("");
        assert!(doc.nodes.is_empty());
    }

    #[test]
    fn single_node_no_entries() {
        let doc = p("node");
        assert_eq!(doc.nodes.len(), 1);
        assert_eq!(doc.nodes[0].name, "node");
        assert!(doc.nodes[0].entries.is_empty());
        assert!(doc.nodes[0].children.is_none());
    }

    #[test]
    fn node_with_argument() {
        let doc = p("node 42");
        assert_eq!(doc.nodes[0].entries.len(), 1);
        match &doc.nodes[0].entries[0] {
            KdlEntry::Argument { value, .. } => match value {
                KdlValue::Number(n) => assert_eq!(n.as_i64, Some(42)),
                _ => panic!("expected number"),
            },
            _ => panic!("expected argument"),
        }
    }

    #[test]
    fn node_with_string_argument() {
        let doc = p("node \"hello\"");
        match &doc.nodes[0].entries[0] {
            KdlEntry::Argument { value, .. } => {
                assert_eq!(value, &KdlValue::String("hello".to_string()));
            }
            _ => panic!("expected argument"),
        }
    }

    #[test]
    fn node_with_property() {
        let doc = p("node key=\"value\"");
        assert_eq!(doc.nodes[0].entries.len(), 1);
        match &doc.nodes[0].entries[0] {
            KdlEntry::Property { key, value, .. } => {
                assert_eq!(key, "key");
                assert_eq!(value, &KdlValue::String("value".to_string()));
            }
            _ => panic!("expected property"),
        }
    }

    #[test]
    fn node_with_mixed_entries() {
        let doc = p("node 1 key=\"val\" 2");
        assert_eq!(doc.nodes[0].entries.len(), 3);
    }

    #[test]
    fn node_with_children() {
        let doc = p("parent {\n    child1\n    child2\n}");
        let parent = &doc.nodes[0];
        assert_eq!(parent.name, "parent");
        let children = parent.children.as_ref().unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].name, "child1");
        assert_eq!(children[1].name, "child2");
    }

    #[test]
    fn node_children_inline() {
        let doc = p("node { child1; child2 }");
        let children = doc.nodes[0].children.as_ref().unwrap();
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn final_node_no_terminator() {
        let doc = p("node { child1; child2 }");
        let children = doc.nodes[0].children.as_ref().unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[1].name, "child2");
    }

    #[test]
    fn multiple_nodes() {
        let doc = p("node1\nnode2\nnode3");
        assert_eq!(doc.nodes.len(), 3);
    }

    #[test]
    fn semicolon_separator() {
        let doc = p("node1; node2; node3");
        assert_eq!(doc.nodes.len(), 3);
    }

    #[test]
    fn type_annotation_on_node() {
        let doc = p("(mytype)node");
        assert_eq!(doc.nodes[0].ty, Some("mytype".to_string()));
        assert_eq!(doc.nodes[0].name, "node");
    }

    #[test]
    fn type_annotation_on_value() {
        let doc = p("node (u8)123");
        match &doc.nodes[0].entries[0] {
            KdlEntry::Argument { ty, value } => {
                assert_eq!(ty, &Some("u8".to_string()));
                match value {
                    KdlValue::Number(n) => assert_eq!(n.as_i64, Some(123)),
                    _ => panic!("expected number"),
                }
            }
            _ => panic!("expected argument"),
        }
    }

    #[test]
    fn keyword_values() {
        let doc = p("node #true #false #null");
        assert_eq!(doc.nodes[0].entries.len(), 3);
        match &doc.nodes[0].entries[0] {
            KdlEntry::Argument { value, .. } => assert_eq!(value, &KdlValue::Bool(true)),
            _ => panic!(),
        }
        match &doc.nodes[0].entries[1] {
            KdlEntry::Argument { value, .. } => assert_eq!(value, &KdlValue::Bool(false)),
            _ => panic!(),
        }
        match &doc.nodes[0].entries[2] {
            KdlEntry::Argument { value, .. } => assert_eq!(value, &KdlValue::Null),
            _ => panic!(),
        }
    }

    #[test]
    fn slashdash_node() {
        let doc = p("node1\n/-node2\nnode3");
        assert_eq!(doc.nodes.len(), 2);
        assert_eq!(doc.nodes[0].name, "node1");
        assert_eq!(doc.nodes[1].name, "node3");
    }

    #[test]
    fn slashdash_entry() {
        let doc = p("node 1 /- 2 3");
        assert_eq!(doc.nodes[0].entries.len(), 2);
    }

    #[test]
    fn slashdash_children() {
        let doc = p("node /- { child } {\n    real-child\n}");
        let children = doc.nodes[0].children.as_ref().unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].name, "real-child");
    }

    #[test]
    fn escline() {
        let doc = p("node \\\n    1 \\\n    2");
        assert_eq!(doc.nodes[0].entries.len(), 2);
    }

    #[test]
    fn bare_keyword_rejected() {
        let err = p_err("true");
        assert_eq!(err.kind, KdlErrorKind::BareKeyword);
    }

    #[test]
    fn bom_accepted() {
        let doc = p("\u{FEFF}node");
        assert_eq!(doc.nodes[0].name, "node");
    }

    #[test]
    fn property_with_spaces_around_equals() {
        let doc = p("node key = \"value\"");
        match &doc.nodes[0].entries[0] {
            KdlEntry::Property { key, value, .. } => {
                assert_eq!(key, "key");
                assert_eq!(value, &KdlValue::String("value".to_string()));
            }
            _ => panic!("expected property"),
        }
    }

    #[test]
    fn version_marker() {
        let doc = p("/- kdl-version 2\nnode");
        assert_eq!(doc.nodes.len(), 1);
        assert_eq!(doc.nodes[0].name, "node");
    }

    #[test]
    fn quoted_node_name() {
        let doc = p("\"my node\" 1");
        assert_eq!(doc.nodes[0].name, "my node");
    }

    #[test]
    fn nested_children() {
        let doc = p("a {\n    b {\n        c\n    }\n}");
        let a = &doc.nodes[0];
        let b = &a.children.as_ref().unwrap()[0];
        let c = &b.children.as_ref().unwrap()[0];
        assert_eq!(c.name, "c");
    }

    #[test]
    fn comments_ignored() {
        let doc = p("// comment\nnode /* inline */ 1 // trailing");
        assert_eq!(doc.nodes.len(), 1);
        assert_eq!(doc.nodes[0].entries.len(), 1);
    }
}
