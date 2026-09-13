//! Versioned interchange format for documentation extracted from source code.
use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceManifest {
    pub version: u32,
    /// Stable name for this extraction scope; use a different name for subsets.
    pub project: String,
    pub language: SourceLanguage,
    pub extractor: String,
    /// Only complete scans may mark previously imported symbols missing.
    pub complete: bool,
    #[serde(default)]
    pub diagnostics: Vec<String>,
    pub symbols: Vec<SourceSymbol>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SourceLanguage {
    Javascript,
    Typescript,
    Java,
}

impl SourceLanguage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Javascript => "javascript",
            Self::Typescript => "typescript",
            Self::Java => "java",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceSymbol {
    /// Qualified symbol name, including parameter types for Java overloads.
    pub key: String,
    pub title: String,
    pub signature: String,
    /// Markdown containing description, parameters, returns, throws and examples.
    pub documentation: String,
    /// Repository-relative path using forward slashes.
    pub path: String,
    pub line: u32,
    #[serde(default)]
    pub references: Vec<String>,
}

impl SourceManifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("unsupported source manifest version (expected 1)".into());
        }
        if self.project.is_empty()
            || self.project.len() > 100
            || !self
                .project
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
        {
            return Err(
                "project must contain 1–100 letters, digits, dots, underscores or hyphens".into(),
            );
        }
        if self.extractor.is_empty() || self.extractor.len() > 200 || self.symbols.len() > 2000 {
            return Err("invalid extractor or more than 2000 symbols".into());
        }
        if self.diagnostics.len() > 100 || self.diagnostics.iter().any(|d| d.len() > 2000) {
            return Err("too many or oversized diagnostics".into());
        }
        let mut keys = HashSet::new();
        for symbol in &self.symbols {
            if symbol.key.is_empty() || symbol.key.len() > 1000 || !keys.insert(&symbol.key) {
                return Err(format!(
                    "empty, oversized or duplicate symbol key: {}",
                    symbol.key
                ));
            }
            if symbol.title.trim().is_empty()
                || symbol.title.len() > 500
                || symbol.signature.len() > 5000
                || symbol.documentation.len() > 50_000
                || symbol.path.is_empty()
                || symbol.path.len() > 500
                || symbol.line == 0
                || symbol.path.starts_with('/')
                || symbol.path.contains(['\\', ':'])
                || symbol.path.split('/').any(|p| p == ".." || p.is_empty())
                || symbol.references.len() > 100
                || symbol.references.iter().any(|r| r.len() > 1000)
            {
                return Err(format!("invalid source symbol: {}", symbol.key));
            }
        }
        Ok(())
    }

    pub fn source_path(&self) -> String {
        format!("source-docs://{}/{}", self.project, self.language.as_str())
    }
}

impl SourceSymbol {
    pub fn markdown(&self, language: SourceLanguage) -> String {
        // A longer fence preserves signatures even when they contain backticks.
        let fence = "`".repeat(
            self.signature
                .split(|c| c != '`')
                .map(str::len)
                .max()
                .unwrap_or(0)
                .max(2)
                + 1,
        );
        let mut rendered = format!(
            "{fence}{}\n{}\n{fence}\n\n{}\n\nSource: {}:{}\n",
            language.as_str(),
            self.signature,
            self.documentation,
            self.path,
            self.line
        );
        if !self.references.is_empty() {
            rendered.push_str("\nReferences:\n");
            for reference in &self.references {
                rendered.push_str(&format!("- {reference}\n"));
            }
        }
        rendered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> SourceManifest {
        SourceManifest {
            version: 1,
            project: "example".into(),
            language: SourceLanguage::Java,
            extractor: "test".into(),
            complete: true,
            diagnostics: vec![],
            symbols: vec![SourceSymbol {
                key: "example.Service#find(java.lang.String)".into(),
                title: "find".into(),
                signature: "String find(String key)".into(),
                documentation: "Looks up a key.".into(),
                path: "src/Service.java".into(),
                line: 4,
                references: vec![],
            }],
        }
    }

    #[test]
    fn accepts_distinct_overloads_but_rejects_duplicate_identity() {
        let mut m = manifest();
        let mut overload = m.symbols[0].clone();
        overload.key = "example.Service#find(int)".into();
        m.symbols.push(overload);
        assert!(m.validate().is_ok());
        m.symbols.push(m.symbols[0].clone());
        assert!(m.validate().is_err());
    }

    #[test]
    fn rejects_unsafe_paths_and_unknown_versions() {
        let mut m = manifest();
        for path in ["../secret", "/etc/passwd", "C:\\source.java"] {
            m.symbols[0].path = path.into();
            assert!(m.validate().is_err());
        }
        m = manifest();
        m.version = 2;
        assert!(m.validate().is_err());
    }
}
