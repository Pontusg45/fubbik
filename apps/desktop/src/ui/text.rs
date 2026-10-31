//! A minimal single-line text field: printable keys, backspace, paste.
//! No selection, no cursor movement, no IME.

use gpui::{App, FocusHandle, Modifiers};

pub struct TextField {
    pub text: String,
    pub focus: FocusHandle,
}

impl TextField {
    pub fn new(cx: &mut App) -> Self {
        Self {
            text: String::new(),
            focus: cx.focus_handle(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOutcome {
    Edited,
    Submit,
    Cancel,
    Up,
    Down,
    /// Not for the field: let it reach key bindings.
    Ignored,
}

pub fn apply_key(
    text: &mut String,
    key: &str,
    key_char: Option<&str>,
    modifiers: &Modifiers,
) -> KeyOutcome {
    if modifiers.platform || modifiers.control {
        return KeyOutcome::Ignored;
    }
    match key {
        "backspace" => {
            if modifiers.alt {
                let trimmed = text.trim_end().len();
                let cut = text[..trimmed]
                    .rfind(char::is_whitespace)
                    .map_or(0, |i| i + 1);
                text.truncate(cut);
            } else {
                text.pop();
            }
            KeyOutcome::Edited
        }
        "enter" => KeyOutcome::Submit,
        "escape" => KeyOutcome::Cancel,
        "up" => KeyOutcome::Up,
        "down" => KeyOutcome::Down,
        _ => match key_char {
            Some(ch) if !ch.chars().any(char::is_control) => {
                text.push_str(ch);
                KeyOutcome::Edited
            }
            _ => KeyOutcome::Ignored,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> Modifiers {
        Modifiers::default()
    }

    #[test]
    fn types_and_deletes() {
        // Given
        let mut text = String::new();

        // When
        apply_key(&mut text, "a", Some("a"), &none());
        apply_key(&mut text, "b", Some("B"), &none());
        apply_key(&mut text, "backspace", None, &none());

        // Then
        assert_eq!(text, "a");
    }

    #[test]
    fn command_keys_are_not_typed() {
        // Given
        let mut text = String::from("x");
        let cmd = Modifiers {
            platform: true,
            ..Default::default()
        };

        // When
        let outcome = apply_key(&mut text, "k", None, &cmd);

        // Then
        assert_eq!(outcome, KeyOutcome::Ignored);
        assert_eq!(text, "x");
    }

    #[test]
    fn option_backspace_deletes_a_word() {
        // Given
        let mut text = String::from("start api");
        let alt = Modifiers {
            alt: true,
            ..Default::default()
        };

        // When
        apply_key(&mut text, "backspace", None, &alt);

        // Then
        assert_eq!(text, "start ");
    }
}
