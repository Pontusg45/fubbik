//! Direct port of `packages/api/src/vocabulary/parser.test.ts`'s test
//! cases onto `fubbik_api::vocabulary::parser::parse_step_text`, proving
//! parity with Node's algorithm.

use fubbik_api::vocabulary::parser::{VocabEntry, WarningType, parse_step_text};

fn vocab() -> Vec<VocabEntry> {
    vec![
        VocabEntry {
            word: "click".into(),
            category: "action".into(),
            expects: Some(vec!["target".into()]),
        },
        VocabEntry {
            word: "type".into(),
            category: "action".into(),
            expects: Some(vec!["target".into()]),
        },
        VocabEntry {
            word: "logged in".into(),
            category: "state".into(),
            expects: None,
        },
        VocabEntry {
            word: "log".into(),
            category: "action".into(),
            expects: Some(vec!["target".into()]),
        },
        VocabEntry {
            word: "button".into(),
            category: "target".into(),
            expects: None,
        },
        VocabEntry {
            word: "field".into(),
            category: "target".into(),
            expects: None,
        },
        VocabEntry {
            word: "submit".into(),
            category: "target".into(),
            expects: None,
        },
        VocabEntry {
            word: "the".into(),
            category: "modifier".into(),
            expects: None,
        },
        VocabEntry {
            word: "is".into(),
            category: "connector".into(),
            expects: Some(vec!["state".into()]),
        },
    ]
}

#[test]
fn parses_a_valid_step_with_no_warnings() {
    // Given the inline inputs and test fixtures.
    // When
    let result = parse_step_text("click the button", &vocab());
    // Then
    assert!(result.warnings.is_empty());
    assert_eq!(result.tokens.len(), 3);
    assert_eq!(result.tokens[0].text, "click");
    assert_eq!(result.tokens[0].category.as_deref(), Some("action"));
    assert_eq!(result.tokens[1].text, "the");
    assert_eq!(result.tokens[1].category.as_deref(), Some("modifier"));
    assert_eq!(result.tokens[2].text, "button");
    assert_eq!(result.tokens[2].category.as_deref(), Some("target"));
}

#[test]
fn matches_multi_word_entries_greedily() {
    // Given the inline inputs and test fixtures.
    // When
    let result = parse_step_text("user is logged in", &vocab());
    let logged_in = result
        .tokens
        .iter()
        .find(|t| t.text.to_lowercase() == "logged in")
        .expect("\"logged in\" should be matched as one token");
    // Then
    assert_eq!(logged_in.category.as_deref(), Some("state"));

    assert!(
        !result.tokens.iter().any(|t| t.text.to_lowercase() == "log"),
        "\"log\" must not appear as a separate token"
    );
}

#[test]
fn produces_warnings_for_unknown_words() {
    // Given the inline inputs and test fixtures.
    // When
    let result = parse_step_text("click the foobar button", &vocab());
    let unknown: Vec<_> = result
        .warnings
        .iter()
        .filter(|w| w.warning_type == WarningType::UnknownWord)
        .collect();
    // Then
    assert_eq!(unknown.len(), 1);
    assert_eq!(unknown[0].word, "foobar");
}

#[test]
fn produces_warning_for_unexpected_category_after_action() {
    // Given the inline inputs and test fixtures.
    // When
    // "click" expects "target", but is followed by another action "type".
    let result = parse_step_text("click type", &vocab());
    let warning = result
        .warnings
        .iter()
        .find(|w| w.warning_type == WarningType::UnexpectedCategory)
        .expect("expected an unexpected_category warning");
    // Then
    assert_eq!(warning.word, "type");
}

#[test]
fn produces_warning_for_dangling_expects_at_end_of_step() {
    // Given the inline inputs and test fixtures.
    // When
    // "click" expects "target" but the step ends.
    let result = parse_step_text("click", &vocab());
    let warning = result
        .warnings
        .iter()
        .find(|w| w.warning_type == WarningType::ExpectsNotSatisfied)
        .expect("expected an expects_not_satisfied warning");
    // Then
    assert_eq!(warning.word, "click");
}

#[test]
fn does_not_produce_unknown_warnings_for_quoted_literals() {
    // Given the inline inputs and test fixtures.
    // When
    let result = parse_step_text(r#"type "hello world" field"#, &vocab());
    // Then
    assert!(result.warnings.is_empty());
    let literal = result
        .tokens
        .iter()
        .find(|t| t.text == "\"hello world\"")
        .expect("expected the quoted literal as a token");
    assert_eq!(literal.category.as_deref(), Some("literal"));
}

#[test]
fn matches_case_insensitively() {
    // Given the inline inputs and test fixtures.
    // When
    let result = parse_step_text("Click THE Button", &vocab());
    // Then
    assert!(result.warnings.is_empty());
    assert_eq!(result.tokens[0].text, "Click");
    assert_eq!(result.tokens[0].category.as_deref(), Some("action"));
    assert_eq!(result.tokens[2].text, "Button");
    assert_eq!(result.tokens[2].category.as_deref(), Some("target"));
}

#[test]
fn returns_empty_result_for_empty_text() {
    // Given the inline inputs and test fixtures.
    // When
    let result = parse_step_text("", &vocab());
    // Then
    assert!(result.tokens.is_empty());
    assert!(result.warnings.is_empty());
}
