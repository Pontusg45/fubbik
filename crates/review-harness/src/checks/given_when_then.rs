//! Every test body carries `// Given`, `// When` and `// Then` comments.

use std::collections::HashSet;

use crate::{Check, TextFile, Violation};

/// Violations present when the check was introduced (counted by `review-harness run`: none).
/// This may only go down.
const BASELINE: usize = 0;

pub struct GivenWhenThen;

impl Check for GivenWhenThen {
    fn name(&self) -> &str {
        "given-when-then"
    }
    fn description(&self) -> &str {
        "tests carry // Given, // When and // Then comments"
    }
    fn roots(&self) -> Vec<&str> {
        vec!["crates"]
    }
    fn extensions(&self) -> Vec<&str> {
        vec![".rs"]
    }
    fn check(&self, file: &TextFile) -> Vec<Violation> {
        let scan = Scan::new(&file.text);
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for attr in scan.test_attributes() {
            let Some((fn_pos, body)) = scan.function_after(attr) else {
                continue;
            };
            if !seen.insert(fn_pos) {
                continue;
            }
            let found = scan.markers_in(body.0, body.1);
            let missing: Vec<&str> = ["Given", "When", "Then"]
                .into_iter()
                .zip(found)
                .filter(|(_, present)| !present)
                .map(|(name, _)| name)
                .collect();
            if !missing.is_empty() {
                let list: Vec<String> = missing.iter().map(|m| format!("// {m}")).collect();
                out.push(Violation {
                    file: file.path.clone(),
                    line: scan.line_of(fn_pos),
                    message: format!("test is missing {}", list.join(", ")),
                });
            }
        }
        out
    }
    fn baseline(&self) -> usize {
        BASELINE
    }
}

/// A lexed view of one Rust file: `code` is the source with comments and string/char literal
/// contents blanked to spaces (newlines kept), so brace and attribute matching cannot be fooled.
/// `comments` holds each `//` line comment's start index and text.
struct Scan {
    code: Vec<char>,
    comments: Vec<(usize, String)>,
}

impl Scan {
    fn new(text: &str) -> Self {
        let src: Vec<char> = text.chars().collect();
        let mut code = src.clone();
        let mut comments = Vec::new();
        let blank = |code: &mut Vec<char>, from: usize, to: usize| {
            for c in &mut code[from..to] {
                if *c != '\n' {
                    *c = ' ';
                }
            }
        };
        let mut i = 0;
        while i < src.len() {
            let c = src[i];
            let next = src.get(i + 1).copied();
            if c == '/' && next == Some('/') {
                let end = src[i..]
                    .iter()
                    .position(|&c| c == '\n')
                    .map_or(src.len(), |p| i + p);
                comments.push((i, src[i + 2..end].iter().collect()));
                blank(&mut code, i, end);
                i = end;
            } else if c == '/' && next == Some('*') {
                let mut depth = 1;
                let mut j = i + 2;
                while j < src.len() && depth > 0 {
                    if src[j] == '/' && src.get(j + 1) == Some(&'*') {
                        depth += 1;
                        j += 2;
                    } else if src[j] == '*' && src.get(j + 1) == Some(&'/') {
                        depth -= 1;
                        j += 2;
                    } else {
                        j += 1;
                    }
                }
                blank(&mut code, i, j);
                i = j;
            } else if let Some(end) = raw_string_end(&src, i) {
                blank(&mut code, i, end);
                i = end;
            } else if c == '"' {
                let mut j = i + 1;
                while j < src.len() && src[j] != '"' {
                    j += if src[j] == '\\' { 2 } else { 1 };
                }
                let end = (j + 1).min(src.len());
                blank(&mut code, i + 1, end.saturating_sub(1).max(i + 1));
                i = end;
            } else if c == '\'' {
                // A char literal is `'x'` or `'\...'`; anything else is a lifetime.
                let end = if next == Some('\\') {
                    src[i + 3..]
                        .iter()
                        .position(|&c| c == '\'')
                        .map(|p| i + 4 + p)
                } else if src.get(i + 2) == Some(&'\'') {
                    Some(i + 3)
                } else {
                    None
                };
                match end {
                    Some(end) => {
                        blank(&mut code, i + 1, end - 1);
                        i = end;
                    }
                    None => i += 1,
                }
            } else {
                i += 1;
            }
        }
        Scan { code, comments }
    }

