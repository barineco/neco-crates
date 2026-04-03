use core::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextPatch {
    start: usize,
    end: usize,
    replacement: String,
}

impl TextPatch {
    pub fn new(
        start: usize,
        end: usize,
        replacement: impl Into<String>,
    ) -> Result<Self, TextPatchError> {
        if start > end {
            return Err(TextPatchError::InvalidRange { start, end });
        }
        Ok(Self {
            start,
            end,
            replacement: replacement.into(),
        })
    }

    pub fn insert(offset: usize, replacement: impl Into<String>) -> Self {
        Self {
            start: offset,
            end: offset,
            replacement: replacement.into(),
        }
    }

    pub fn delete(start: usize, end: usize) -> Result<Self, TextPatchError> {
        Self::new(start, end, "")
    }

    pub fn replace(
        start: usize,
        end: usize,
        replacement: impl Into<String>,
    ) -> Result<Self, TextPatchError> {
        Self::new(start, end, replacement)
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn replacement(&self) -> &str {
        &self.replacement
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchConflict {
    pub first_index: usize,
    pub second_index: usize,
    pub first_start: usize,
    pub first_end: usize,
    pub second_start: usize,
    pub second_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextPatchError {
    InvalidRange { start: usize, end: usize },
    OffsetOutOfBounds { offset: usize, len: usize },
    InvalidUtf8Boundary { offset: usize },
    Conflict(PatchConflict),
    BlockNotFound { block_name: String },
    UnclosedBlock { block_name: String },
    AmbiguousEntry { block_name: String, key: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockRange {
    pub start: usize,
    pub content_start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownEntry<'a> {
    pub key: &'a str,
    pub replacement: &'a str,
}

pub fn validate_patches(source: &str, patches: &[TextPatch]) -> Result<(), TextPatchError> {
    let sorted = sorted_patches(patches);
    let mut previous_non_empty: Option<SortedPatch<'_>> = None;

    for current in sorted {
        validate_patch_bounds(source, current.patch)?;
        if current.patch.start == current.patch.end {
            if let Some(previous) = previous_non_empty {
                if current.patch.start < previous.patch.end {
                    return Err(TextPatchError::Conflict(PatchConflict {
                        first_index: previous.index,
                        second_index: current.index,
                        first_start: previous.patch.start,
                        first_end: previous.patch.end,
                        second_start: current.patch.start,
                        second_end: current.patch.end,
                    }));
                }
            }
            continue;
        }

        if let Some(previous) = previous_non_empty {
            if current.patch.start < previous.patch.end {
                return Err(TextPatchError::Conflict(PatchConflict {
                    first_index: previous.index,
                    second_index: current.index,
                    first_start: previous.patch.start,
                    first_end: previous.patch.end,
                    second_start: current.patch.start,
                    second_end: current.patch.end,
                }));
            }
        }
        previous_non_empty = Some(current);
    }

    Ok(())
}

pub fn apply_patch(source: &str, patch: &TextPatch) -> Result<String, TextPatchError> {
    apply_patches(source, core::slice::from_ref(patch))
}

pub fn apply_patches(source: &str, patches: &[TextPatch]) -> Result<String, TextPatchError> {
    validate_patches(source, patches)?;

    let mut ordered = sorted_patches(patches);
    ordered.sort_by(|left, right| {
        left.patch
            .start
            .cmp(&right.patch.start)
            .then_with(|| left.patch.end.cmp(&right.patch.end))
            .then_with(|| left.index.cmp(&right.index))
    });

    let mut output = source.to_string();
    for current in ordered.into_iter().rev() {
        output.replace_range(
            current.patch.start..current.patch.end,
            current.patch.replacement(),
        );
    }
    Ok(output)
}

pub fn find_block_range(source: &str, block_name: &str) -> Result<BlockRange, TextPatchError> {
    let Some((block_start, open_brace)) = find_named_block_start(source, block_name) else {
        return Err(TextPatchError::BlockNotFound {
            block_name: block_name.to_string(),
        });
    };

    let mut depth = 0usize;
    for (offset, ch) in source[open_brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let end = open_brace + offset + ch.len_utf8();
                    return Ok(BlockRange {
                        start: block_start,
                        content_start: open_brace + 1,
                        end,
                    });
                }
            }
            _ => {}
        }
    }

    Err(TextPatchError::UnclosedBlock {
        block_name: block_name.to_string(),
    })
}

pub fn replace_block(
    source: &str,
    block_name: &str,
    replacement: &str,
) -> Result<TextPatch, TextPatchError> {
    let range = find_block_range(source, block_name)?;
    TextPatch::replace(range.content_start, range.end - 1, replacement)
}

pub fn merge_known_entries(
    source: &str,
    block_name: &str,
    entries: &[KnownEntry<'_>],
) -> Result<TextPatch, TextPatchError> {
    let range = find_block_range(source, block_name)?;
    let block_content = &source[range.content_start..range.end - 1];

    for (index, entry) in entries.iter().enumerate() {
        if entries[..index]
            .iter()
            .any(|earlier| earlier.key == entry.key)
        {
            return Err(TextPatchError::AmbiguousEntry {
                block_name: block_name.to_string(),
                key: entry.key.to_string(),
            });
        }
    }

    let segments = split_top_level_entries(block_content);
    for entry in entries {
        let matches = segments
            .iter()
            .filter(|segment| identify_entry_key(segment) == Some(entry.key))
            .count();
        if matches > 1 {
            return Err(TextPatchError::AmbiguousEntry {
                block_name: block_name.to_string(),
                key: entry.key.to_string(),
            });
        }
    }

    let mut merged: Vec<String> = Vec::with_capacity(segments.len() + entries.len());
    let mut seen: Vec<&str> = Vec::with_capacity(entries.len());

    for segment in &segments {
        if let Some(key) = identify_entry_key(segment) {
            if let Some(entry) = entries.iter().find(|candidate| candidate.key == key) {
                if !seen.contains(&key) {
                    merged.push(entry.replacement.to_string());
                    seen.push(key);
                    continue;
                }
            }
        }
        merged.push((*segment).to_string());
    }

    let mut merged = trim_trailing_blank_segments(merged);
    merged = compact_blank_segment_runs(merged);
    for entry in entries {
        if seen.contains(&entry.key) {
            continue;
        }
        merged.push(entry.replacement.to_string());
    }

    let replacement = if merged.is_empty() {
        String::new()
    } else {
        let mut text = merged.concat();
        if !text.starts_with('\n') {
            text.insert(0, '\n');
        }
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text
    };
    TextPatch::replace(range.content_start, range.end - 1, replacement)
}

#[derive(Clone, Copy)]
struct SortedPatch<'a> {
    index: usize,
    patch: &'a TextPatch,
}

fn sorted_patches(patches: &[TextPatch]) -> Vec<SortedPatch<'_>> {
    let mut sorted: Vec<_> = patches
        .iter()
        .enumerate()
        .map(|(index, patch)| SortedPatch { index, patch })
        .collect();
    sorted.sort_by(|left, right| compare_for_validation(*left, *right));
    sorted
}

fn compare_for_validation(left: SortedPatch<'_>, right: SortedPatch<'_>) -> Ordering {
    left.patch
        .start
        .cmp(&right.patch.start)
        .then_with(|| left.patch.is_insert().cmp(&right.patch.is_insert()))
        .then_with(|| left.patch.end.cmp(&right.patch.end))
        .then_with(|| left.index.cmp(&right.index))
}

fn validate_patch_bounds(source: &str, patch: &TextPatch) -> Result<(), TextPatchError> {
    let len = source.len();
    if patch.start > len {
        return Err(TextPatchError::OffsetOutOfBounds {
            offset: patch.start,
            len,
        });
    }
    if patch.end > len {
        return Err(TextPatchError::OffsetOutOfBounds {
            offset: patch.end,
            len,
        });
    }
    if !source.is_char_boundary(patch.start) {
        return Err(TextPatchError::InvalidUtf8Boundary {
            offset: patch.start,
        });
    }
    if !source.is_char_boundary(patch.end) {
        return Err(TextPatchError::InvalidUtf8Boundary { offset: patch.end });
    }
    Ok(())
}

fn find_named_block_start(source: &str, block_name: &str) -> Option<(usize, usize)> {
    let mut search_from = 0usize;
    while search_from <= source.len() {
        let offset = source[search_from..].find(block_name)?;
        let name_start = search_from + offset;
        let name_end = name_start + block_name.len();
        if !is_identifier_boundary(source, name_start, name_end) {
            search_from = name_end;
            continue;
        }

        let mut cursor = name_end;
        while let Some(ch) = source[cursor..].chars().next() {
            if ch.is_whitespace() {
                cursor += ch.len_utf8();
                continue;
            }
            if ch == '{' {
                return Some((name_start, cursor));
            }
            break;
        }
        search_from = name_end;
    }
    None
}

fn is_identifier_boundary(source: &str, start: usize, end: usize) -> bool {
    let previous_ok = if start == 0 {
        true
    } else {
        source[..start]
            .chars()
            .next_back()
            .map(|ch| !is_identifier_char(ch))
            .unwrap_or(true)
    };
    let next_ok = if end == source.len() {
        true
    } else {
        source[end..]
            .chars()
            .next()
            .map(|ch| !is_identifier_char(ch))
            .unwrap_or(true)
    };
    previous_ok && next_ok
}

fn split_top_level_entries(source: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut depth = 0usize;
    let mut segment_start = 0usize;

    for (index, ch) in source.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            '\n' if depth == 0 => {
                segments.push(&source[segment_start..index + 1]);
                segment_start = index + 1;
            }
            _ => {}
        }
    }

