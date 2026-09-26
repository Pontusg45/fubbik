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

pub struct CommandOutput<T> {
    pub data: T,
    pub human: String,
    pub quiet: Vec<String>,
}

pub fn format<T: Serialize>(mode: OutputMode, output: &CommandOutput<T>) -> Result<String> {
    Ok(match mode {
        OutputMode::Json => format!("{}\n", serde_json::to_string_pretty(&output.data)?),
        OutputMode::Human if output.human.is_empty() => String::new(),
        OutputMode::Human => format!("{}\n", output.human),
        OutputMode::Quiet if output.quiet.is_empty() => String::new(),
        OutputMode::Quiet => format!("{}\n", output.quiet.join("\n")),
    })
}

pub fn render<T: Serialize>(mode: OutputMode, output: CommandOutput<T>) -> Result<()> {
    print!("{}", format(mode, &output)?);
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
#[cfg(test)]
mod tests {
    use super::{CommandOutput, OutputMode, format};

    #[test]
    fn renders_all_modes_from_one_result() {
        // Given
        let result = CommandOutput {
            data: serde_json::json!({"id": "chunk-1"}),
            human: "Created chunk-1".into(),
            quiet: vec!["chunk-1".into()],
        };

        // When
        let json = format(OutputMode::Json, &result).unwrap();
        let human = format(OutputMode::Human, &result).unwrap();
        let quiet = format(OutputMode::Quiet, &result).unwrap();

        // Then
        assert_eq!(json, "{\n  \"id\": \"chunk-1\"\n}\n");
        assert_eq!(human, "Created chunk-1\n");
        assert_eq!(quiet, "chunk-1\n");
    }

    #[test]
    fn an_empty_quiet_result_emits_no_lines() {
        // Given
        let result = CommandOutput {
            data: Vec::<String>::new(),
            human: "No items found.".into(),
            quiet: Vec::new(),
        };

        // When
        let quiet = format(OutputMode::Quiet, &result).unwrap();

        // Then
        assert!(quiet.is_empty());
    }
}