    /// 1-based line of an index.
    fn line_of(&self, pos: usize) -> usize {
        1 + self.code[..pos].iter().filter(|&&c| c == '\n').count()
    }

    /// Index just past each `#[...]` attribute whose path is `test` or ends in `::test`.
    fn test_attributes(&self) -> Vec<usize> {
        let mut out = Vec::new();
        let mut i = 0;
        while i + 1 < self.code.len() {
            if self.code[i] == '#' && self.code[i + 1] == '[' {
                let end = self
                    .matching(i + 1, '[', ']')
                    .unwrap_or(self.code.len() - 1)
                    + 1;
                let path: String = self.code[i + 2..end - 1]
                    .iter()
                    .take_while(|c| {
                        c.is_alphanumeric() || **c == '_' || **c == ':' || c.is_whitespace()
                    })
                    .collect();
                let path = path.trim();
                if path == "test" || path.ends_with("::test") {
                    out.push(end);
                }
                i = end;
            } else {
                i += 1;
            }
        }
        out
    }

    /// The `fn` that the attribute ending at `from` decorates: its keyword index and body span
    /// (inside the braces). `None` for anything else, or a function without a body.
    fn function_after(&self, from: usize) -> Option<(usize, (usize, usize))> {
        let mut i = from;
        let fn_pos = loop {
            let c = *self.code.get(i)?;
            if c == '#' && self.code.get(i + 1) == Some(&'[') {
                i = self.matching(i + 1, '[', ']')? + 1;
            } else if c == '{' || c == '}' || c == ';' {
                return None;
            } else if c == 'f'
                && self.code.get(i + 1) == Some(&'n')
                && self.code.get(i + 2).is_some_and(|c| c.is_whitespace())
                && (i == 0 || !is_ident(self.code[i - 1]))
            {
                break i;
            } else {
                i += 1;
            }
        };
        let open =
            (fn_pos..self.code.len()).find(|&j| self.code[j] == '{' || self.code[j] == ';')?;
        if self.code[open] == ';' {
            return None;
        }
        let close = self.matching(open, '{', '}')?;
        Some((fn_pos, (open, close)))
    }

