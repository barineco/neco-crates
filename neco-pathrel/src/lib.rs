/// Case handling used when comparing path segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathCaseSensitivity {
    Sensitive,
    Insensitive,
}

/// Path comparison policy shared by relation helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathPolicy {
    pub separator: char,
    pub case_sensitivity: PathCaseSensitivity,
}

impl PathPolicy {
    #[inline]
    pub const fn new(separator: char, case_sensitivity: PathCaseSensitivity) -> Self {
        Self {
            separator,
            case_sensitivity,
        }
    }

    #[inline]
    pub const fn posix() -> Self {
        Self::new('/', PathCaseSensitivity::Sensitive)
    }
}

/// Return whether `path` is equal to `target` or lies under `target`.
pub fn path_matches_or_contains(path: &str, target: &str, policy: &PathPolicy) -> bool {
    let normalized_path = normalized_for_compare(path, policy.separator);
    let normalized_target = normalized_for_compare(target, policy.separator);
    if compare_with_policy(normalized_path, normalized_target, policy) {
        return true;
    }
    match strip_prefix_with_policy(normalized_path, normalized_target, policy) {
        Some(remainder) => starts_with_separator(remainder, policy.separator),
        None => false,
    }
}

/// Return the direct parent slice when one exists under the given policy.
pub fn parent_path<'a>(path: &'a str, policy: &PathPolicy) -> Option<&'a str> {
    let normalized = trim_trailing_separators(path, policy.separator);
    if normalized.is_empty() {
        return None;
    }
    if is_root_path(normalized, policy.separator) {
        return None;
    }
    let last_index = normalized.rfind(policy.separator)?;
    if last_index == 0 {
        return Some(&normalized[..1]);
    }
    Some(&normalized[..last_index])
}

/// Join `base` and `name` using the policy separator.
pub fn join_path(base: &str, name: &str, policy: &PathPolicy) -> String {
    let normalized_base = trim_trailing_separators(base, policy.separator);
    let normalized_name = trim_leading_separators(name, policy.separator);
    if normalized_base.is_empty() {
        return normalized_name.to_string();
    }
    if normalized_name.is_empty() {
        return normalized_base.to_string();
    }
    if is_root_path(normalized_base, policy.separator) {
        return format!("{}{normalized_name}", policy.separator);
    }
    format!("{normalized_base}{}{normalized_name}", policy.separator)
}

/// Remap `path` from the renamed `source` subtree into `target`.
pub fn remap_path_for_rename(
    path: &str,
    source: &str,
    target: &str,
    policy: &PathPolicy,
) -> Option<String> {
    let normalized_path = normalized_for_compare(path, policy.separator);
    let normalized_source = normalized_for_compare(source, policy.separator);
    let normalized_target = normalized_for_compare(target, policy.separator);
    if compare_with_policy(normalized_path, normalized_source, policy) {
        return Some(normalized_target.to_string());
    }
    let remainder = strip_prefix_with_policy(normalized_path, normalized_source, policy)?;
    if !starts_with_separator(remainder, policy.separator) {
        return None;
    }
    let stripped_remainder = trim_leading_separators(remainder, policy.separator);
    Some(join_path(normalized_target, stripped_remainder, policy))
}

fn compare_with_policy(left: &str, right: &str, policy: &PathPolicy) -> bool {
    match policy.case_sensitivity {
        PathCaseSensitivity::Sensitive => left == right,
        PathCaseSensitivity::Insensitive => left.to_lowercase() == right.to_lowercase(),
    }
}

fn strip_prefix_with_policy<'a>(
    path: &'a str,
    prefix: &str,
    policy: &PathPolicy,
) -> Option<&'a str> {
    match policy.case_sensitivity {
        PathCaseSensitivity::Sensitive => path.strip_prefix(prefix),
        PathCaseSensitivity::Insensitive => {
            let prefix_len = prefix.len();
            let path_prefix = path.get(..prefix_len)?;
            if path_prefix.to_lowercase() == prefix.to_lowercase() {
                path.get(prefix_len..)
            } else {
                None
            }
        }
    }
}

fn normalized_for_compare(path: &str, separator: char) -> &str {
    let trimmed = trim_trailing_separators(path, separator);
    if trimmed.is_empty() && path.starts_with(separator) {
        root_slice(path, separator)
    } else {
        trimmed
    }
}

fn trim_trailing_separators(path: &str, separator: char) -> &str {
    if path.is_empty() {
        return path;
    }
    let trimmed = path.trim_end_matches(separator);
    if trimmed.is_empty() && path.starts_with(separator) {
        root_slice(path, separator)
    } else {
        trimmed
    }
}

