//! Review harness: project convention checks that AI-written changes must pass before they land.
//!
//! A [`Check`] declares which files it looks at and reports [`Violation`]s for one file at a time.
//! The [`runner`] walks the tree once per check, applies the shared ignore list, and collects a [`Report`].

pub mod checks;
pub mod rules;
pub mod runner;
pub mod source;

pub use runner::{Report, run};
pub use source::TextFile;

/// One rule broken at one place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Repo-relative path, `/`-separated.
    pub file: String,
    /// 1-based line number.
    pub line: usize,
    pub message: String,
}

/// A convention check over raw file contents.
pub trait Check: Send + Sync {
    /// kebab-case and unique; used by `--only` and in report output.
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    /// Repo-relative directories to walk, e.g. `["apps/web/src"]`.
    fn roots(&self) -> Vec<&str>;
    /// File extensions to include, with the dot: `[".ts", ".tsx"]`.
    fn extensions(&self) -> Vec<&str>;
    /// Report every violation in one file.
    fn check(&self, file: &TextFile) -> Vec<Violation>;
}
