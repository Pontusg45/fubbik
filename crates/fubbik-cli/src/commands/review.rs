use anyhow::Result;
use comfy_table::{Table, presets::UTF8_FULL};
use owo_colors::OwoColorize;

use crate::ReviewCommand;
use crate::client::{Client, Proposal};
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, command: ReviewCommand, mode: OutputMode) -> Result<()> {
    match command {
        ReviewCommand::List {
            status,
            chunk,
            limit,
        } => {
            let mut proposals = if status == "all" {
                let mut all = Vec::new();
                for status in ["pending", "approved", "rejected"] {
                    all.extend(
                        client
                            .list_proposals(Some(status), chunk.as_deref(), limit)
                            .await?,
                    );
                }
                all.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                all.truncate(limit as usize);
                all
            } else {
                client
                    .list_proposals(Some(&status), chunk.as_deref(), limit)
                    .await?
            };
            if mode == OutputMode::Json {
                return output::json(&proposals);
            }
            if mode == OutputMode::Quiet {
                for proposal in proposals.drain(..) {
                    println!("{}", proposal.id);
                }
                return Ok(());
            }
            render_table(&proposals);
        }
        ReviewCommand::Show { id } => {
            let proposal = client.get_proposal(&id).await?;
            if output::id_or_json(mode, &proposal.id, &proposal)? {
                return Ok(());
            }
            println!("{} {}", "Proposal".bold(), proposal.id.dimmed());
            println!("{} {}", "chunk:".dimmed(), proposal.chunk_id);
            println!("{} {}", "status:".dimmed(), proposal.status);
            if let Some(reason) = &proposal.reason {
                println!("{} {reason}", "reason:".dimmed());
            }
            println!("{}", serde_json::to_string_pretty(&proposal.changes)?);
        }
        ReviewCommand::Approve { id, note } => {
            let proposal = client
                .review_proposal(&id, "approve", note.as_deref())
                .await?;
            if !output::id_or_json(mode, &proposal.id, &proposal)? {
                println!("{} {}", "approved".green(), proposal.id.dimmed());
            }
        }
        ReviewCommand::Reject { id, note } => {
            let proposal = client
                .review_proposal(&id, "reject", note.as_deref())
                .await?;
            if !output::id_or_json(mode, &proposal.id, &proposal)? {
                println!("{} {}", "rejected".red(), proposal.id.dimmed());
            }
        }
    }
    Ok(())
}

fn render_table(proposals: &[Proposal]) {
    if proposals.is_empty() {
        println!("No proposals found.");
        return;
    }
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["ID", "Status", "Chunk", "Fields", "Reason"]);
    for proposal in proposals {
        let fields = proposal
            .changes
            .as_object()
            .map(|changes| changes.keys().cloned().collect::<Vec<_>>().join(", "))
            .unwrap_or_default();
        table.add_row(vec![
            proposal.id.clone(),
            proposal.status.clone(),
            proposal
                .chunk_title
                .clone()
                .unwrap_or_else(|| proposal.chunk_id.clone()),
            fields,
            proposal.reason.clone().unwrap_or_default(),
        ]);
    }
    println!("{table}");
}