    if segment_start < source.len() {
        segments.push(&source[segment_start..]);
    }

    segments
}

fn identify_entry_key(entry: &str) -> Option<&str> {
    let trimmed = entry.trim_start();
    if trimmed.is_empty() {
        return None;
    }

    let end = trimmed
        .char_indices()
        .find_map(|(index, ch)| (!is_identifier_char(ch)).then_some(index))
        .unwrap_or(trimmed.len());
    (end > 0).then_some(&trimmed[..end])
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
}

fn trim_trailing_blank_segments(mut segments: Vec<String>) -> Vec<String> {
    while segments
        .last()
        .is_some_and(|segment| segment.trim().is_empty())
    {
        segments.pop();
    }
    segments
}

fn compact_blank_segment_runs(segments: Vec<String>) -> Vec<String> {
    let mut compacted = Vec::with_capacity(segments.len());
    let mut previous_blank = false;
    for segment in segments {
        let is_blank = segment.trim().is_empty();
        if is_blank && previous_blank {
            continue;
        }
        previous_blank = is_blank;
        compacted.push(segment);
    }
    compacted
}

impl TextPatch {
    fn is_insert(&self) -> bool {
        self.start == self.end
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_patch, apply_patches, find_block_range, merge_known_entries, replace_block,
        validate_patches, BlockRange, KnownEntry, PatchConflict, TextPatch, TextPatchError,
    };

