//! Direct port of `packages/api/src/vocabulary/suggest.ts`'s
//! `suggestVocabulary` — asks a local Ollama `llama3.2` model to extract
//! vocabulary entries from a space's chunks.
//!
//! `suggest_vocabulary` is **infallible**, matching Node's
//! `Effect.Effect<SuggestedEntry[], never>` signature: every failure mode
//! (Ollama unreachable, non-2xx response, unparseable JSON, no `[...]`
//! found in the model's output) degrades to an empty `Vec`, never an
//! error — Node's own `.pipe(Effect.catchAll(() => Effect.succeed([])))`
//! (`suggest.ts:116`). This is a fourth Ollama-failure policy alongside
//! the three `crate::search::service`'s module doc describes (probe-first
//! for enrichment, 502-on-failure for semantic search, `orElse([])` for
//! `similar-to`): here there is no probe and no error path at all — every
//! failure mode collapses to `[]`, unconditionally. The transport lives in
//! `fubbik_ai::OllamaClient`, carried on `AppState`.

const VALID_CATEGORIES: [&str; 6] = ["actor", "action", "target", "outcome", "state", "modifier"];

fn is_valid_category(c: &str) -> bool {
    VALID_CATEGORIES.contains(&c)
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedEntry {
    pub word: String,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expects: Option<Vec<String>>,
}

const PROMPT_TEMPLATE: &str = "You are analyzing code documentation chunks to extract a controlled vocabulary for behavior-driven requirements (BDD/Gherkin style).\n\nGiven the following chunks of documentation/code, extract meaningful vocabulary entries. Each entry has:\n- \"word\": a short word or phrase (1-3 words, lowercase)\n- \"category\": one of \"actor\", \"action\", \"target\", \"outcome\", \"state\", \"modifier\"\n- \"expects\" (optional): array of category names that should follow this word\n\nCategories:\n- actor: who performs the action (e.g., \"user\", \"admin\", \"system\")\n- action: what is done (e.g., \"click\", \"submit\", \"navigate\")\n- target: what the action is performed on (e.g., \"button\", \"form\", \"page\")\n- outcome: what should result (e.g., \"displayed\", \"saved\", \"redirected\")\n- state: a condition (e.g., \"logged in\", \"visible\", \"enabled\")\n- modifier: connecting/clarifying words (e.g., \"the\", \"a\", \"should\")\n\nActions typically expect [\"target\"]. Actors typically expect [\"action\"].\n\nReturn a JSON array of objects. Only return the JSON array, no other text.\n\nCHUNKS:\n";

/// Infallible, matching Node's `Effect.Effect<SuggestedEntry[], never>`:
/// every failure mode degrades to an empty `Vec`.
///
/// The `ollama_url` override parameter this function used to take is gone —
/// the client now arrives from `AppState`, which supersedes it and gives
/// tests a better injection point (a real mock server rather than a URL
/// string threaded through the service layer).
pub async fn suggest_vocabulary(
    client: &fubbik_ai::OllamaClient,
    chunks: &[(String, String)],
) -> Vec<SuggestedEntry> {
    try_suggest(client, chunks).await.unwrap_or_default()
}

async fn try_suggest(
    client: &fubbik_ai::OllamaClient,
    chunks: &[(String, String)],
) -> Option<Vec<SuggestedEntry>> {
    // Truncate chunk content to fit context (~8000 chars total), matching
    // Node's loop exactly: `chars().count()` stands in for JS's UTF-16
    // `.length` (equal for ASCII/BMP text, the expected shape here).
    let mut total_chars = 0usize;
    let mut truncated_chunks: Vec<String> = Vec::new();
    for (title, content) in chunks {
        let entry = format!("### {title}\n{content}");
        let entry_len = entry.chars().count();
        if total_chars + entry_len > 8000 {
            let remaining = 8000usize.saturating_sub(total_chars);
            if remaining > 100 {
                let truncated: String = entry.chars().take(remaining).collect();
                truncated_chunks.push(truncated);
            }
            break;
        }
        truncated_chunks.push(entry);
        total_chars += entry_len;
    }

    let prompt = format!("{PROMPT_TEMPLATE}{}", truncated_chunks.join("\n\n"));

    // Node does NOT pass `format: "json"` here — it asks for a bare
    // completion and then greedily extracts the first `[` to the last `]`,
    // because llama3.2 reliably wraps the array in prose. `generate_json`
    // would reject that prose, so this path keeps the raw-string contract
    // and does its own extraction below.
    let response_text: String = client.generate_raw(&prompt, "llama3.2").await.ok()?;
    let response_text = response_text.trim();

    // Node: `responseText.match(/\[[\s\S]*\]/)` — greedy, first `[` to
    // last `]` in the whole string.
    let (Some(start), Some(end)) = (response_text.find('['), response_text.rfind(']')) else {
        return Some(Vec::new());
    };
    if end < start {
        return Some(Vec::new());
    }
    let json_slice = &response_text[start..=end];

    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_slice) else {
        return Some(Vec::new());
    };
    let Some(arr) = parsed.as_array() else {
        return Some(Vec::new());
    };

    let mut entries = Vec::new();
    for item in arr {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let Some(word) = obj.get("word").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(category) = obj.get("category").and_then(|v| v.as_str()) else {
            continue;
        };
        let word = word.trim().to_lowercase();
        let category = category.trim().to_lowercase();
        if word.is_empty() || !is_valid_category(&category) {
            continue;
        }

        let expects = obj
            .get("expects")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|e| e.as_str())
                    .filter(|e| is_valid_category(&e.to_lowercase()))
                    .map(|e| e.to_lowercase())
                    .collect::<Vec<String>>()
            })
            .filter(|v| !v.is_empty());

        entries.push(SuggestedEntry {
            word,
            category,
            expects,
        });
    }

    Some(entries)
}
