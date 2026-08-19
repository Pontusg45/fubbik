//! Direct port of `packages/api/src/vocabulary/parser.ts`'s `parseStepText`
//! — matches a BDD step's free text against a space's controlled
//! vocabulary and flags warnings.
//!
//! **Position semantics**: positions are Unicode scalar (`char`) offsets
//! into the input, not UTF-16 code units the way Node's
//! `string.length`/`.substring()` indices are. For ASCII and most BMP text
//! (the expected shape of a BDD step: `"click the button"`) these agree
//! exactly; they only diverge for astral-plane characters (emoji, rare
//! CJK extensions), the same accepted divergence `search::parser::tokenize`
//! already carries in this crate. Case-folding uses
//! `char::to_ascii_lowercase`, not full Unicode case folding — JS's
//! `.toLowerCase()` is closer to full Unicode folding, but the algorithm's
//! index alignment (`lowered[i]` must correspond 1:1 to `chars[i]`) breaks
//! under multi-char case expansions (e.g. `'İ'.to_lowercase()` yields two
//! chars) in *both* languages; ASCII-only folding keeps the common case
//! (English vocabulary words) exact and never panics on the rare case.
//!
//! Ported test cases live in `parser.test.ts`
//! (`packages/api/src/vocabulary/parser.test.ts`) and are reproduced
//! verbatim in this crate's `tests/vocabulary_parser.rs`.

