use fubbik_ai::OllamaClient;
use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::{chunk, vocabulary};
use sqlx::PgPool;

use super::dto::{
    AiConnectionSuggestion, AiRequirementStep, GeneratedChunk, StructuredRequirement,
    SummaryResponse,
};

const GENERATION_MODEL: &str = "llama3.2";

pub async fn summarize(
    pool: &PgPool,
    ai: &OllamaClient,
    user_id: &str,
    chunk_id: &str,
) -> AppResult<SummaryResponse> {
    let chunk = owned_chunk(pool, user_id, chunk_id).await?;
    let prompt = format!(
        "Summarize this knowledge chunk in 2-3 sentences:\n\nTitle: {}\n\nContent: {}",
        chunk.title, chunk.content
    );
    let summary = ai.generate_raw(&prompt, GENERATION_MODEL).await?;
    Ok(SummaryResponse { summary })
}

pub async fn suggest_connections(
    pool: &PgPool,
    ai: &OllamaClient,
    user_id: &str,
    chunk_id: &str,
) -> AppResult<Vec<AiConnectionSuggestion>> {
    let target = owned_chunk(pool, user_id, chunk_id).await?;
    let chunk_list = chunk::list_titles(pool, user_id)
        .await?
        .into_iter()
        .filter(|candidate| candidate.id != target.id)
        .map(|candidate| format!("- {}: {}", candidate.id, candidate.title))
        .collect::<Vec<_>>()
        .join("\n");
    let prompt = format!(
        "Given this chunk:\nTitle: {}\nContent: {}\n\nSuggest which of these chunks it should be connected to and why. Return a JSON array of objects with \"id\" and \"relation\" fields only:\n{}",
        target.title, target.content, chunk_list
    );
    let raw = ai.generate_raw(&prompt, GENERATION_MODEL).await?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

pub async fn generate(ai: &OllamaClient, prompt: &str) -> AppResult<GeneratedChunk> {
    let request = format!(
        "Generate a knowledge chunk based on this prompt. Return valid JSON only with these fields: title (string), content (string), type (one of: note, document, reference, schema, checklist), tags (array of strings):\n\n{prompt}"
    );
    let raw = ai.generate_raw(&request, GENERATION_MODEL).await?;
    Ok(
        serde_json::from_str(&raw).unwrap_or_else(|_| GeneratedChunk {
            title: prompt.to_owned(),
            content: raw,
            chunk_type: "note".to_owned(),
            tags: Vec::new(),
        }),
    )
}

pub async fn structure_requirement(
    pool: &PgPool,
    ai: &OllamaClient,
    user_id: &str,
    description: &str,
    space_id: Option<&str>,
) -> AppResult<StructuredRequirement> {
    if description.chars().count() > 5000 {
        return Err(AppError::Validation(
            "description must be at most 5000 characters".into(),
        ));
    }
    if !ai.is_available().await {
        return Err(AppError::External("AI service error".into()));
    }

    let vocabulary_context = if let Some(space_id) = space_id {
        match vocabulary::list(pool, user_id, space_id).await {
            Ok(entries) => {
                let mut categories = std::collections::BTreeMap::<String, Vec<String>>::new();
                for entry in entries {
                    categories
                        .entry(entry.category)
                        .or_default()
                        .push(entry.word);
                }
                categories
                    .into_iter()
                    .map(|(category, words)| format!("{category}: {}", words.join(", ")))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            Err(error) => {
                tracing::warn!("could not load requirement vocabulary: {error}");
                String::new()
            }
        }
    } else {
        String::new()
    };
    let vocab_section = if vocabulary_context.is_empty() {
        String::new()
    } else {
        format!("\nUse this vocabulary if possible:\n{vocabulary_context}\n")
    };
    let prompt = format!(
        "Convert this requirement description into structured Given/When/Then steps.\n{vocab_section}\nDescription: {description}\n\nReturn as JSON: {{ \"steps\": [{{ \"keyword\": \"given\"|\"when\"|\"then\"|\"and\", \"text\": \"...\" }}] }}\n\nRules:\n- Start with \"given\" steps for preconditions\n- Then \"when\" steps for the action\n- Then \"then\" steps for expected outcomes\n- Use \"and\" for additional steps within a phase\n- Keep step text concise and clear\n- Do not include the keyword in the text field"
    );

    #[derive(serde::Deserialize)]
    struct ModelResult {
        steps: Vec<serde_json::Value>,
    }
    let result: ModelResult = ai.generate_json(&prompt, GENERATION_MODEL).await?;
    let steps = result
        .steps
        .into_iter()
        .filter_map(|step| serde_json::from_value::<AiRequirementStep>(step).ok())
        .filter(|step| !step.text.trim().is_empty())
        .collect::<Vec<_>>();
    if steps.is_empty() {
        return Err(AppError::External("AI service error".into()));
    }
    Ok(StructuredRequirement { steps })
}

async fn owned_chunk(pool: &PgPool, user_id: &str, chunk_id: &str) -> AppResult<chunk::Chunk> {
    chunk::find_by_id(pool, user_id, chunk_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Chunk".into()))
}
