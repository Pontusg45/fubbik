//! Native Java source-comment extraction for declarations whose symbol
//! identity can be reconstructed without a compiler or a JVM.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use fubbik_core::source_docs::{SourceLanguage, SourceManifest, SourceSymbol};
use tree_sitter::{Node, Parser};

pub fn extract(root: &Path, project: &str) -> Result<SourceManifest> {
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
    parser.set_language(&tree_sitter_java::LANGUAGE.into())?;
    let mut symbols = Vec::new();
    for file in files {
        let source = std::fs::read_to_string(&file)?;
        let tree = parser
            .parse(&source, None)
            .context("could not parse Java source")?;
        if tree.root_node().has_error() {
            bail!("source parsing failed: {}", file.display());
        }
        let path = file
            .strip_prefix(&root)?
            .to_string_lossy()
            .replace('\\', "/");
        extract_file(tree.root_node(), &source, &path, &mut symbols)?;
    }
    symbols.sort_by(|left, right| left.key.cmp(&right.key));
    let manifest = SourceManifest {
        version: 1,
        project: project.to_owned(),
        language: SourceLanguage::Java,
        extractor: format!("fubbik tree-sitter-java {}", env!("CARGO_PKG_VERSION")),
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
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        let path = entry.path();
        if kind.is_dir() {
            if !matches!(
                entry.file_name().to_string_lossy().as_ref(),
                ".git" | "node_modules" | "target" | "build" | "dist" | ".next"
            ) {
                collect_files(&path, files)?;
            }
        } else if kind.is_file() && path.extension().is_some_and(|value| value == "java") {
            files.push(path);
        }
    }
    Ok(())
}

fn extract_file(
    root: Node<'_>,
    source: &str,
    path: &str,
    symbols: &mut Vec<SourceSymbol>,
) -> Result<()> {
    let mut cursor = root.walk();
    let mut package = String::new();
    let mut preceding_doc = None;
    let mut consumed = HashSet::new();
    for node in root.named_children(&mut cursor) {
        match node.kind() {
            "package_declaration" => {
                package = text(node, source)
                    .trim_start_matches("package")
                    .trim()
                    .trim_end_matches(';')
                    .to_owned();
            }
            "block_comment" => {
                let comment = text(node, source);
                preceding_doc = comment.starts_with("/**").then_some((node, comment));
            }
            "class_declaration" => {
                let class_name = node
                    .child_by_field_name("name")
                    .context("Java class has no name")?;
                let class_name = text(class_name, source);
                let qualified = if package.is_empty() {
                    class_name.to_owned()
                } else {
                    format!("{package}.{class_name}")
                };
                let visible = is_visible(node, source);
                if let Some((comment_node, comment)) = preceding_doc.take() {
                    consumed.insert(comment_node.start_byte());
                    if visible {
                        symbols.push(SourceSymbol {
                            key: qualified.clone(),
                            title: qualified.clone(),
                            signature: qualified.clone(),
                            documentation: java_doc(comment)?,
                            path: path.to_owned(),
                            line: node.start_position().row as u32 + 1,
                            references: Vec::new(),
                        });
                    }
                }
                if visible {
                    extract_methods(node, source, path, &qualified, symbols, &mut consumed)?;
                }
            }
            _ => {
                preceding_doc = None;
            }
        }
    }
    reject_unhandled_docs(root, source, path, &consumed)?;
    Ok(())
}

fn extract_methods(
    class: Node<'_>,
    source: &str,
    path: &str,
    qualified: &str,
    symbols: &mut Vec<SourceSymbol>,
    consumed: &mut HashSet<usize>,
) -> Result<()> {
    let body = class
        .child_by_field_name("body")
        .context("Java class has no body")?;
    let mut cursor = body.walk();
    let mut preceding_doc = None;
    for node in body.named_children(&mut cursor) {
        if node.kind() == "block_comment" {
            let comment = text(node, source);
            preceding_doc = comment.starts_with("/**").then_some((node, comment));
            continue;
        }
        if let Some((comment_node, comment)) = preceding_doc.take() {
            if node.kind() != "method_declaration" {
                continue;
            }
            consumed.insert(comment_node.start_byte());
            if !is_visible(node, source) {
                continue;
            }
            let name = node
                .child_by_field_name("name")
                .context("Java method has no name")?;
            let name = text(name, source);
            let return_type = node
                .child_by_field_name("type")
                .context("Java method has no return type")?;
            let return_type = resolve_type(text(return_type, source), path)?;
            let parameters = node
                .child_by_field_name("parameters")
                .context("Java method has no parameters")?;
            let mut param_cursor = parameters.walk();
            let mut types = Vec::new();
            for parameter in parameters.named_children(&mut param_cursor) {
                if parameter.kind() != "formal_parameter" {
                    bail!("unsupported Java parameter in {path}");
                }
                let kind = parameter
                    .child_by_field_name("type")
                    .context("Java parameter has no type")?;
                types.push(resolve_type(text(kind, source), path)?);
            }
            let types = types.join(",");
            let key = format!("{qualified}#{name}({types})");
            symbols.push(SourceSymbol {
                key: key.clone(),
                title: key,
                signature: format!("{return_type} {name}({types})"),
                documentation: java_doc(comment)?,
                path: path.to_owned(),
                line: node.start_position().row as u32 + 1,
                references: Vec::new(),
            });
        }
    }
    Ok(())
}

