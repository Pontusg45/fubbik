//! Search query DSL parser.
//!
//! Direct port of `packages/api/src/search/parser.ts`. This parser never
//! fails: unknown fields become `{field, operator: "is", value}` and are
//! silently ignored downstream (by the service layer, not here). There is
//! deliberately no error type — `parse_query_string` always returns a
//! `Vec<QueryClause>`, even for garbage input.
//!
//! A handful of behaviours look like bugs but are exact ports of Node's
//! semantics — see the doc comments on `resolve_operator_and_value` and the
//! `hops:` handling in `parse_query_string`. Do not "fix" them; they are
//! pinned by `crates/fubbik-api/tests/search_parser.rs`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct QueryClause {
    pub field: String,
    pub operator: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub params: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub negate: Option<bool>,
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

/// Splits a query string into tokens, respecting double-quoted strings.
/// Quoted strings are returned with their quotes stripped.
///
/// e.g. `type:reference "bare text" NOT tag:api` ->
/// `["type:reference", "bare text", "NOT", "tag:api"]`
///
/// There is no escape handling: a `\"` inside a quoted run is not special,
/// the loop just reads up to the next `"`.
fn tokenize(input: &str) -> Vec<String> {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < len {
        while i < len && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= len {
            break;
        }

        if chars[i] == '"' {
            i += 1; // skip opening quote
            let mut token = String::new();
            while i < len && chars[i] != '"' {
                token.push(chars[i]);
                i += 1;
            }
            if i < len {
                i += 1; // skip closing quote
            }
            tokens.push(token);
        } else {
            let mut token = String::new();
            while i < len && !chars[i].is_whitespace() {
                token.push(chars[i]);
                i += 1;
            }
            tokens.push(token);
        }
    }

    tokens
}

/// Strips at most one leading and one trailing `"` — the Rust equivalent of
/// the JS `.replace(/^"|"$/g, "")`, which (despite the `g` flag) only ever
/// removes a quote anchored at the very start and/or very end of the string,
/// never quotes in the middle.
fn strip_quotes(s: &str) -> String {
    let mut chars: Vec<char> = s.chars().collect();
    if chars.first() == Some(&'"') {
        chars.remove(0);
    }
    if chars.last() == Some(&'"') {
        chars.pop();
    }
    chars.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Field-specific operator detection
// ---------------------------------------------------------------------------

struct Resolved {
    operator: String,
    value: String,
    params: Option<BTreeMap<String, String>>,
}

/// Given a raw `field:rawValue` token (already split on the first `:`),
/// determine the appropriate operator and normalise the value.
///
/// Deliberate, non-obvious behaviour ported verbatim from Node:
/// - `connections:3+` -> `gte` (trailing `+` stripped). `connections:3`
///   with no `+` stays `is` — it is NOT promoted to `gte`.
/// - `updated:30d` -> `within` (trailing `d` stripped, requires the whole
///   value to be `^\d+d$`). `updated:30` with no `d` stays `is`.
/// - `tag:a,b` -> `any_of`, but the value is left unsplit as `"a,b"`; the
///   comma-splitting happens downstream in the service, not here.
/// - `path:"A"->"B"` splits on the first `->`, re-strips residual quotes
///   from each side, and reports operator `is` with `value = from` plus
///   `params = {from, to}`.
fn resolve_operator_and_value(field: &str, raw_value: &str) -> Resolved {
    if field == "connections"
        && let Some(stripped) = raw_value.strip_suffix('+')
    {
        return Resolved {
            operator: "gte".to_string(),
            value: stripped.to_string(),
            params: None,
        };
    }

    if field == "updated" && is_digits_then_d(raw_value) {
        let stripped = &raw_value[..raw_value.len() - 1];
        return Resolved {
            operator: "within".to_string(),
            value: stripped.to_string(),
            params: None,
        };
    }

    if field == "tag" && raw_value.contains(',') {
        return Resolved {
            operator: "any_of".to_string(),
            value: raw_value.to_string(),
            params: None,
        };
    }

    if field == "path"
        && let Some(arrow_idx) = raw_value.find("->")
    {
        let from = strip_quotes(&raw_value[..arrow_idx]);
        let to = strip_quotes(&raw_value[arrow_idx + 2..]);
        let mut params = BTreeMap::new();
        params.insert("from".to_string(), from.clone());
        params.insert("to".to_string(), to);
        return Resolved {
            operator: "is".to_string(),
            value: from,
            params: Some(params),
        };
    }

    Resolved {
        operator: "is".to_string(),
        value: raw_value.to_string(),
        params: None,
    }
}

/// Matches JS `/^\d+d$/`: one or more ASCII digits followed by a literal `d`.
fn is_digits_then_d(s: &str) -> bool {
    match s.strip_suffix('d') {
        Some(digits) => !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()),
        None => false,
    }
}

