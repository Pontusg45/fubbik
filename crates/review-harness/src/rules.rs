//! Pattern rules loaded from a `review-rules.toml` file.

use std::fs;
use std::path::Path;

use anyhow::Context;
use regex::Regex;
use serde::Deserialize;

use crate::{Check, TextFile, Violation};

/// The starter rule file written by `review-harness init`.
pub const EXAMPLE_RULES: &str = include_str!("../review-rules.example.toml");

/// A line-by-line regex rule.
#[derive(Debug)]
pub struct PatternRule {
    pub name: String,
    pub description: String,
    pub pattern: Regex,
    pub roots: Vec<String>,
    pub extensions: Vec<String>,
    pub message: String,
    /// Repo-relative, glob-free path prefixes exempt from the rule.
    pub allow: Vec<String>,
}

impl Check for PatternRule {
    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        &self.description
    }
    fn roots(&self) -> Vec<&str> {
        self.roots.iter().map(String::as_str).collect()
    }
    fn extensions(&self) -> Vec<&str> {
        self.extensions.iter().map(String::as_str).collect()
    }
    fn check(&self, file: &TextFile) -> Vec<Violation> {
        if self.allow.iter().any(|p| file.path.starts_with(p.as_str())) {
            return Vec::new();
        }
        file.text
            .lines()
            .enumerate()
            .filter(|(_, line)| self.pattern.is_match(line))
            .map(|(i, _)| Violation {
                file: file.path.clone(),
                line: i + 1,
                message: self.message.clone(),
            })
            .collect()
    }
}

#[derive(Deserialize)]
struct RawFile {
    #[serde(default)]
    rule: Vec<RawRule>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRule {
    name: String,
    description: String,
    pattern: String,
    roots: Vec<String>,
    extensions: Vec<String>,
    message: String,
    #[serde(default)]
    allow: Vec<String>,
}

/// Load every `[[rule]]` from a TOML file. Errors name the rule and the field.
pub fn load_rules(path: &Path) -> anyhow::Result<Vec<PatternRule>> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let raw: RawFile =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    raw.rule
        .into_iter()
        .map(|r| {
            let pattern = Regex::new(&r.pattern)
                .map_err(|e| anyhow::anyhow!("rule \"{}\": invalid pattern: {e}", r.name))?;
            Ok(PatternRule {
                name: r.name,
                description: r.description,
                pattern,
                roots: r.roots,
                extensions: r.extensions,
                message: r.message,
                allow: r.allow,
            })
        })
        .collect()
}

/// Write the example rule file into `root`, refusing to overwrite.
pub fn init(root: &Path) -> anyhow::Result<()> {
    let target = root.join("review-rules.toml");
    if target.exists() {
        anyhow::bail!("review-rules.toml already exists");
    }
    fs::write(&target, EXAMPLE_RULES).with_context(|| format!("writing {}", target.display()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("review-harness-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    const ONE_RULE: &str = r#"
[[rule]]
name = "no-as-child"
description = "no asChild"
pattern = '\basChild\b'
roots = ["apps/web/src"]
extensions = [".tsx"]
message = "use render"
allow = ["apps/web/src/legacy/"]
"#;

    fn load(name: &str, toml: &str) -> anyhow::Result<Vec<PatternRule>> {
        let dir = temp_dir(name);
        let path = dir.join("review-rules.toml");
        fs::write(&path, toml).unwrap();
        let result = load_rules(&path);
        fs::remove_dir_all(dir).unwrap();
        result
    }

    #[test]
    fn loads_rules_from_toml() {
        // Given / When
        let rules = load("load", ONE_RULE).unwrap();
        // Then
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].name(), "no-as-child");
        assert_eq!(rules[0].roots(), vec!["apps/web/src"]);
        assert_eq!(rules[0].extensions(), vec![".tsx"]);
        assert_eq!(rules[0].allow, vec!["apps/web/src/legacy/"]);
    }

    #[test]
    fn reports_line_numbers_for_matches() {
        // Given
        let rule = load("lines", ONE_RULE).unwrap().remove(0);
        let file = TextFile {
            path: "apps/web/src/a.tsx".into(),
            text: "ok\n<X asChild />\nfine\n<Y asChild/>\n".into(),
        };
        // When
        let violations = rule.check(&file);
        // Then
        let lines: Vec<usize> = violations.iter().map(|v| v.line).collect();
        assert_eq!(lines, vec![2, 4]);
        assert_eq!(violations[0].message, "use render");
        assert_eq!(violations[0].file, "apps/web/src/a.tsx");
    }

    #[test]
    fn allow_prefix_exempts_files() {
        // Given
        let rule = load("allow", ONE_RULE).unwrap().remove(0);
        let file = TextFile {
            path: "apps/web/src/legacy/old.tsx".into(),
            text: "<X asChild />\n".into(),
        };
        // When / Then
        assert!(rule.check(&file).is_empty());
    }

    #[test]
    fn invalid_regex_names_the_rule() {
        // Given
        let toml = ONE_RULE.replace(r"'\basChild\b'", "'(unclosed'");
        // When
        let err = load("badre", &toml).unwrap_err();
        // Then
        let msg = format!("{err:#}");
        assert!(
            msg.contains(r#"rule "no-as-child": invalid pattern:"#),
            "{msg}"
        );
    }

    #[test]
    fn init_writes_a_loadable_file_and_refuses_to_overwrite() {
        // Given
        let dir = temp_dir("init-unit");
        // When
        init(&dir).unwrap();
        let again = init(&dir).unwrap_err();
        // Then
        assert!(load_rules(&dir.join("review-rules.toml")).is_ok());
        assert!(
            again
                .to_string()
                .contains("review-rules.toml already exists")
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