    /// Index of the bracket closing the one at `open`.
    fn matching(&self, open: usize, up: char, down: char) -> Option<usize> {
        let mut depth = 0usize;
        for j in open..self.code.len() {
            if self.code[j] == up {
                depth += 1;
            } else if self.code[j] == down {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
        }
        None
    }

    /// Whether `// Given`, `// When` and `// Then` markers occur in the line comments within `from..to`.
    fn markers_in(&self, from: usize, to: usize) -> [bool; 3] {
        let mut found = [false; 3];
        for (start, text) in &self.comments {
            if *start < from || *start >= to {
                continue;
            }
            let text = text.trim();
            // A marker line also counts for the later words it names: `// Given / When`,
            // `// When and Then`.
            let named = |word| text.split(|c: char| !is_ident(c)).any(|w| w == word);
            if starts_with_word(text, "Given") {
                found[0] = true;
                found[1] |= named("When");
                found[2] |= named("Then");
            } else if starts_with_word(text, "When") {
                found[1] = true;
                found[2] |= named("Then");
            } else if starts_with_word(text, "Then") {
                found[2] = true;
            }
        }
        found
    }
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn starts_with_word(text: &str, word: &str) -> bool {
    text.strip_prefix(word)
        .is_some_and(|rest| !rest.starts_with(is_ident))
}

/// If a raw string literal (`r"..."`, `r#"..."#`, `br#"..."#`) starts at `i`, the index just past it.
fn raw_string_end(src: &[char], i: usize) -> Option<usize> {
    if i > 0 && is_ident(src[i - 1]) {
        return None;
    }
    let mut j = i;
    if src.get(j) == Some(&'b') {
        j += 1;
    }
    if src.get(j) != Some(&'r') {
        return None;
    }
    j += 1;
    let hashes = src[j..].iter().take_while(|&&c| c == '#').count();
    j += hashes;
    if src.get(j) != Some(&'"') {
        return None;
    }
    j += 1;
    while j < src.len() {
        if src[j] == '"'
            && src[j + 1..]
                .iter()
                .take(hashes)
                .filter(|&&c| c == '#')
                .count()
                == hashes
        {
            return Some(j + 1 + hashes);
        }
        j += 1;
    }
    Some(src.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn violations(text: &str) -> Vec<Violation> {
        GivenWhenThen.check(&TextFile {
            path: "crates/x/src/lib.rs".into(),
            text: text.into(),
        })
    }

    #[test]
    fn test_with_all_three_markers_passes() {
        // Given
        let src = "#[test]\nfn ok() {\n    // Given the inputs\n    let a = 1;\n    // When run\n    let b = a;\n    // Then equal\n    assert_eq!(a, b);\n}\n";
        // When
        let found = violations(src);
        // Then
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn missing_then_is_reported_at_the_test_line() {
        // Given
        let src = "mod t {\n    #[test]\n    #[ignore]\n    fn lacks_then() {\n        // Given\n        // When\n        let _ = 1;\n    }\n}\n";
        // When
        let found = violations(src);
        // Then
        assert_eq!(
            found,
            vec![Violation {
                file: "crates/x/src/lib.rs".into(),
                line: 4,
                message: "test is missing // Then".into(),
            }]
        );
    }

    #[test]
    fn combined_when_then_counts() {
        // Given
        let combined = ["// When and Then", "// When / Then", "// When/Then"];
        for marker in combined {
            let src = format!(
                "#[tokio::test(flavor = \"multi_thread\")]\nasync fn t() {{\n    // Given\n    {marker}\n    assert!(true);\n}}\n"
            );
            // When
            let found = violations(&src);
            // Then
            assert!(found.is_empty(), "{marker}: {found:?}");
        }
    }

    #[test]
    fn given_when_combined_counts() {
        // Given
        let src = "#[test]\nfn t() {\n    // Given / When\n    let x = 1;\n    // Then\n    assert_eq!(x, 1);\n}\n";
        // When
        let found = violations(src);
        // Then
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn non_test_functions_are_ignored() {
        // Given
        let src = "fn helper() {\n    let _ = 1;\n}\n\n#[cfg(test)]\nmod tests {\n    fn also_helper() {}\n}\n";
        // When
        let found = violations(src);
        // Then
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn braces_inside_strings_do_not_confuse_matching() {
        // Given
        let src = "#[test]\nfn first() {\n    let s = \"}\";\n    let c = '}';\n    /* } */\n    let r = r#\"} // Given \"#;\n    // Given\n    // When\n}\n\n#[test]\nfn second() {\n    // Given\n    // When\n    // Then\n}\n";
        // When
        let found = violations(src);
        // Then
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].line, 2);
        assert_eq!(found[0].message, "test is missing // Then");
    }

    #[test]
    fn markers_inside_strings_do_not_count() {
        // Given
        let src = "#[test]\nfn t() {\n    let _s = \"// Given // When // Then\";\n}\n";
        // When
        let found = violations(src);
        // Then
        assert_eq!(
            found[0].message,
            "test is missing // Given, // When, // Then"
        );
    }

    #[test]
    fn sqlx_test_with_arguments_is_a_test() {
        // Given
        let src =
            "#[sqlx::test(migrations = \"../m\")]\nasync fn t(pool: Pool) {\n    // Given\n}\n";
        // When
        let found = violations(src);
        // Then
        assert_eq!(found[0].message, "test is missing // When, // Then");
    }
}
