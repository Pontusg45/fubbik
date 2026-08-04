//! Port of `packages/api/src/spaces/normalize-url.ts`. Used both when
//! storing a `code`-kind space's `remoteUrl` (create/update) and when
//! resolving `GET /api/spaces/detect?remoteUrl=` — the same function on
//! both sides is what makes `detect` able to find a space created through
//! this same API (see the module doc on `super::service` for the historical
//! Node seed-data gap this symmetry avoids).
//!
//! No `regex` crate dependency in this workspace, so the two regexes in the
//! original are hand-rolled below rather than pulled in for a four-line
//! helper. Each replicates its regex's semantics precisely, including the
//! JS-truthiness edge cases (`""` is falsy, `?? null` only replaces
//! `null`/`undefined`) — see `service::detect`/`service::update` for where
//! those edge cases matter.

/// Mirrors Node's `normalizeGitUrl`:
///
/// ```js
/// function normalizeGitUrl(url) {
///     let normalized = url.trim();
///     const sshMatch = normalized.match(/^(?:ssh:\/\/)?git@([^:/]+)[:/](.+)$/);
///     if (sshMatch) {
///         normalized = `${sshMatch[1]}/${sshMatch[2]}`;
///     } else {
///         normalized = normalized.replace(/^[a-z+]+:\/\//, "");
///         normalized = normalized.replace(/^[^@]+@/, "");
///     }
///     normalized = normalized.replace(/\.git$/, "");
///     normalized = normalized.replace(/\/+$/, "");
///     return normalized;
/// }
/// ```
pub fn normalize_git_url(url: &str) -> String {
    let trimmed = url.trim();
    let mut normalized = ssh_form(trimmed).unwrap_or_else(|| strip_scheme_and_userinfo(trimmed));

    if let Some(stripped) = normalized.strip_suffix(".git") {
        normalized = stripped.to_string();
    }
    while normalized.ends_with('/') {
        normalized.pop();
    }
    normalized
}

/// `^(?:ssh:\/\/)?git@([^:/]+)[:/](.+)$` — an optional `ssh://` prefix,
/// literal `git@`, a host with no `:`/`/`, a `:` or `/` separator, then a
/// non-empty path. Returns `host/path` on a match, `None` otherwise (the
/// caller falls through to the scheme/userinfo-stripping branch, exactly as
/// the `else` in Node's `if (sshMatch) {...} else {...}` does).
fn ssh_form(s: &str) -> Option<String> {
    let rest = s.strip_prefix("ssh://").unwrap_or(s);
    let rest = rest.strip_prefix("git@")?;
    let idx = rest.find([':', '/'])?;
    let host = &rest[..idx];
    let path = &rest[idx + 1..];
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some(format!("{host}/{path}"))
}

/// `replace(/^[a-z+]+:\/\//, "")` then `replace(/^[^@]+@/, "")`.
fn strip_scheme_and_userinfo(s: &str) -> String {
    let mut normalized = s;

    if let Some(scheme_end) = normalized.find("://") {
        let scheme = &normalized[..scheme_end];
        if !scheme.is_empty() && scheme.chars().all(|c| c.is_ascii_lowercase() || c == '+') {
            normalized = &normalized[scheme_end + 3..];
        }
    }

    if let Some(at_idx) = normalized.find('@')
        && at_idx > 0
    {
        normalized = &normalized[at_idx + 1..];
    }

    normalized.to_string()
}

#[cfg(test)]
mod tests {
    use super::normalize_git_url;

    #[test]
    fn ssh_shorthand_form() {
        assert_eq!(
            normalize_git_url("git@github.com:acme/fubbik.git"),
            "github.com/acme/fubbik"
        );
    }

    #[test]
    fn ssh_scheme_form() {
        assert_eq!(
            normalize_git_url("ssh://git@github.com/acme/fubbik.git"),
            "github.com/acme/fubbik"
        );
    }

    #[test]
    fn https_form() {
        assert_eq!(
            normalize_git_url("https://github.com/acme/fubbik.git"),
            "github.com/acme/fubbik"
        );
    }

    #[test]
    fn https_with_userinfo() {
        assert_eq!(
            normalize_git_url("https://user@github.com/acme/fubbik.git"),
            "github.com/acme/fubbik"
        );
    }

    #[test]
    fn trailing_slashes_are_stripped() {
        assert_eq!(
            normalize_git_url("https://github.com/acme/fubbik///"),
            "github.com/acme/fubbik"
        );
    }

    #[test]
    fn already_normalized_is_unchanged() {
        assert_eq!(
            normalize_git_url("github.com/acme/fubbik"),
            "github.com/acme/fubbik"
        );
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!(
            normalize_git_url("  git@github.com:acme/fubbik.git  "),
            "github.com/acme/fubbik"
        );
    }
}
