use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use review_harness::{Check, checks, rules, run};

#[derive(Parser)]
#[command(
    name = "review-harness",
    about = "Convention checks for AI-written changes"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List registered checks.
    List {
        /// Repository root holding review-rules.toml.
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Rule file (default: <root>/review-rules.toml).
        #[arg(long)]
        rules: Option<PathBuf>,
    },
    /// Write a starter review-rules.toml.
    Init {
        /// Directory to write into.
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Run checks and exit non-zero when any check exceeds its baseline.
    Run {
        /// Repository root to check.
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Run only the named checks.
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,
        /// Rule file (default: <root>/review-rules.toml).
        #[arg(long)]
        rules: Option<PathBuf>,
        /// Also list violations of checks that are within their baseline.
        #[arg(long)]
        verbose: bool,
    },
}

/// Compiled checks plus any rules from the rule file (an explicit path must exist).
fn registry(root: &Path, rules_path: Option<PathBuf>) -> anyhow::Result<Vec<Box<dyn Check>>> {
    let mut registered = checks::all();
    let explicit = rules_path.is_some();
    let path = rules_path.unwrap_or_else(|| root.join("review-rules.toml"));
    if explicit || path.is_file() {
        for rule in rules::load_rules(&path)? {
            registered.push(Box::new(rule));
        }
    }
    Ok(registered)
}

fn main() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init { root } => {
            rules::init(&root)?;
            println!("wrote {}", root.join("review-rules.toml").display());
            Ok(ExitCode::SUCCESS)
        }
        Command::List { root, rules } => {
            let registered = registry(&root, rules)?;
            {
                if registered.is_empty() {
                    println!("No checks registered yet.");
                }
                for check in &registered {
                    println!("  {:<32} {}", check.name(), check.description());
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Run {
            root,
            only,
            rules,
            verbose,
        } => {
            let mut registered = registry(&root, rules)?;
            if let Some(unknown) = only
                .iter()
                .find(|n| !registered.iter().any(|c| c.name() == n.as_str()))
            {
                anyhow::bail!("unknown check: {unknown}");
            }
            if !only.is_empty() {
                registered.retain(|c| only.iter().any(|n| n == c.name()));
            }
            let report = run(&root, &registered)?;
            for result in &report.results {
                if verbose || result.over_baseline() || result.baseline == 0 {
                    for v in &result.violations {
                        println!("{}:{}  [{}] {}", v.file, v.line, result.name, v.message);
                    }
                }
                println!(
                    "[{}] {} violation(s) (baseline {}){}",
                    result.name,
                    result.violations.len(),
                    result.baseline,
                    if result.over_baseline() { " FAIL" } else { "" }
                );
            }
            println!(
                "{} check(s), {} violation(s)",
                report.results.len(),
                report.violation_count()
            );
            Ok(if report.passed() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
    }
}
