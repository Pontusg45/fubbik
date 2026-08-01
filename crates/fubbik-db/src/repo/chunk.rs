use chrono::NaiveDateTime;
use fubbik_core::error::AppResult;
use sqlx::PgPool;

/// `camelCase` serialisation is mandatory, not cosmetic: the 106 web files
/// that consume this API were written against Drizzle's camelCase output.
/// Emitting snake_case would silently break every one of them.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub id: String,
    pub title: String,
    pub content: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub user_id: String,
    pub summary: Option<String>,
    pub rationale: Option<String>,
    pub consequences: Option<String>,
    pub origin: String,
    pub review_status: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub archived_at: Option<NaiveDateTime>,
}

pub struct NewChunk {
    pub title: String,
    pub content: String,
    pub chunk_type: String,
    pub rationale: Option<String>,
}

#[derive(Default)]
pub struct ChunkPatch {
    pub title: Option<String>,
    pub content: Option<String>,
    pub chunk_type: Option<String>,
    pub rationale: Option<String>,
    pub consequences: Option<String>,
}


pub async fn create(pool: &PgPool, user_id: &str, new: NewChunk) -> AppResult<Chunk> {
    let id = crate::new_id();
    let c = sqlx::query_as!(
        Chunk,
        r#"INSERT INTO chunk (id, title, content, type, user_id, rationale)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id, title, content, type AS chunk_type, user_id, summary,
                     rationale, consequences, origin, review_status,
                     created_at, updated_at, archived_at"#,
        id,
        new.title,
        new.content,
        new.chunk_type,
        user_id,
        new.rationale
    )
    .fetch_one(pool)
    .await?;
    Ok(c)
}

pub async fn find_by_id(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Option<Chunk>> {
    let c = sqlx::query_as!(
        Chunk,
        r#"SELECT id, title, content, type AS chunk_type, user_id, summary,
                  rationale, consequences, origin, review_status,
                  created_at, updated_at, archived_at
           FROM chunk WHERE id = $1 AND user_id = $2"#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(c)
}

/// Applies only the fields present in the patch. COALESCE keeps unset
/// columns untouched, so a partial PATCH cannot silently clear data.
pub async fn update(pool: &PgPool, user_id: &str, id: &str, patch: ChunkPatch) -> AppResult<Option<Chunk>> {
    let c = sqlx::query_as!(
        Chunk,
        r#"UPDATE chunk SET
             title = COALESCE($3, title),
             content = COALESCE($4, content),
             type = COALESCE($5, type),
             rationale = COALESCE($6, rationale),
             consequences = COALESCE($7, consequences),
             updated_at = now()
           WHERE id = $1 AND user_id = $2
           RETURNING id, title, content, type AS chunk_type, user_id, summary,
                     rationale, consequences, origin, review_status,
                     created_at, updated_at, archived_at"#,
        id,
        user_id,
        patch.title,
        patch.content,
        patch.chunk_type,
        patch.rationale,
        patch.consequences
    )
    .fetch_optional(pool)
    .await?;
    Ok(c)
}

pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let res = sqlx::query!("DELETE FROM chunk WHERE id = $1 AND user_id = $2", id, user_id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Sort {
    #[default]
    Newest,
    Oldest,
    Alpha,
    Updated,
}

pub struct ListParams {
    pub chunk_type: Option<String>,
    pub search: Option<String>,
    pub origin: Option<String>,
    pub review_status: Option<String>,
    pub sort: Sort,
    pub limit: i64,
    pub offset: i64,
}

impl Default for ListParams {
    fn default() -> Self {
        Self {
            chunk_type: None,
            search: None,
            origin: None,
            review_status: None,
            sort: Sort::Newest,
            limit: 50,
            offset: 0,
        }
    }
}

/// Lists a user's non-archived chunks.
///
/// Uses QueryBuilder rather than `query_as!` because the filter set is
/// dynamic. Every user value is pushed as a bind parameter, never
/// formatted into the SQL string.
pub async fn list(pool: &PgPool, user_id: &str, params: ListParams) -> AppResult<Vec<Chunk>> {
    let mut qb = sqlx::QueryBuilder::new(
        "SELECT id, title, content, type AS chunk_type, user_id, summary, \
         rationale, consequences, origin, review_status, \
         created_at, updated_at, archived_at \
         FROM chunk WHERE archived_at IS NULL AND user_id = ",
    );
    qb.push_bind(user_id);

    if let Some(t) = &params.chunk_type {
        qb.push(" AND type = ").push_bind(t);
    }
    if let Some(o) = &params.origin {
        qb.push(" AND origin = ").push_bind(o);
    }
    if let Some(r) = &params.review_status {
        qb.push(" AND review_status = ").push_bind(r);
    }
    if let Some(s) = &params.search {
        // ILIKE with escaped wildcards: a user searching for "100%" must not
        // match everything.
        let pattern = format!("%{}%", s.replace('\\', r"\\").replace('%', r"\%").replace('_', r"\_"));
        qb.push(" AND (title ILIKE ").push_bind(pattern.clone());
        qb.push(" OR content ILIKE ").push_bind(pattern);
        qb.push(")");
    }

    qb.push(match params.sort {
        Sort::Newest => " ORDER BY created_at DESC",
        Sort::Oldest => " ORDER BY created_at ASC",
        Sort::Alpha => " ORDER BY title ASC",
        Sort::Updated => " ORDER BY updated_at DESC",
    });

    qb.push(" LIMIT ").push_bind(params.limit.clamp(1, 500));
    qb.push(" OFFSET ").push_bind(params.offset.max(0));

    let rows = qb.build_query_as::<Chunk>().fetch_all(pool).await?;
    Ok(rows)
}
