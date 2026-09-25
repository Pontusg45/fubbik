use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};

pub fn read(
    inline: Option<String>,
    file: Option<&Path>,
    stdin: bool,
    empty_when_missing: bool,
) -> Result<Option<String>> {
    if let Some(value) = inline {
        return Ok(Some(value));
    }
    if let Some(path) = file {
        return std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))
            .map(Some);
    }
    if stdin {
        let mut value = String::new();
        std::io::stdin()
            .read_to_string(&mut value)
            .context("failed to read stdin")?;
        return Ok(Some(value));
    }
    Ok(empty_when_missing.then(String::new))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_content_takes_precedence_over_missing_inputs() {
        // Given inline content with no file or stdin selection
        // When shared command input is read
        let value = read(Some("template".into()), None, false, false).unwrap();
        // Then the inline value is returned unchanged
        assert_eq!(value.as_deref(), Some("template"));
    }

    #[test]
    fn commands_can_request_empty_content_when_no_input_is_given() {
        // Given no content source for a command where an empty body is valid
        // When shared command input is read with its empty fallback enabled
        let value = read(None, None, false, true).unwrap();
        // Then an explicit empty string is returned
        assert_eq!(value.as_deref(), Some(""));
    }
}
