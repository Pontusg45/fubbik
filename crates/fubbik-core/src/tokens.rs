//! Token counting for context budgeting.
//!
//! Ports `packages/api/src/context/utils.ts:19-37`, which calls
//! `encodingForModel("gpt-4o")` from `js-tiktoken`. `gpt-4o` maps to the
//! `o200k_base` encoding.
//!
//! This is not a cosmetic parity choice. `score::budget_chunks` greedily
//! fills a token budget, so the tokenizer decides *which chunks appear in
//! an export*. A `len / 4` approximation would under-count code-heavy
//! content, over-fill budgets, and select a different set of chunks than
//! Node for the same request — a divergence no test could catch without
//! comparing the two backends directly.
//!
//! Node's `Math.ceil(text.length / 4)` fallback is deliberately NOT ported.
//! It exists because `js-tiktoken` loads its BPE data lazily and can fail;
//! `tiktoken-rs` embeds that data at compile time and cannot fail the same
//! way. Porting a branch that can never execute would add untestable code
//! whose only effect, if it somehow fired, would be to silently change what
//! an export contains.
use std::sync::OnceLock;

use tiktoken_rs::CoreBPE;

fn encoder() -> &'static CoreBPE {
    static ENCODER: OnceLock<CoreBPE> = OnceLock::new();
    ENCODER
        .get_or_init(|| tiktoken_rs::o200k_base().expect("o200k_base is embedded at compile time"))
}

pub fn estimate_tokens(text: &str) -> usize {
    encoder().encode_ordinary(text).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pinned against known-good counts so a future tiktoken-rs upgrade that
    /// changes the encoding is caught by a red test rather than by exports
    /// quietly changing which chunks fit in a budget.
    ///
    /// Counts independently verified against OpenAI's own reference
    /// implementation (Python `tiktoken`, `tiktoken.get_encoding("o200k_base")`,
    /// via `encode_ordinary`), not just this crate's output. The seed-string
    /// count was corrected from a guess of 5 to the verified value of 4:
    /// `"# Project Context\n\n"` tokenizes as `['#', ' Project', ' Context',
    /// '\n\n']` — four tokens, not five.
    #[test]
    fn counts_match_o200k_base_for_known_strings() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("hello"), 1);
        assert_eq!(estimate_tokens("hello world"), 2);
        // The budgeter's seed string — its count is load-bearing.
        assert_eq!(estimate_tokens("# Project Context\n\n"), 4);
    }

    /// A token is not a character. If this ever equals the character count,
    /// the real encoder has been replaced by a length-based approximation.
    #[test]
    fn is_not_a_character_count() {
        let text = "The quick brown fox jumps over the lazy dog";
        let tokens = estimate_tokens(text);
        assert!(tokens > 0);
        assert!(
            tokens < text.chars().count(),
            "a real tokenizer produces fewer tokens than characters for English prose; \
             got {tokens} tokens for {} chars",
            text.chars().count()
        );
    }

    #[test]
    fn handles_non_ascii_without_panicking() {
        assert!(estimate_tokens("héllo wörld — ünïcode") > 0);
        assert!(estimate_tokens("日本語のテキスト") > 0);
    }
}
