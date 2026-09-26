//! Native source-comment extraction. Each parser returns the existing versioned
//! manifest, so import and conflict handling remain independent of the parser.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use fubbik_core::source_docs::{SourceLanguage, SourceManifest, SourceSymbol};
use tree_sitter::{Node, Parser};

pub fn extract_javascript(root: &Path, project: &str) -> Result<SourceManifest> {
    let root = root
        .canonicalize()
        .context("could not resolve source root")?;
    if !root.is_dir() {
        bail!("source root is not a directory: {}", root.display());
    }
    let mut files = Vec::new();
    collect_files(&root, &mut files)?;
    files.sort();
    if files.is_empty() {
        bail!("No matching source files found");
    }

    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_javascript::LANGUAGE.into())
        .context("could not load JavaScript grammar")?;
    let mut symbols = Vec::new();
    for file in files {
        let source = std::fs::read_to_string(&file)
            .with_context(|| format!("could not read {}", file.display()))?;
        let tree = parser
            .parse(&source, None)
            .with_context(|| format!("could not parse {}", file.display()))?;
        if tree.root_node().has_error() {
            bail!("source parsing failed: {}", file.display());
        }
        let path = file
            .strip_prefix(&root)?
            .to_string_lossy()
            .replace('\\', "/");
        extract_js_file(tree.root_node(), &source, &path, &mut symbols)?;
    }
    symbols.sort_by(|left, right| left.key.cmp(&right.key));
    let manifest = SourceManifest {
        version: 1,
        project: project.to_owned(),
        language: SourceLanguage::Javascript,
        extractor: format!(
            "fubbik tree-sitter-javascript {}",
            env!("CARGO_PKG_VERSION")
        ),
        complete: true,
        diagnostics: Vec::new(),
        symbols,
    };
    manifest.validate().map_err(anyhow::Error::msg)?;
    Ok(manifest)
}

fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            if !matches!(
                entry.file_name().to_string_lossy().as_ref(),
                ".git" | "node_modules" | "target" | "build" | "dist" | ".next"
            ) {
                collect_files(&path, files)?;
            }
        } else if file_type.is_file()
            && matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("js" | "mjs" | "cjs")
            )
        {
            files.push(path);
        }
    }
    Ok(())
}

fn extract_js_file(
    root: Node<'_>,
    source: &str,
    path: &str,
    symbols: &mut Vec<SourceSymbol>,
) -> Result<()> {
    let mut cursor = root.walk();
    let mut preceding_doc = None;
    let mut consumed = HashSet::new();
    for node in root.named_children(&mut cursor) {
        if node.kind() == "comment" {
            let comment = text(node, source);
            preceding_doc = comment.starts_with("/**").then_some((node, comment));
            continue;
        }
        let declaration = if node.kind() == "export_statement" {
            node.child_by_field_name("declaration")
        } else {
            Some(node)
        };
        if let Some(class) =
            declaration.filter(|declaration| declaration.kind() == "class_declaration")
        {
            extract_class_methods(class, source, path, symbols, &mut consumed)?;
        }
        if let Some((comment_node, comment)) = preceding_doc.take()
            && let Some(declaration) = declaration
        {
            let name = match declaration.kind() {
                "function_declaration" | "class_declaration" => {
                    declaration.child_by_field_name("name")
                }
                "lexical_declaration" | "variable_declaration" => {
                    let mut children = declaration.walk();
                    let mut declarators = declaration
                        .named_children(&mut children)
                        .filter(|child| child.kind() == "variable_declarator");
                    let first = declarators.next();
                    if declarators.next().is_some() {
                        None
                    } else {
                        first.and_then(|child| child.child_by_field_name("name"))
                    }
                }
                _ => None,
            };
            if let Some(name) = name {
                consumed.insert(comment_node.start_byte());
                let title = text(name, source).to_owned();
                let documentation = JsDoc::parse(comment)?;
                let signature = if matches!(
                    declaration.kind(),
                    "lexical_declaration" | "variable_declaration"
                ) {
                    documentation.member_signature(&title)
                } else {
                    documentation.signature(&title)
                };
                symbols.push(SourceSymbol {
                    key: format!("{path}::{title}"),
                    title,
                    signature,
                    documentation: documentation.markdown(),
                    path: path.to_owned(),
                    line: declaration.start_position().row as u32 + 1,
                    references: documentation.see,
                });
            }
        }
    }
    reject_unhandled_jsdoc(root, source, path, &consumed)?;
    Ok(())
}