/// Matches JS `/^hops:\d+$/`.
fn hops_value(token: &str) -> Option<&str> {
    let digits = token.strip_prefix("hops:")?;
    if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
        Some(digits)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parses a structured query string into a list of `QueryClause`s. Never
/// fails — there is no error type in this parser, matching Node.
///
/// Supported syntax:
/// - `type:reference`
/// - `tag:api`, `tag:api,auth` (-> `any_of`)
/// - `connections:3+` (-> `gte`)
/// - `updated:30d` (-> `within`)
/// - `near:"Auth Flow" hops:2` (hops attaches to the most recent `near`
///   clause; it is not a standalone clause, and scanning is backwards-only —
///   a `hops:` after any non-`near` clause, or with no preceding `near` at
///   all, silently produces no effect)
/// - `path:"A"->"B"` (-> params `from`/`to`)
/// - `NOT tag:deprecated` (negates only the very next clause)
/// - bare words / `"quoted phrase"` -> `{field: "text", operator: "contains"}`
pub fn parse_query_string(input: &str) -> Vec<QueryClause> {
    let tokens = tokenize(input);
    let mut clauses: Vec<QueryClause> = Vec::new();
    let mut negate = false;

    for token in tokens {
        if token == "NOT" {
            negate = true;
            continue;
        }

        if let Some(hops) = hops_value(&token) {
            if let Some(last_near) = clauses.iter_mut().rev().find(|c| c.field == "near") {
                let params = last_near.params.get_or_insert_with(BTreeMap::new);
                params.insert("hops".to_string(), hops.to_string());
            }
            negate = false;
            continue;
        }

        let colon_idx = token.find(':');
        if let Some(idx) = colon_idx
            && idx > 0
        {
            let field = token[..idx].to_string();
            let raw_value = strip_quotes(&token[idx + 1..]);

            let resolved = resolve_operator_and_value(&field, &raw_value);

            clauses.push(QueryClause {
                field,
                operator: resolved.operator,
                value: resolved.value,
                params: resolved.params,
                negate: if negate { Some(true) } else { None },
            });
            negate = false;
            continue;
        }

        // Bare text (already unquoted by the tokenizer if it was quoted in the input).
        clauses.push(QueryClause {
            field: "text".to_string(),
            operator: "contains".to_string(),
            value: token,
            params: None,
            negate: if negate { Some(true) } else { None },
        });
        negate = false;
    }

    clauses
}

// ---------------------------------------------------------------------------
// Serializer — inverse of parse_query_string
// ---------------------------------------------------------------------------

fn quote_if_space(v: &str) -> String {
    if v.contains(' ') {
        format!("\"{v}\"")
    } else {
        v.to_string()
    }
}

/// Converts a list of `QueryClause`s back to a query string. Used to keep
/// pill state and a text input in sync. Exact inverse of
/// `parse_query_string`.
pub fn clauses_to_query_string(clauses: &[QueryClause]) -> String {
    clauses
        .iter()
        .map(|clause| {
            let prefix = if clause.negate == Some(true) {
                "NOT "
            } else {
                ""
            };

            if clause.field == "text" {
                return format!("{prefix}{}", quote_if_space(&clause.value));
            }

            if clause.field == "path" {
                let from = clause
                    .params
                    .as_ref()
                    .and_then(|p| p.get("from"))
                    .filter(|v| !v.is_empty());
                let to = clause
                    .params
                    .as_ref()
                    .and_then(|p| p.get("to"))
                    .filter(|v| !v.is_empty());
                if let (Some(from), Some(to)) = (from, to) {
                    return format!(
                        "{prefix}path:{}->{}",
                        quote_if_space(from),
                        quote_if_space(to)
                    );
                }
            }

            if clause.field == "near" {
                let mut result = format!("{prefix}near:{}", quote_if_space(&clause.value));
                if let Some(hops) = clause
                    .params
                    .as_ref()
                    .and_then(|p| p.get("hops"))
                    .filter(|v| !v.is_empty())
                {
                    result.push_str(&format!(" hops:{hops}"));
                }
                return result;
            }

            if clause.operator == "gte" && clause.field == "connections" {
                return format!("{prefix}{}:{}+", clause.field, clause.value);
            }

            if clause.operator == "within" && clause.field == "updated" {
                return format!("{prefix}{}:{}d", clause.field, clause.value);
            }

            format!("{prefix}{}:{}", clause.field, quote_if_space(&clause.value))
        })
        .collect::<Vec<_>>()
        .join(" ")
}
