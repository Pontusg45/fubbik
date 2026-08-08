use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::settings::{self, CodebaseSetting, InstanceSetting, UserSetting};
use fubbik_db::repo::space;
use sqlx::PgPool;

use super::dto::{FeatureFlags, SettingsMap};

/// Shared reduction: every "get all settings for a scope" endpoint folds
/// its rows into a bare `{key: value}` map. The three repo modules can't
/// share SQL (three different tables, three different scope columns), but
/// every caller of them converges on this one shape — this function is
/// where the task brief's "one shared key-value pattern... rather than
/// three divergent implementations" actually lives in code.
fn to_map<I: IntoIterator<Item = (String, serde_json::Value)>>(rows: I) -> SettingsMap {
    rows.into_iter().collect()
}

pub async fn get_all_user_settings(pool: &PgPool, user_id: &str) -> AppResult<SettingsMap> {
    let rows = settings::list_user_settings(pool, user_id).await?;
    Ok(to_map(rows.into_iter().map(|r| (r.key, r.value.0))))
}

pub async fn set_user_setting(
    pool: &PgPool,
    user_id: &str,
    key: &str,
    value: serde_json::Value,
) -> AppResult<UserSetting> {
    let id = fubbik_db::new_id();
    settings::set_user_setting(pool, &id, user_id, key, value).await
}

/// Checks space ownership up front via `space::find_by_id` (404 if the
/// space isn't the caller's or doesn't exist) before touching
/// `codebase_settings` — the same shape as
/// `chunks::routes::get_applies_to` pre-checking chunk ownership via
/// `service::get` before calling `chunk_meta::get_applies_to`. Without
/// this pre-check, `settings::list_codebase_settings`'s own `EXISTS` guard
/// would silently return an empty map instead of a 404, which is
/// indistinguishable from "this space is yours and has no settings yet" —
/// not the behaviour we want for a foreign space.
pub async fn get_all_codebase_settings(
    pool: &PgPool,
    user_id: &str,
    space_id: &str,
) -> AppResult<SettingsMap> {
    space::find_by_id(pool, user_id, space_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Space".into()))?;

    let rows = settings::list_codebase_settings(pool, space_id, user_id).await?;
    Ok(to_map(rows.into_iter().map(|r| (r.key, r.value.0))))
}

/// Mirrors `get_all_codebase_settings`'s ownership pre-check, then relies
/// on `settings::set_codebase_setting`'s own `EXISTS` guard as
/// defense-in-depth against a TOCTOU race between the check and the write
/// (same belt-and-suspenders shape as
/// `workspaces::service::add_space_to_workspace`). The `ok_or_else` after
/// the repo call is a safety net, not the primary guard — the pre-check
/// above is what makes the *ordinary* cross-user case a clean 404 instead
/// of falling through to it.
///
/// This ownership check is a **deliberate divergence from Node**
/// (`packages/api/src/settings/service.ts:39-41` has none at all —
/// see `tests/fixtures/node-contract-2b/_mutating.md`: "No
/// ownership/existence check on `codebaseId`"), required by the task
/// brief ("writing settings on another user's space must be impossible"),
/// not a silent addition.
pub async fn set_codebase_setting(
    pool: &PgPool,
    user_id: &str,
    space_id: &str,
    key: &str,
    value: serde_json::Value,
) -> AppResult<CodebaseSetting> {
    space::find_by_id(pool, user_id, space_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Space".into()))?;

    let id = fubbik_db::new_id();
    settings::set_codebase_setting(pool, &id, space_id, user_id, key, value)
        .await?
        .ok_or_else(|| AppError::NotFound("Space".into()))
}

/// **Deliberately unauthenticated at the route layer** — see
/// `routes::get_instance_settings`'s doc comment. This function itself
/// takes no `user_id` because `instance_settings` has no owner column at
/// all; that is the whole point of the table.
pub async fn get_all_instance_settings(pool: &PgPool) -> AppResult<SettingsMap> {
    let rows = settings::list_instance_settings(pool).await?;
    Ok(to_map(rows.into_iter().map(|r| (r.key, r.value.0))))
}

pub async fn set_instance_setting(
    pool: &PgPool,
    key: &str,
    value: serde_json::Value,
) -> AppResult<InstanceSetting> {
    settings::set_instance_setting(pool, key, value).await
}

/// Computed view over `instance_settings` — see `dto::FeatureFlags`'s doc
/// comment for the exact semantics being ported.
pub async fn get_feature_flags(pool: &PgPool) -> AppResult<FeatureFlags> {
    let map = get_all_instance_settings(pool).await?;
    let flag = |key: &str| map.get(key).and_then(|v| v.as_bool()).unwrap_or(true);

    Ok(FeatureFlags {
        ai_enabled: flag("aiEnabled"),
        enrichment_enabled: flag("enrichmentEnabled"),
        semantic_search_enabled: flag("semanticSearchEnabled"),
        ai_suggestions_enabled: flag("aiSuggestionsEnabled"),
        vocabulary_suggest_enabled: flag("vocabularySuggestEnabled"),
    })
}