fn extract_class_methods(
    class: Node<'_>,
    source: &str,
    path: &str,
    symbols: &mut Vec<SourceSymbol>,
    consumed: &mut HashSet<usize>,
) -> Result<()> {
    let Some(name) = class.child_by_field_name("name") else {
        return Ok(());
    };
    let class_name = text(name, source);
    let Some(body) = class.child_by_field_name("body") else {
        return Ok(());
    };
    let mut cursor = body.walk();
    let mut preceding_doc = None;
    for node in body.named_children(&mut cursor) {
        if node.kind() == "comment" {
            let comment = text(node, source);
            preceding_doc = comment.starts_with("/**").then_some((node, comment));
            continue;
        }
        if let Some((comment_node, comment)) = preceding_doc.take()
            && node.kind() == "method_definition"
            && let Some(name) = node.child_by_field_name("name")
        {
            let method_name = text(name, source);
            let separator = if text(node, source).starts_with("static ") {
                "."
            } else {
                "#"
            };
            let title = format!("{class_name}{separator}{method_name}");
            let documentation = JsDoc::parse(comment)?;
            consumed.insert(comment_node.start_byte());
            symbols.push(SourceSymbol {
                key: format!("{path}::{title}"),
                signature: documentation.signature(&title),
                title,
                documentation: documentation.markdown(),
                path: path.to_owned(),
                line: node.start_position().row as u32 + 1,
                references: documentation.see,
            });
        }
    }
    Ok(())
}

fn reject_unhandled_jsdoc(
    node: Node<'_>,
    source: &str,
    path: &str,
    consumed: &HashSet<usize>,
) -> Result<()> {
    if node.kind() == "comment"
        && text(node, source).starts_with("/**")
        && !consumed.contains(&node.start_byte())
    {
        bail!(
            "source extraction cannot safely represent a documented declaration at {path}:{}",
            node.start_position().row + 1
        );
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        reject_unhandled_jsdoc(child, source, path, consumed)?;
    }
    Ok(())
}

fn text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

#[derive(Default)]
struct JsDoc {
    description: String,
    params: Vec<(String, String, String)>,
    returns: Vec<(String, String)>,
    throws: Vec<(String, String)>,
    examples: Vec<String>,
    deprecated: Option<String>,
    see: Vec<String>,
}

impl JsDoc {
    fn parse(comment: &str) -> Result<Self> {
        let mut doc = Self::default();
        let body = comment.trim_start_matches("/**").trim_end_matches("*/");
        let mut current_tag = String::new();
        for raw in body.lines() {
            let line = raw.trim().trim_start_matches('*').trim();
            if let Some(rest) = line.strip_prefix("@param ") {
                let (kind, value) = typed_value(rest);
                let (name, description) = value.split_once(' ').unwrap_or((value, ""));
                doc.params.push((
                    name.to_owned(),
                    kind,
                    description.trim_start_matches("- ").to_owned(),
                ));
                current_tag.clear();
            } else if let Some(rest) = line
                .strip_prefix("@returns ")
                .or_else(|| line.strip_prefix("@return "))
            {
                let (kind, description) = typed_value(rest);
                doc.returns.push((kind, description.to_owned()));
                current_tag.clear();
            } else if let Some(rest) = line
                .strip_prefix("@throws ")
                .or_else(|| line.strip_prefix("@exception "))
            {
                let (kind, description) = typed_value(rest);
                doc.throws.push((kind, description.to_owned()));
                current_tag.clear();
            } else if let Some(rest) = line.strip_prefix("@example") {
                doc.examples.push(rest.trim().to_owned());
                current_tag = "example".into();
            } else if let Some(rest) = line.strip_prefix("@deprecated") {
                doc.deprecated = Some(rest.trim().to_owned());
                current_tag.clear();
            } else if let Some(rest) = line.strip_prefix("@see ") {
                doc.see.push(rest.trim().to_owned());
                current_tag.clear();
            } else if line.starts_with('@') {
                bail!("unsupported JSDoc tag: {line}");
            } else if current_tag == "example" {
                if let Some(example) = doc.examples.last_mut() {
                    if !example.is_empty() {
                        example.push('\n');
                    }
                    example.push_str(line);
                }
            } else if !line.is_empty() {
                if !doc.description.is_empty() {
                    doc.description.push(' ');
                }
                doc.description.push_str(line);
            }
        }
        Ok(doc)
    }

