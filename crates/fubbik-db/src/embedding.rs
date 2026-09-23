//! Wire representation for the `embedding` pgvector column.
//!
//! `embedding` is a 768-dimension `vector` column. Node's Drizzle schema
//! decodes it to a plain `number[]` (see `packages/db/src/schema/chunk.ts`'s
//! `customType` for `vector`, whose `fromDriver` strips the surrounding
//! `[...]` and splits on `,`) and serialises `null` when unset — which is
//! what every sampled row returned in practice.
//!
//! Rather than add the `pgvector` crate purely to round-trip a column most
//! rows leave `null`, this reproduces Node's own decoding: the column is
//! selected as `embedding::text` (a plain Postgres cast, no new dependency)
//! and `EmbeddingVec` parses that text the same way Node's `fromDriver`
//! does. That gives exact wire parity for both the null case (Node and
//! Rust both emit `null`) and the populated case (both emit a JSON array of
//! numbers) without pulling in a crate whose only job here would be
//! encoding/decoding the pgvector binary format we never otherwise touch.
use sqlx::Decode;
use sqlx::postgres::{PgTypeInfo, PgValueRef};

#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingVec(pub Vec<f32>);

impl serde::Serialize for EmbeddingVec {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

// Declared compatible with TEXT because the column is always selected as
// `embedding::text` — sqlx sees a text value on the wire, never the raw
// pgvector binary format.
impl sqlx::Type<sqlx::Postgres> for EmbeddingVec {
    fn type_info() -> PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

/// Mirrors Node's `fromDriver` for the `vector` custom type exactly:
/// `(value as string).slice(1, -1).split(",").map(Number)`.
fn parse_pgvector_text(raw: &str) -> Result<Vec<f32>, std::num::ParseFloatError> {
    let trimmed = raw.trim().trim_start_matches('[').trim_end_matches(']');
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    trimmed
        .split(',')
        .map(|part| part.trim().parse::<f32>())
        .collect()
}

impl<'r> Decode<'r, sqlx::Postgres> for EmbeddingVec {
    fn decode(value: PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let raw = <&str as Decode<sqlx::Postgres>>::decode(value)?;
        Ok(EmbeddingVec(parse_pgvector_text(raw)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bracketed_csv_like_nodes_fromdriver() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(
            parse_pgvector_text("[0.1,0.25,-3]").unwrap(),
            vec![0.1, 0.25, -3.0]
        );
    }

    #[test]
    fn empty_vector_parses_to_empty() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(parse_pgvector_text("[]").unwrap(), Vec::<f32>::new());
    }

    #[test]
    fn serialises_as_a_plain_json_array() {
        // Given the inline inputs and test fixtures.
        // When
        let v = EmbeddingVec(vec![0.5, 1.5]);
        // Then
        assert_eq!(serde_json::to_string(&v).unwrap(), "[0.5,1.5]");
    }
}
