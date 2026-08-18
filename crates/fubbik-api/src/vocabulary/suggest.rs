//! Direct port of `packages/api/src/vocabulary/suggest.ts`'s
//! `suggestVocabulary` — asks a local Ollama `llama3.2` model to extract
//! vocabulary entries from a space's chunks.
//!
//! `suggest_vocabulary` is **infallible**, matching Node's
//! `Effect.Effect<SuggestedEntry[], never>` signature: every failure mode
//! (Ollama unreachable, non-2xx response, unparseable JSON, no `[...]`
//! found in the model's output) degrades to an empty `Vec`, never an
//! error — Node's own `.pipe(Effect.catchAll(() => Effect.succeed([])))`
//! (`suggest.ts:116`). This is the first Ollama-calling code path in this
//! Rust port (see `crate::search::service`'s module doc, which notes no
//! such pipeline existed yet); `reqwest` was already a `fubbik-api`
//! dependency, added ahead of this domain landing.

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

#[derive(serde::Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

const PROMPT_TEMPLATE: &str = "You are analyzing code documentation chunks to extract a controlled vocabulary for behavior-driven requirements (BDD/Gherkin style).\n\nGiven the following chunks of documentation/code, extract meaningful vocabulary entries. Each entry has:\n- \"word\": a short word or phrase (1-3 words, lowercase)\n- \"category\": one of \"actor\", \"action\", \"target\", \"outcome\", \"state\", \"modifier\"\n- \"expects\" (optional): array of category names that should follow this word\n\nCategories:\n- actor: who performs the action (e.g., \"user\", \"admin\", \"system\")\n- action: what is done (e.g., \"click\", \"submit\", \"navigate\")\n- target: what the action is performed on (e.g., \"button\", \"form\", \"page\")\n- outcome: what should result (e.g., \"displayed\", \"saved\", \"redirected\")\n- state: a condition (e.g., \"logged in\", \"visible\", \"enabled\")\n- modifier: connecting/clarifying words (e.g., \"the\", \"a\", \"should\")\n\nActions typically expect [\"target\"]. Actors typically expect [\"action\"].\n\nReturn a JSON array of objects. Only return the JSON array, no other text.\n\nCHUNKS:\n";

/// `ollama_url` overrides the `OLLAMA_URL` env var (defaulting to
/// `http://localhost:11434` when neither is set) — matches Node's
/// `suggestVocabulary(chunks, ollamaUrl?)` optional-override parameter,
/// kept for the same reason: tests can point this at a mock server without
/// touching process environment.
pub async fn suggest_vocabulary(
    chunks: &[(String, String)],
    ollama_url: Option<&str>,
) -> Vec<SuggestedEntry> {
    try_suggest(chunks, ollama_url).await.unwrap_or_default()
}

async fn try_suggest(
    chunks: &[(String, String)],
    ollama_url: Option<&str>,
) -> Option<Vec<SuggestedEntry>> {
    let url = ollama_url.map(|s| s.to_string()).unwrap_or_else(|| {
        std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string())
    });

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

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{url}/api/generate"))
        .json(&serde_json::json!({
            "model": "llama3.2",
            "prompt": prompt,
            "stream": false
        }))
        .send()
        .await
        .ok()?;

    if !res.status().is_success() {
        return Some(Vec::new());
    }

    let data: OllamaGenerateResponse = res.json().await.ok()?;
    let response_text = data.response.trim();

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
