use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Human,
    Json,
    Quiet,
}

impl OutputMode {
    pub fn as_env(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Json => "json",
            Self::Quiet => "quiet",
        }
    }
}

pub fn json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn id_or_json<T: Serialize>(mode: OutputMode, id: &str, value: &T) -> Result<bool> {
    match mode {
        OutputMode::Human => Ok(false),
        OutputMode::Json => {
            json(value)?;
            Ok(true)
        }
        OutputMode::Quiet => {
            println!("{id}");
            Ok(true)
        }
    }
}
