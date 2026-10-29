//! Which `fubbik.toml` feature switch each MCP tool depends on.
//!
//! Tools not listed here belong to the always-on core (chunks, context,
//! search, vocabulary) and are never hidden. A tool is mapped by the endpoint
//! it calls, so groups may span features.

use crate::tool_groups;

/// The feature key a tool needs, or `None` for core tools.
pub(crate) fn feature_for_tool(name: &str) -> Option<&'static str> {
    if tool_groups::plan::handles(name) {
        return Some("plans");
    }
    if tool_groups::matrix::handles(name) {
        return Some("matrices");
    }
    if tool_groups::coordination::handles(name) {
        return Some("coordination");
    }
    match name {
        // Tasks are served by the plans router; `get_context_for_task` reads /api/plans.
        "add_task" | "list_tasks" | "complete_task" | "get_context_for_task" => Some("plans"),
        "list_requirements"
        | "create_requirement"
        | "update_requirement_status"
        | "suggest_requirements"
        | "create_requirements_batch"
        | "get_requirements" => Some("requirements"),
        _ => None,
    }
}

/// Whether `enabled` (unknown when `None`) allows the tool.
pub(crate) fn is_enabled(name: &str, enabled: Option<&[String]>) -> bool {
    match (feature_for_tool(name), enabled) {
        (Some(key), Some(enabled)) => enabled.iter().any(|feature| feature == key),
        _ => true,
    }
}