    fn signature(&self, title: &str) -> String {
        let params = self
            .params
            .iter()
            .map(|(name, kind, _)| {
                format!(
                    "{name}{}",
                    if kind.is_empty() {
                        String::new()
                    } else {
                        format!(": {kind}")
                    }
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let returns = self
            .returns
            .iter()
            .map(|(kind, _)| kind.as_str())
            .filter(|kind| !kind.is_empty())
            .collect::<Vec<_>>()
            .join(" | ");
        if returns.is_empty() {
            format!("{title}({params})")
        } else {
            format!("{title}({params}): {returns}")
        }
    }

    fn member_signature(&self, title: &str) -> String {
        let returns = self
            .returns
            .iter()
            .map(|(kind, _)| kind.as_str())
            .filter(|kind| !kind.is_empty())
            .collect::<Vec<_>>()
            .join(" | ");
        if returns.is_empty() {
            title.to_owned()
        } else {
            format!("{title}: {returns}")
        }
    }

    fn markdown(&self) -> String {
        let mut rendered = self.description.clone();
        if !self.params.is_empty() {
            rendered.push_str("\n\n### Parameters\n\n");
            rendered.push_str(
                &self
                    .params
                    .iter()
                    .map(|(name, kind, description)| format!("- **{name}**: {kind} {description}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        if !self.returns.is_empty() {
            rendered.push_str("\n\n### Returns\n\n");
            rendered.push_str(
                &self
                    .returns
                    .iter()
                    .map(|(kind, description)| format!("{kind} {description}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        if !self.examples.is_empty() {
            rendered.push_str("\n\n### Examples\n\n");
            rendered.push_str(
                &self
                    .examples
                    .iter()
                    .map(|example| example.trim_end())
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            );
        }
        if !self.throws.is_empty() {
            rendered.push_str("\n\n### Throws\n\n");
            rendered.push_str(
                &self
                    .throws
                    .iter()
                    .map(|(kind, description)| format!("{kind} {description}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        if let Some(deprecated) = &self.deprecated {
            rendered.push_str("\n\n### Deprecated\n\n");
            rendered.push_str(if deprecated.is_empty() {
                "Deprecated."
            } else {
                deprecated
            });
        }
        if !self.see.is_empty() {
            rendered.push_str("\n\n### See also\n\n");
            rendered.push_str(&self.see.join("\n"));
        }
        rendered
    }
}

fn typed_value(value: &str) -> (String, &str) {
    let Some(value) = value.strip_prefix('{') else {
        return (String::new(), value);
    };
    let Some((kind, rest)) = value.split_once('}') else {
        return (String::new(), value);
    };
    (kind.to_owned(), rest.trim())
}

#[cfg(test)]
mod tests {
    use super::extract_javascript;

    #[test]
    fn extracts_documented_javascript_without_external_tools() {
        // Given a source tree with a documented export and an undocumented helper
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("example.js"), "/**\n * Find a value.\n * @param {string} name - Lookup name.\n * @returns {string} Stored value.\n * @example find('example');\n */\nexport function find(name) { return name; }\nfunction hidden() {}\n").unwrap();

        // When the native JavaScript extractor scans the tree
        let manifest = extract_javascript(root.path(), "fixture").unwrap();

        // Then it produces the existing versioned manifest contract
        assert_eq!(manifest.symbols.len(), 1);
        let symbol = &manifest.symbols[0];
        assert_eq!(symbol.key, "example.js::find");
        assert_eq!(symbol.signature, "find(name: string): string");
        assert_eq!(
            symbol.documentation,
            "Find a value.\n\n### Parameters\n\n- **name**: string Lookup name.\n\n### Returns\n\nstring Stored value.\n\n### Examples\n\nfind('example');"
        );
        assert_eq!(symbol.line, 7);
    }

    #[test]
    fn rejects_invalid_syntax_without_claiming_a_complete_scan() {
        // Given an invalid documented JavaScript declaration
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("broken.js"),
            "/** Broken. */\nexport function (",
        )
        .unwrap();

        // When extraction attempts to parse the source
        let error = extract_javascript(root.path(), "fixture").unwrap_err();

        // Then no complete manifest is returned
        assert!(error.to_string().contains("source parsing failed"));
    }

    #[test]
    fn refuses_documented_declarations_it_cannot_yet_represent() {
        // Given a documented anonymous export the extractor cannot identify safely
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("anonymous.js"),
            "/** Public object. */\nexport default { key: 1 };\n",
        )
        .unwrap();

        // When extraction sees the unsupported documented declaration
        let error = extract_javascript(root.path(), "fixture").unwrap_err();

        // Then it fails instead of marking existing imported class symbols missing
        assert!(error.to_string().contains("cannot safely represent"));
    }

    #[test]
    fn refuses_unhandled_jsdoc_tags_instead_of_dropping_documentation() {
        // Given a documented function with a tag the native parser does not support
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("tag.js"), "/**\n * Public function.\n * @template T\n */\nexport function value(input) { return input; }\n").unwrap();

        // When the native extractor encounters that tag
        let error = extract_javascript(root.path(), "fixture").unwrap_err();

        // Then it rejects the scan before any manifest can archive prior symbols
        assert!(error.to_string().contains("unsupported JSDoc tag"));
    }

    #[test]
    fn extracts_classes_methods_and_documented_variables() {
        // Given documented forms that share the legacy JSDoc symbol model
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("forms.js"),
            include_str!("../tests/fixtures/source-docs-forms/forms.js"),
        )
        .unwrap();

        // When the native extractor scans the forms
        let manifest = extract_javascript(root.path(), "forms").unwrap();

        // Then it keeps class, member, and variable symbol identities
        let names = manifest
            .symbols
            .iter()
            .map(|symbol| symbol.title.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["Calculator", "Calculator#add", "answer", "twice"]);
        assert_eq!(manifest.symbols[3].signature, "twice: number");
    }
}
