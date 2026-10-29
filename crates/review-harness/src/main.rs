use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use review_harness::{checks, run};

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
    List,
    /// Run checks and exit non-zero on any violation.
    Run {
        /// Repository root to check.
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Run only the named checks.
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,
    },
}

fn main() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();
    let mut registered = checks::all();
    match cli.command {
        Command::List => {
            if registered.is_empty() {
                println!("No checks registered yet.");
            }
            for check in &registered {
                println!("  {:<32} {}", check.name(), check.description());
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Run { root, only } => {
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
                for v in &result.violations {
                    println!("{}:{}  [{}] {}", v.file, v.line, result.name, v.message);
                }
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