#[derive(Debug, Clone)]
pub struct VocabEntry {
    pub word: String,
    pub category: String,
    pub expects: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
// Distinct OpenAPI name — see the note on `saved_graph::Position`.
#[schema(as = TextSpan)]
pub struct Position {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ParsedToken {
    pub text: String,
    pub category: Option<String>,
    pub position: Position,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WarningType {
    UnknownWord,
    UnexpectedCategory,
    ExpectsNotSatisfied,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct VocabularyWarning {
    pub position: Position,
    #[serde(rename = "type")]
    pub warning_type: WarningType,
    pub word: String,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ParseResult {
    pub tokens: Vec<ParsedToken>,
    pub warnings: Vec<VocabularyWarning>,
}

struct ExtractedLiteral {
    text: String,
    start: usize,
    end: usize,
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

struct LastNonModifier<'a> {
    entry: &'a VocabEntry,
    token: ParsedToken,
}

/// Parse step text against a controlled vocabulary.
///
/// Algorithm (matches `parser.ts` step-by-step):
/// 1. Extract quoted strings and numbers, mask their positions.
/// 2. Lowercase remaining text (ASCII-only, see module doc).
/// 3. Sort vocabulary by word length descending (greedy longest match).
/// 4. Scan left-to-right matching the longest vocab entry at each position.
/// 5. Validate slot expectations between tokens.
/// 6. Check dangling expects at end of sequence.
pub fn parse_step_text(text: &str, vocabulary: &[VocabEntry]) -> ParseResult {
    if text.trim().is_empty() {
        return ParseResult {
            tokens: Vec::new(),
            warnings: Vec::new(),
        };
    }

    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    let mut tokens: Vec<ParsedToken> = Vec::new();
    let mut warnings: Vec<VocabularyWarning> = Vec::new();

    // Step 1a: extract quoted strings (double and single quotes).
    let mut literals: Vec<ExtractedLiteral> = Vec::new();
    {
        let mut i = 0;
        while i < len {
            let c = chars[i];
            if c == '"' || c == '\'' {
                let mut j = i + 1;
                while j < len && chars[j] != c {
                    j += 1;
                }
                if j < len {
                    let end = j + 1;
                    literals.push(ExtractedLiteral {
                        text: chars[i..end].iter().collect(),
                        start: i,
                        end,
                    });
                    i = end;
                    continue;
                }
            }
            i += 1;
        }
    }

    // Step 1b: extract standalone numbers (`\b\d+(?:\.\d+)?\b`) not inside
    // an already-extracted quoted literal.
    {
        let mut i = 0;
        while i < len {
            if chars[i].is_ascii_digit() && (i == 0 || !is_word_char(chars[i - 1])) {
                let mut j = i;
                while j < len && chars[j].is_ascii_digit() {
                    j += 1;
                }
                let mut end = j;
                if j < len && chars[j] == '.' && j + 1 < len && chars[j + 1].is_ascii_digit() {
                    let mut k = j + 1;
                    while k < len && chars[k].is_ascii_digit() {
                        k += 1;
                    }
                    end = k;
                }
                if end >= len || !is_word_char(chars[end]) {
                    let in_quote = literals.iter().any(|l| i >= l.start && i < l.end);
                    if !in_quote {
                        literals.push(ExtractedLiteral {
                            text: chars[i..end].iter().collect(),
                            start: i,
                            end,
                        });
                    }
                    i = end;
                    continue;
                }
            }
            i += 1;
        }
    }

    literals.sort_by_key(|l| l.start);

    // Step 2: lowered copy with literal ranges masked to spaces, so vocab
    // matching in step 4 can never straddle into a literal's characters.
    let mut lowered: Vec<char> = chars.iter().map(|c| c.to_ascii_lowercase()).collect();
    for lit in &literals {
        for c in &mut lowered[lit.start..lit.end] {
            *c = ' ';
        }
    }

    let mut consumed = vec![false; len];
    for lit in &literals {
        tokens.push(ParsedToken {
            text: lit.text.clone(),
            category: Some("literal".to_string()),
            position: Position {
                start: lit.start,
                end: lit.end,
            },
        });
        for c in &mut consumed[lit.start..lit.end] {
            *c = true;
        }
    }

    // Step 3: vocabulary sorted longest-word-first (char count).
    let mut sorted_vocab: Vec<&VocabEntry> = vocabulary.iter().collect();
    sorted_vocab.sort_by_key(|v| std::cmp::Reverse(v.word.chars().count()));
    let lowered_vocab_words: Vec<Vec<char>> = sorted_vocab
        .iter()
        .map(|v| v.word.chars().map(|c| c.to_ascii_lowercase()).collect())
        .collect();

    // Step 4: left-to-right scan.
    let mut pos = 0usize;
    while pos < len {
        if consumed[pos] {
            pos += 1;
            continue;
        }
        if lowered[pos].is_whitespace() {
            pos += 1;
            continue;
        }

        let mut matched = false;
        for (vi, vocab_word) in lowered_vocab_words.iter().enumerate() {
            let wlen = vocab_word.len();
            let end = pos + wlen;
            if end > len {
                continue;
            }
            if lowered[pos..end] != vocab_word[..] {
                continue;
            }
            if end < len && !lowered[end].is_whitespace() && !consumed[end] {
                continue;
            }
            if pos > 0 && !lowered[pos - 1].is_whitespace() && !consumed[pos - 1] {
                continue;
            }

            let entry = sorted_vocab[vi];
            tokens.push(ParsedToken {
                text: chars[pos..end].iter().collect(),
                category: Some(entry.category.clone()),
                position: Position { start: pos, end },
            });
            for c in &mut consumed[pos..end] {
                *c = true;
            }
            pos = end;
            matched = true;
            break;
        }

        if !matched {
            let mut word_end = pos;
            while word_end < len && !lowered[word_end].is_whitespace() && !consumed[word_end] {
                word_end += 1;
            }
            let unknown_word: String = chars[pos..word_end].iter().collect();
            tokens.push(ParsedToken {
                text: unknown_word.clone(),
                category: None,
                position: Position {
                    start: pos,
                    end: word_end,
                },
            });
            warnings.push(VocabularyWarning {
                position: Position {
                    start: pos,
                    end: word_end,
                },
                warning_type: WarningType::UnknownWord,
                word: unknown_word.clone(),
                message: format!("Unknown word: \"{unknown_word}\""),
            });
            for c in &mut consumed[pos..word_end] {
                *c = true;
            }
            pos = word_end;
        }
    }

    tokens.sort_by_key(|t| t.position.start);

    // Step 5: validate slot expectations. Modifiers are transparent.
    let mut last_non_modifier: Option<LastNonModifier> = None;

    for token in &tokens {
        if token.category.as_deref() == Some("literal") {
            if last_non_modifier
                .as_ref()
                .is_some_and(|l| l.entry.expects.is_some())
            {
                last_non_modifier = None;
            }
            continue;
        }

        let Some(vocab_entry) = vocabulary
            .iter()
            .find(|v| v.word.to_lowercase() == token.text.to_lowercase())
        else {
            continue;
        };

        if vocab_entry.category == "modifier" {
            continue;
        }

        if let Some(last) = &last_non_modifier
            && let Some(expects) = &last.entry.expects
            && !expects.iter().any(|e| e == &vocab_entry.category)
        {
            warnings.push(VocabularyWarning {
                position: token.position.clone(),
                warning_type: WarningType::UnexpectedCategory,
                word: token.text.clone(),
                message: format!(
                    "Expected {} after \"{}\", got {} \"{}\"",
                    expects.join(" or "),
                    last.token.text,
                    vocab_entry.category,
                    token.text
                ),
            });
        }

        last_non_modifier = Some(LastNonModifier {
            entry: vocab_entry,
            token: token.clone(),
        });
    }

    // Step 6: end-of-sequence check.
    if let Some(last) = &last_non_modifier
        && let Some(expects) = &last.entry.expects
    {
        warnings.push(VocabularyWarning {
            position: last.token.position.clone(),
            warning_type: WarningType::ExpectsNotSatisfied,
            word: last.token.text.clone(),
            message: format!(
                "\"{}\" expects {} to follow, but step ends",
                last.token.text,
                expects.join(" or ")
            ),
        });
    }

    ParseResult { tokens, warnings }
}
