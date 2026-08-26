//! The `GET /api/graph` response.
//!
//! Seven fields, one per thing the web actually reads. Node returns thirteen;
//! the other six (`communities`, `bridges`, `codeFiles`, `codeSymbols`,
//! `concepts`, `coRefEdges`) have no reader in any client, and three of them
//! are permanently empty on Node too. Because `apps/web` types itself from
//! `openapi.json`, omitting them turns a future reader into a compile error
//! instead of a silent `undefined`.

use fubbik_db::age::{BehaviorRuleVertex, GovernsEdge};
use fubbik_db::repo::graph::{ChunkMeta, ChunkSpaceMapping, ChunkTagWithType, GraphConnection};
use fubbik_db::repo::tag_type::TagType;

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphResponse {
    pub chunks: Vec<ChunkMeta>,
    pub connections: Vec<GraphConnection>,
    pub chunk_tags: Vec<ChunkTagWithType>,
    pub tag_types: Vec<TagType>,
    /// Named `chunkCodebases` on the wire: the `codebase → space` rename never
    /// reached this field, and `apps/web/src/features/graph/group-strategies.ts:69`
    /// destructures the old name. Renaming it is a web change, not a port.
    #[serde(rename = "chunkCodebases")]
    pub chunk_spaces: Vec<ChunkSpaceMapping>,
    pub behavior_rules: Vec<BehaviorRuleVertex>,
    pub governs_edges: Vec<GovernsEdge>,
}