    #[test]
    fn apply_patch_replaces_single_range() {
        let patch = TextPatch::replace(6, 11, "there").expect("valid patch");
        let updated = apply_patch("hello world", &patch).expect("patch should apply");
        assert_eq!(updated, "hello there");
    }

    #[test]
    fn text_patch_rejects_reversed_range() {
        let err = TextPatch::replace(4, 2, "x").expect_err("reversed range must fail");
        assert_eq!(err, TextPatchError::InvalidRange { start: 4, end: 2 });
    }

    #[test]
    fn apply_patch_rejects_non_utf8_boundary_offsets() {
        let patch = TextPatch::replace(1, 3, "A").expect("range ordering is valid");
        let err = apply_patch("あい", &patch).expect_err("middle of code point must fail");
        assert_eq!(err, TextPatchError::InvalidUtf8Boundary { offset: 1 });
    }

    #[test]
    fn apply_patches_uses_deterministic_original_offsets() {
        let patches = vec![
            TextPatch::replace(0, 1, "A").expect("valid patch"),
            TextPatch::insert(3, "!"),
        ];
        let updated = apply_patches("abc", &patches).expect("patches should apply");
        assert_eq!(updated, "Abc!");
    }

    #[test]
    fn validate_patches_rejects_overlapping_ranges() {
        let patches = vec![
            TextPatch::replace(1, 3, "XX").expect("valid patch"),
            TextPatch::replace(2, 4, "YY").expect("valid patch"),
        ];
        let err = validate_patches("abcdef", &patches).expect_err("overlap must fail");
        assert_eq!(
            err,
            TextPatchError::Conflict(PatchConflict {
                first_index: 0,
                second_index: 1,
                first_start: 1,
                first_end: 3,
                second_start: 2,
                second_end: 4,
            })
        );
    }

