pub mod add;
pub mod config;
pub mod connection;
pub mod context;
pub mod delete;
pub mod docs;
pub mod enrich;
pub mod generate;
pub mod get;
pub mod health;
pub mod input;
pub mod list;
pub mod open;
pub mod plan;
pub mod plugin;
pub mod requirement;
pub mod review;
pub mod search;
pub mod source_docs;
pub mod space;
pub mod stale;
pub mod stats;
pub mod status;
pub mod tag;
pub mod task;
pub mod update;
pub mod updates;

use crate::client::Chunk;
use comfy_table::{Table, presets::UTF8_FULL};

/// Shared table renderer so every listing command formats identically.
pub fn render_table(chunks: &[Chunk]) {
    if chunks.is_empty() {
        println!("No chunks found.");
        return;
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["ID", "Type", "Title", "Updated"]);
    for c in chunks {
        table.add_row(vec![
            // Full id, not truncated: `get` needs the whole 24-character
            // id, and a shortened id copied from this table 404s.
            c.id.clone(),
            c.chunk_type.clone(),
            c.title.chars().take(60).collect::<String>(),
            c.updated_at.chars().take(10).collect::<String>(),
        ]);
    }
    println!("{table}");
}
