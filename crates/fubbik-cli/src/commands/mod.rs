pub mod add;
pub mod get;
pub mod health;
pub mod list;
pub mod search;

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
            c.id.chars().take(8).collect::<String>(),
            c.chunk_type.clone(),
            c.title.chars().take(60).collect::<String>(),
            c.updated_at.chars().take(10).collect::<String>(),
        ]);
    }
    println!("{table}");
}
