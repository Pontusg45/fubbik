use fubbik_db::repo::coordination::{AgentRun, CoordinationEntry, TaskClaim};

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JoinRunBody {
    pub handle: String,
    pub parent_run_id: Option<String>,
    pub external_key: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default = "empty_object")]
    pub metadata: serde_json::Value,
}

fn empty_object() -> serde_json::Value {
    serde_json::json!({})
}

#[derive(Debug, Default, serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct BoardQuery {
    pub run_id: Option<String>,
    pub after_sequence: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ClaimAction {
    Claim,
    Renew,
    Release,
}

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimBody {
    pub run_id: String,
    pub action: ClaimAction,
    pub lease_seconds: Option<i64>,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimResponse {
    pub action: String,
    pub claim: Option<TaskClaim>,
}

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransitionTaskBody {
    pub run_id: String,
    pub status: String,
    pub note: Option<String>,
    pub client_mutation_id: String,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransitionTaskResponse {
    pub task: fubbik_db::repo::plan::PlanTask,
    pub entry: CoordinationEntry,
}

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
#[schema(as = CoordinationCreateEntryBody)]
#[serde(rename_all = "camelCase")]
pub struct CreateEntryBody {
    pub run_id: String,
    pub recipient_run_id: Option<String>,
    pub task_id: Option<String>,
    pub reply_to_id: Option<String>,
    pub kind: String,
    pub body: String,
    #[serde(default = "empty_object")]
    pub metadata: serde_json::Value,
    pub client_mutation_id: String,
}

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AckRunBody {
    pub through_sequence: i64,
    pub status: Option<String>,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BoardPlan {
    pub id: String,
    pub title: String,
    pub status: String,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BoardTask {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub order: i32,
    pub depends_on: Vec<String>,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BoardCursor {
    pub next_sequence: i64,
    pub acknowledged_sequence: Option<i64>,
    pub has_more: bool,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BoardSnapshot {
    pub plan: BoardPlan,
    pub tasks: Vec<BoardTask>,
    pub runs: Vec<AgentRun>,
    pub claims: Vec<TaskClaim>,
    pub entries: Vec<CoordinationEntry>,
    pub cursor: BoardCursor,
}