    #[test]
    fn validate_patches_allows_multiple_inserts_at_same_offset() {
        let patches = vec![TextPatch::insert(1, "X"), TextPatch::insert(1, "Y")];
        validate_patches("ab", &patches).expect("same-offset inserts should be allowed");
        let updated = apply_patches("ab", &patches).expect("patches should apply");
        assert_eq!(updated, "aXYb");
    }

    #[test]
    fn validate_patches_allows_insert_at_range_end() {
        let patches = vec![
            TextPatch::replace(1, 3, "BC").expect("valid patch"),
            TextPatch::insert(3, "!"),
        ];
        validate_patches("abcd", &patches).expect("insert at end boundary is allowed");
        let updated = apply_patches("abcd", &patches).expect("patches should apply");
        assert_eq!(updated, "aBC!d");
    }

    #[test]
    fn validate_patches_rejects_insert_at_range_start() {
        let patches = vec![
            TextPatch::replace(1, 3, "BC").expect("valid patch"),
            TextPatch::insert(1, "!"),
        ];
        let err =
            validate_patches("abcd", &patches).expect_err("insert at range start must conflict");
        assert_eq!(
            err,
            TextPatchError::Conflict(PatchConflict {
                first_index: 0,
                second_index: 1,
                first_start: 1,
                first_end: 3,
                second_start: 1,
                second_end: 1,
            })
        );
    }

    #[test]
    fn find_block_range_returns_named_brace_block() {
        let source = "settings {\n  theme \"dark\"\n}\n";
        let range = find_block_range(source, "settings").expect("settings block should exist");
        assert_eq!(
            range,
            BlockRange {
                start: 0,
                content_start: 10,
                end: source.len() - 1,
            }
        );
        assert_eq!(
            &source[range.content_start..range.end - 1],
            "\n  theme \"dark\"\n"
        );
    }

    #[test]
    fn replace_block_returns_patch_for_block_content() {
        let source = "settings {\n  theme \"dark\"\n}\n";
        let patch = replace_block(source, "settings", "\n  theme \"light\"\n")
            .expect("replace should succeed");
        let updated = apply_patch(source, &patch).expect("patch should apply");
        assert_eq!(updated, "settings {\n  theme \"light\"\n}\n");
    }

    #[test]
    fn find_block_range_errors_when_block_is_missing() {
        let err = find_block_range("root {\n}\n", "settings").expect_err("missing block must fail");
        assert_eq!(
            err,
            TextPatchError::BlockNotFound {
                block_name: "settings".to_string(),
            }
        );
    }

    #[test]
    fn find_block_range_errors_when_block_is_unclosed() {
        let err = find_block_range("settings {\n  theme \"dark\"\n", "settings")
            .expect_err("unclosed block must fail");
        assert_eq!(
            err,
            TextPatchError::UnclosedBlock {
                block_name: "settings".to_string(),
            }
        );
    }

    #[test]
    fn merge_known_entries_replaces_existing_and_appends_missing_keys() {
        let source = "settings {\n  theme \"dark\"\n  shell \"/bin/zsh\"\n  extra 1\n}\n";
        let entries = [
            KnownEntry {
                key: "theme",
                replacement: "  theme \"light\"\n",
            },
            KnownEntry {
                key: "font",
                replacement: "  font {\n    size 12\n  }\n",
            },
        ];
        let patch =
            merge_known_entries(source, "settings", &entries).expect("merge should succeed");
        let updated = apply_patch(source, &patch).expect("patch should apply");
        assert_eq!(
            updated,
            "settings {\n  theme \"light\"\n  shell \"/bin/zsh\"\n  extra 1\n  font {\n    size 12\n  }\n}\n"
        );
    }

    #[test]
    fn merge_known_entries_rejects_ambiguous_known_key() {
        let source = "settings {\n  theme \"dark\"\n  theme \"light\"\n}\n";
        let entries = [KnownEntry {
            key: "theme",
            replacement: "  theme \"paper\"\n",
        }];
        let err = merge_known_entries(source, "settings", &entries)
            .expect_err("duplicate keys must fail");
        assert_eq!(
            err,
            TextPatchError::AmbiguousEntry {
                block_name: "settings".to_string(),
                key: "theme".to_string(),
            }
        );
    }
}
