//! Direct port of `packages/api/src/requirements/export.ts` — pure string
//! formatting, no SQL, no I/O. Backs `GET /requirements/{id}/export` and
//! `GET /requirements/export`.

use fubbik_db::repo::requirement::RequirementStep;

/// Replaces `{key}` placeholders in `text` with `params[key]`, leaving an
/// unmatched placeholder untouched (`{key}` stays literal text) — matches
/// Node's `interpolate` exactly (`packages/api/src/requirements/export.ts:
/// 3-6`): `text.replace(/\{(\w+)\}/g, (_, key) => params[key] ?? `{${key}}`)`.
/// `\w` in JS is `[A-Za-z0-9_]`; reproduced here with `is_word_char` rather
/// than pulling in a `regex` dependency this crate otherwise avoids (see
/// `search::parser::tokenize`'s equivalent hand-rolled scan).
fn interpolate(text: &str, params: Option<&std::collections::HashMap<String, String>>) -> String {
    let Some(params) = params else {
        return text.to_string();
    };
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            let mut j = i + 1;
            while j < chars.len() && is_word_char(chars[j]) {
                j += 1;
            }
            if j > i + 1 && j < chars.len() && chars[j] == '}' {
                let key: String = chars[i + 1..j].iter().collect();
                match params.get(&key) {
                    Some(value) => out.push_str(value),
                    None => {
                        out.push('{');
                        out.push_str(&key);
                        out.push('}');
                    }
                }
                i = j + 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn capitalize(keyword: &str) -> String {
    let mut chars = keyword.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Matches Node's `toGherkin` (`packages/api/src/requirements/export.ts:12-20`).
pub fn to_gherkin(title: &str, steps: &[RequirementStep]) -> String {
    let mut lines = vec![
        format!("Feature: {title}"),
        String::new(),
        format!("  Scenario: {title}"),
    ];
    for step in steps {
        let keyword = capitalize(step.keyword.as_str());
        let text = interpolate(&step.text, step.params.as_ref());
        lines.push(format!("    {keyword} {text}"));
    }
    lines.join("\n")
}

/// Matches Node's `toVitest` (`packages/api/src/requirements/export.ts:22-39`).
pub fn to_vitest(title: &str, steps: &[RequirementStep]) -> String {
    let step_comments = steps
        .iter()
        .map(|s| {
            let keyword = capitalize(s.keyword.as_str());
            let text = interpolate(&s.text, s.params.as_ref());
            format!("    // {keyword} {text}")
        })
        .collect::<Vec<_>>()
        .join("\n");

    [
        format!("describe(\"{title}\", () => {{"),
        format!("  it(\"{title}\", () => {{"),
        step_comments,
        "    throw new Error(\"Not implemented\");".to_string(),
        "  });".to_string(),
        "});".to_string(),
    ]
    .join("\n")
}

/// Matches Node's `toMarkdown` (`packages/api/src/requirements/export.ts:41-49`).
pub fn to_markdown(title: &str, steps: &[RequirementStep]) -> String {
    let mut lines = vec![format!("# {title}"), String::new()];
    for step in steps {
        let keyword = capitalize(step.keyword.as_str());
        let text = interpolate(&step.text, step.params.as_ref());
        lines.push(format!("- [ ] **{keyword}** {text}"));
    }
    lines.join("\n")
}

/// Matches Node's `exportOne` switch (`packages/api/src/requirements/
/// service.ts:280-295`): an unrecognised `format` falls back to markdown,
/// same as Node's `default` branch.
pub fn export_one(title: &str, steps: &[RequirementStep], format: &str) -> String {
    match format {
        "gherkin" => to_gherkin(title, steps),
        "vitest" => to_vitest(title, steps),
        _ => to_markdown(title, steps),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fubbik_db::repo::requirement::StepKeyword;

    fn step(keyword: StepKeyword, text: &str) -> RequirementStep {
        RequirementStep {
            keyword,
            text: text.to_string(),
            params: None,
        }
    }

    #[test]
    fn gherkin_matches_node_shape() {
        let steps = vec![
            step(StepKeyword::Given, "a user"),
            step(StepKeyword::When, "they log in"),
            step(StepKeyword::Then, "they see the dashboard"),
        ];
        let out = to_gherkin("Login", &steps);
        assert_eq!(
            out,
            "Feature: Login\n\n  Scenario: Login\n    Given a user\n    When they log in\n    Then they see the dashboard"
        );
    }

    #[test]
    fn interpolate_leaves_unmatched_placeholder_literal() {
        let mut params = std::collections::HashMap::new();
        params.insert("name".to_string(), "Alice".to_string());
        let mut s = step(StepKeyword::Given, "{name} logs in as {role}");
        s.params = Some(params);
        let out = to_markdown("T", std::slice::from_ref(&s));
        assert!(out.contains("Alice logs in as {role}"));
    }

    #[test]
    fn unknown_format_falls_back_to_markdown() {
        let steps = vec![step(StepKeyword::Given, "x")];
        assert_eq!(export_one("T", &steps, "bogus"), to_markdown("T", &steps));
    }
}
