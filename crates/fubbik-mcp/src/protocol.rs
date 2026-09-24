use anyhow::Result;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::{Server, error_response};

pub async fn run(base_url: &str) -> Result<()> {
    let server = Server::new(base_url);
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(error) => {
                write_message(
                    &mut stdout,
                    &error_response(Value::Null, -32700, format!("parse error: {error}")),
                )
                .await?;
                continue;
            }
        };

        if let Some(response) = server.handle(request).await {
            write_message(&mut stdout, &response).await?;
        }
    }
    Ok(())
}

async fn write_message(writer: &mut tokio::io::Stdout, message: &Value) -> Result<()> {
    writer
        .write_all(serde_json::to_string(message)?.as_bytes())
        .await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}
