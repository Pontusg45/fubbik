use std::collections::{HashMap, HashSet};

use fubbik_db::repo::template::{ExtractionTarget, FieldMapping, MatchMode, Template};

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ParsedDoc {
    pub title: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub tags: Vec<String>,
    pub content: String,
}

#[derive(Debug, Default, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alternatives: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consequences: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<HashMap<String, String>>,
}

#[derive(Clone)]
struct Heading {
    level: usize,
    text: String,
}

pub fn parse_doc(path: &str, raw: &str) -> ParsedDoc {
    let (frontmatter, body) = parse_frontmatter(raw);
    let mut content = body;
    let mut title = frontmatter
        .get("title")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    if title.is_none()
        && let Some((start, end, heading)) = first_h1(&content)
    {
        title = Some(heading);
        content.replace_range(start..end, "");
        content = content.trim().to_owned();
    }
    let title = title.unwrap_or_else(|| {
        let name = path.rsplit('/').next().unwrap_or(path);
        let stem = if name.len() >= 3 && name[name.len() - 3..].eq_ignore_ascii_case(".md") {
            &name[..name.len() - 3]
        } else {
            name
        };
        stem.replace(['-', '_'], " ")
    });
    let mut tags = frontmatter
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .map(|v| v.as_str().unwrap_or_default().to_owned())
        .collect::<Vec<_>>();
    tags.extend(super::frontmatter::tags_from_path(path));
    let mut seen = HashSet::new();
    tags.retain(|tag| seen.insert(tag.clone()));
    let doc_type = frontmatter
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("document")
        .to_owned();
    ParsedDoc {
        title,
        doc_type,
        tags,
        content: content.trim().to_owned(),
    }
}

pub fn best_template<'a>(raw: &str, templates: &'a [Template]) -> Option<(&'a Template, f64)> {
    let (frontmatter, _) = parse_frontmatter(raw);
    let headings = headings(raw);
    let mut scored = templates
        .iter()
        .filter_map(|template| {
            template.match_rules.as_ref().and_then(|rules| {
                let rules = &rules.0;
                let mut score = 0.0;
                for rule in &rules.headings {
                    let matched = headings.iter().any(|heading| {
                        rule.level
                            .is_none_or(|level| level as usize == heading.level)
                            && rule
                                .patterns
                                .iter()
                                .any(|p| heading_matches(p, &heading.text, rule.match_mode))
                    });
                    if rule.required && !matched {
                        return None;
                    }
                    if matched {
                        score += if rule.required { 1.0 } else { 0.5 };
                    }
                }
                for rule in &rules.frontmatter {
                    let value = frontmatter.get(&rule.key);
                    let matched = match rule.match_mode {
                        fubbik_db::repo::template::FrontmatterMatchMode::Exists => value.is_some(),
                        fubbik_db::repo::template::FrontmatterMatchMode::Exact => {
                            value.map(value_string).as_deref() == rule.value.as_deref()
                        }
                        fubbik_db::repo::template::FrontmatterMatchMode::OneOf => {
                            value.map(value_string).is_some_and(|v| {
                                rule.values
                                    .as_ref()
                                    .is_some_and(|values| values.contains(&v))
                            })
                        }
                    };
                    if matched {
                        score += 1.0;
                    }
                }
                (score >= rules.min_score && score > 0.0).then_some((template, score))
            })
        })
        .collect::<Vec<_>>();
    scored.sort_by(|(a, ascore), (b, bscore)| {
        bscore
            .total_cmp(ascore)
            .then_with(|| b.priority.cmp(&a.priority))
            .then_with(|| required_count(b).cmp(&required_count(a)))
    });
    scored.into_iter().next()
}

pub fn extract_fields(raw: &str, mappings: &[FieldMapping]) -> (ExtractedFields, String) {
    if mappings.is_empty() {
        return (ExtractedFields::default(), raw.to_owned());
    }
    let sections = sections(raw);
    let mut extracted = ExtractedFields::default();
    let mut consumed = HashSet::new();
    for mapping in mappings {
        let Some((index, section)) = sections.iter().enumerate().find(|(_, section)| {
            section.heading.as_ref().is_some_and(|heading| {
                mapping
                    .headings
                    .iter()
                    .any(|pattern| heading_matches(pattern, heading, mapping.match_mode))
            })
        }) else {
            continue;
        };
        let content = section.content.trim().to_owned();
        if mapping.target == ExtractionTarget::Content {
            continue;
        }
        consumed.insert(index);
        match mapping.target {
            ExtractionTarget::Rationale => extracted.rationale = Some(content),
            ExtractionTarget::Alternatives => {
                extracted.alternatives = Some(split_bullets(&content))
            }
            ExtractionTarget::Consequences => extracted.consequences = Some(content),
            ExtractionTarget::Summary => extracted.summary = Some(content),
            ExtractionTarget::Scope => extracted.scope = Some(parse_scope(&content)),
            ExtractionTarget::Content => {}
        }
    }
    let remaining = sections
        .iter()
        .enumerate()
        .filter(|(i, _)| !consumed.contains(i))
        .map(|(_, section)| match (&section.heading, section.level) {
            (Some(heading), Some(level)) => {
                format!("{} {heading}\n{}", "#".repeat(level), section.content)
            }
            _ => section.content.clone(),
        })
        .collect::<Vec<_>>()
        .join("")
        .trim_end()
        .to_owned();
    (
        extracted,
        if remaining.is_empty() {
            raw.to_owned()
        } else {
            remaining
        },
    )
}