fn trim_leading_separators(path: &str, separator: char) -> &str {
    path.trim_start_matches(separator)
}

fn starts_with_separator(path: &str, separator: char) -> bool {
    path.starts_with(separator)
}

fn is_root_path(path: &str, separator: char) -> bool {
    let mut chars = path.chars();
    matches!(chars.next(), Some(ch) if ch == separator) && chars.next().is_none()
}

fn root_slice(path: &str, separator: char) -> &str {
    let len = separator.len_utf8();
    if path.starts_with(separator) && path.len() >= len {
        &path[..len]
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::{
        join_path, parent_path, path_matches_or_contains, remap_path_for_rename,
        PathCaseSensitivity, PathPolicy,
    };

    fn posix() -> PathPolicy {
        PathPolicy::posix()
    }

    fn insensitive() -> PathPolicy {
        PathPolicy::new('/', PathCaseSensitivity::Insensitive)
    }

    #[test]
    fn exact_match_is_true() {
        assert!(path_matches_or_contains(
            "/workspace/src",
            "/workspace/src",
            &posix(),
        ));
    }

    #[test]
    fn descendant_match_is_true() {
        assert!(path_matches_or_contains(
            "/workspace/src/lib.rs",
            "/workspace/src",
            &posix(),
        ));
    }

    #[test]
    fn prefix_collision_is_not_treated_as_descendant() {
        assert!(!path_matches_or_contains(
            "/workspace/barista",
            "/workspace/bar",
            &posix(),
        ));
    }

    #[test]
    fn trailing_separator_difference_is_ignored_for_comparison() {
        assert!(path_matches_or_contains(
            "/workspace/src/lib.rs",
            "/workspace/src/",
            &posix(),
        ));
    }

    #[test]
    fn case_sensitivity_is_policy_driven() {
        assert!(!path_matches_or_contains(
            "/Workspace/Src",
            "/workspace/src",
            &posix()
        ));
        assert!(path_matches_or_contains(
            "/Workspace/Src",
            "/workspace/src",
            &insensitive(),
        ));
    }

    #[test]
    fn parent_path_returns_none_for_root_and_single_segment() {
        assert_eq!(parent_path("/", &posix()), None);
        assert_eq!(parent_path("workspace", &posix()), None);
    }

    #[test]
    fn parent_path_returns_parent_slice_without_allocating() {
        assert_eq!(
            parent_path("/workspace/src/lib.rs", &posix()),
            Some("/workspace/src")
        );
        assert_eq!(parent_path("/workspace/src/", &posix()), Some("/workspace"));
    }

    #[test]
    fn join_path_inserts_one_separator() {
        assert_eq!(
            join_path("/workspace/src/", "lib.rs", &posix()),
            "/workspace/src/lib.rs"
        );
        assert_eq!(
            join_path("/workspace/src", "/lib.rs", &posix()),
            "/workspace/src/lib.rs"
        );
    }

    #[test]
    fn join_path_handles_root_and_empty_segments() {
        assert_eq!(join_path("/", "lib.rs", &posix()), "/lib.rs");
        assert_eq!(join_path("/workspace/src/", "", &posix()), "/workspace/src");
        assert_eq!(join_path("", "/lib.rs", &posix()), "lib.rs");
    }

    #[test]
    fn rename_remap_matches_file_exactly() {
        assert_eq!(
            remap_path_for_rename(
                "/workspace/old.txt",
                "/workspace/old.txt",
                "/workspace/new.txt",
                &posix()
            ),
            Some("/workspace/new.txt".to_string())
        );
    }

    #[test]
    fn rename_remap_updates_subtree_descendants() {
        assert_eq!(
            remap_path_for_rename(
                "/workspace/dir/nested/file.txt",
                "/workspace/dir",
                "/workspace/renamed",
                &posix(),
            ),
            Some("/workspace/renamed/nested/file.txt".to_string())
        );
    }

    #[test]
    fn rename_remap_rejects_prefix_collision() {
        assert_eq!(
            remap_path_for_rename(
                "/workspace/barista/file.txt",
                "/workspace/bar",
                "/workspace/renamed",
                &posix(),
            ),
            None
        );
    }

    #[test]
    fn rename_remap_ignores_trailing_separator_mismatch() {
        assert_eq!(
            remap_path_for_rename(
                "/workspace/dir/file.txt",
                "/workspace/dir/",
                "/workspace/renamed/",
                &posix(),
            ),
            Some("/workspace/renamed/file.txt".to_string())
        );
    }
}
