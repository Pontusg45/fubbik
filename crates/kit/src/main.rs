use std::io::{IsTerminal, Write};
use std::process::ExitCode;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use kit::git::Git;
use kit::sync::SyncOutcome;
use kit::{branch, port, push, sync};

#[derive(Parser)]
#[command(name = "fk", about = "Everyday commands, safe by default")]
struct Cli {
    /// Print machine-readable JSON where applicable
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Fetch and fast-forward the current branch (never rebases)
    Sync {
        #[arg(long)]
        dry_run: bool,
    },
    /// Show unpushed commits and push after confirmation
    Push {
        /// Skip the confirmation prompt
        #[arg(long)]
        yes: bool,
    },
    /// Create and switch to a branch from a clean tree
    Branch {
        name: String,
        #[arg(long)]
        from: Option<String>,
    },
    /// Manage processes listening on TCP ports
    Port {
        #[command(subcommand)]
        command: PortCommand,
    },
}

#[derive(Subcommand)]
enum PortCommand {
    /// Kill whatever is using each port (a free port is not an error)
    Kill {
        #[arg(required = true)]
        ports: Vec<u16>,
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Sync { dry_run } => {
            let git = open_git()?;
            let outcome = sync::sync(&git, dry_run)?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&outcome)?);
            } else {
                match outcome {
                    SyncOutcome::UpToDate => println!("Already up to date."),
                    SyncOutcome::FastForwarded { commits } => {
                        println!("Fast-forwarded {commits} commit(s).")
                    }
                    SyncOutcome::WouldFastForward { commits } => {
                        println!("Would fast-forward {commits} commit(s).")
                    }
                }
            }
        }
        Command::Push { yes } => {
            let git = open_git()?;
            let plan = push::plan(&git)?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                let target = plan.upstream.as_deref().unwrap_or("origin (new branch)");
                println!(
                    "Push {} commit(s) from {} to {target}:",
                    plan.commits.len(),
                    plan.branch
                );
                for c in &plan.commits {
                    println!("  {} {}", &c.sha[..c.sha.len().min(7)], c.subject);
                }
            }
            if !yes {
                if !std::io::stdin().is_terminal() {
                    bail!("refusing to push without a terminal; pass --yes");
                }
                eprint!("Push? [y/N] ");
                std::io::stderr().flush()?;
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer)?;
                if !matches!(answer.trim(), "y" | "Y" | "yes") {
                    bail!("aborted");
                }
            }
            push::execute(&git, &plan)?;
        }
        Command::Branch { name, from } => {
            let git = open_git()?;
            branch::create(&git, &name, from.as_deref())?;
            if !cli.json {
                println!("Switched to new branch {name}");
            }
        }
        Command::Port {
            command: PortCommand::Kill { ports, dry_run },
        } => {
            let mut failed = 0;
            for p in ports {
                match port::kill(p, dry_run) {
                    Ok(outcome) if cli.json => {
                        println!("{}", serde_json::to_string_pretty(&outcome)?)
                    }
                    Ok(outcome) => println!("{}", port::describe(p, &outcome)),
                    Err(err) => {
                        eprintln!("error: port {p}: {err}");
                        failed += 1;
                    }
                }
            }
            if failed > 0 {
                bail!("{failed} port(s) failed");
            }
        }
    }
    Ok(())
}

fn open_git() -> Result<Git> {
    Git::open(std::env::current_dir()?)
}