fn heading_matches(pattern: &str, heading: &str, mode: MatchMode) -> bool {
    let pattern = pattern.to_lowercase();
    let heading = heading.to_lowercase();
    match mode {
        MatchMode::Exact => heading == pattern,
        MatchMode::Prefix => heading.starts_with(&pattern),
        MatchMode::Contains => heading.contains(&pattern),
    }
}

fn headings(raw: &str) -> Vec<Heading> {
    raw.lines()
        .filter_map(|line| {
            let level = line.chars().take_while(|c| *c == '#').count();
            (level > 0 && level <= 6 && line.as_bytes().get(level) == Some(&b' ')).then(|| {
                Heading {
                    level,
                    text: line[level + 1..].trim().to_owned(),
                }
            })
        })
        .collect()
}

fn first_h1(raw: &str) -> Option<(usize, usize, String)> {
    let mut offset = 0;
    for line in raw.split_inclusive('\n') {
        let bare = line.trim_end_matches('\n');
        if let Some(title) = bare.strip_prefix("# ") {
            return Some((offset, offset + line.len(), title.trim().to_owned()));
        }
        offset += line.len();
    }
    None
}

fn parse_frontmatter(raw: &str) -> (HashMap<String, serde_json::Value>, String) {
    let Some(rest) = raw.strip_prefix("---\n") else {
        return (HashMap::new(), raw.to_owned());
    };
    let Some(end) = rest.find("\n---") else {
        return (HashMap::new(), raw.to_owned());
    };
    let yaml = &rest[..end];
    let body = rest[end + 4..]
        .strip_prefix('\n')
        .unwrap_or(&rest[end + 4..])
        .trim()
        .to_owned();
    let mut map = HashMap::new();
    let lines = yaml.lines().collect::<Vec<_>>();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if !line.starts_with(char::is_whitespace)
            && let Some((key, value)) = line.split_once(':')
        {
            let value = value.trim();
            if !value.is_empty() {
                map.insert(key.to_owned(), serde_json::Value::String(value.to_owned()));
            } else {
                let mut array = Vec::new();
                let mut object = serde_json::Map::new();
                while i + 1 < lines.len() && lines[i + 1].starts_with(char::is_whitespace) {
                    i += 1;
                    let nested = lines[i].trim();
                    if let Some(item) = nested.strip_prefix("- ") {
                        array.push(serde_json::Value::String(item.trim().to_owned()));
                    } else if let Some((k, v)) = nested.split_once(':') {
                        object.insert(
                            k.trim().to_owned(),
                            serde_json::Value::String(v.trim().to_owned()),
                        );
                    }
                }
                if !array.is_empty() {
                    map.insert(key.to_owned(), serde_json::Value::Array(array));
                } else if !object.is_empty() {
                    map.insert(key.to_owned(), serde_json::Value::Object(object));
                }
            }
        }
        i += 1;
    }
    (map, body)
}

fn value_string(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}
fn required_count(template: &Template) -> usize {
    template
        .match_rules
        .as_ref()
        .map_or(0, |r| r.0.headings.iter().filter(|h| h.required).count())
}

struct Section {
    heading: Option<String>,
    level: Option<usize>,
    content: String,
}
fn sections(raw: &str) -> Vec<Section> {
    let mut out = Vec::new();
    let mut heading = None;
    let mut level = None;
    let mut buffer = Vec::new();
    for line in raw.split('\n') {
        let count = line.chars().take_while(|c| *c == '#').count();
        if (2..=6).contains(&count) && line.as_bytes().get(count) == Some(&b' ') {
            out.push(Section {
                heading,
                level,
                content: buffer.join("\n"),
            });
            heading = Some(line[count + 1..].trim().to_owned());
            level = Some(count);
            buffer.clear();
        } else {
            buffer.push(line);
        }
    }
    out.push(Section {
        heading,
        level,
        content: buffer.join("\n"),
    });
    out
}
fn split_bullets(content: &str) -> Vec<String> {
    let v = content
        .lines()
        .filter_map(|l| {
            l.trim_start()
                .strip_prefix("- ")
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    if v.is_empty() && !content.trim().is_empty() {
        vec![content.trim().to_owned()]
    } else {
        v
    }
}
fn parse_scope(content: &str) -> HashMap<String, String> {
    content
        .lines()
        .filter_map(|line| {
            line.split_once(':')
                .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
                .filter(|(k, _)| !k.is_empty())
        })
        .collect()
}