fn is_visible(node: Node<'_>, source: &str) -> bool {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "modifiers")
        .is_some_and(|modifiers| {
            let value = text(modifiers, source);
            value
                .split_whitespace()
                .any(|part| part == "public" || part == "protected")
        })
}

fn resolve_type(kind: &str, path: &str) -> Result<String> {
    let kind = kind.trim();
    if matches!(
        kind,
        "boolean" | "byte" | "short" | "int" | "long" | "float" | "double" | "char" | "void"
    ) {
        return Ok(kind.to_owned());
    }
    if matches!(
        kind,
        "String" | "Object" | "Integer" | "Long" | "Boolean" | "Exception" | "RuntimeException"
    ) {
        return Ok(format!("java.lang.{kind}"));
    }
    if kind.starts_with("java.") {
        return Ok(kind.to_owned());
    }
    bail!("cannot resolve Java type {kind} in {path} without a compiler")
}

fn java_doc(comment: &str) -> Result<String> {
    let body = comment.trim_start_matches("/**").trim_end_matches("*/");
    let mut summary = String::new();
    let mut tags = Vec::new();
    for raw in body.lines() {
        let line = raw.trim().trim_start_matches('*').trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('@') {
            if !["@param ", "@return ", "@throws ", "@see ", "@deprecated"]
                .iter()
                .any(|tag| line.starts_with(tag))
            {
                bail!("unsupported Javadoc tag: {line}");
            }
            tags.push(line.to_owned());
        } else if let Some(tag) = tags.last_mut() {
            let tag: &mut String = tag;
            tag.push(' ');
            tag.push_str(line);
        } else {
            if !summary.is_empty() {
                summary.push(' ');
            }
            summary.push_str(line);
        }
    }
    if comment.contains("{@") {
        bail!("unsupported inline Javadoc tag");
    }
    for tag in tags {
        summary.push_str("\n\n");
        summary.push_str(&tag);
    }
    Ok(summary)
}

fn reject_unhandled_docs(
    node: Node<'_>,
    source: &str,
    path: &str,
    consumed: &HashSet<usize>,
) -> Result<()> {
    if node.kind() == "block_comment"
        && text(node, source).starts_with("/**")
        && !consumed.contains(&node.start_byte())
    {
        bail!(
            "native Java extraction cannot safely represent a documented declaration at {path}:{}",
            node.start_position().row + 1
        );
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        reject_unhandled_docs(child, source, path, consumed)?;
    }
    Ok(())
}

fn text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::extract;

    #[test]
    fn extracts_documented_public_java_overloads_without_jvm() {
        // Given the checked-in Java fixture with two documented overloads
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/source-docs");

        // When the native Java extractor scans the source
        let manifest = extract(&root, "fixture").unwrap();

        // Then overload keys, signatures, and source locations are stable
        assert_eq!(manifest.symbols.len(), 3);
        assert_eq!(manifest.symbols[0].key, "sample.Lookup");
        assert_eq!(manifest.symbols[1].key, "sample.Lookup#find(int)");
        assert_eq!(
            manifest.symbols[2].key,
            "sample.Lookup#find(java.lang.String)"
        );
        assert_eq!(
            manifest.symbols[2].signature,
            "java.lang.String find(java.lang.String)"
        );
    }

    #[test]
    fn refuses_a_documented_public_field_before_import() {
        // Given a public field that this native subset cannot preserve yet
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("Store.java"),
            "public class Store {\n/** Public value. */\npublic int value;\n}\n",
        )
        .unwrap();

        // When the extractor scans the documented field
        let error = extract(root.path(), "fixture").unwrap_err();

        // Then it cannot mark the partial class scan complete
        assert!(error.to_string().contains("cannot safely represent"));
    }
}
