use super::*;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredChunk {
    pub title: String,
    pub content: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub tags: Vec<String>,
    pub tier: u8,
    pub category: String,
    pub source: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub applies_to: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredConnection {
    pub source_title: String,
    pub target_title: String,
    pub relation: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Tip {
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct Discovery {
    pub root: PathBuf,
    pub chunks: Vec<DiscoveredChunk>,
    pub connections: Vec<DiscoveredConnection>,
    pub tags: Vec<String>,
    pub tips: Vec<Tip>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportError {
    pub item: String,
    pub error: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub chunks_created: i64,
    pub connections_created: usize,
    pub errors: Vec<ImportError>,
}
