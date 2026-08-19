//! Port of `packages/api/src/requirements/dependency-service.ts`.

use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::{requirement, requirement_dependency};
use sqlx::PgPool;

use super::dto::{DependencyGraph, DependencyGraphEdge, DependencyGraphNode, DependencySides};

/// Mirrors Node's `addDependency` (`packages/api/src/requirements/
/// dependency-service.ts:13-27`). Direct self-dependency
/// (`requirement_id == depends_on_id`) is rejected by the database's own
/// `no_self_dependency` CHECK constraint (surfaces as `AppError::Database`,
/// same as Node relying on the identical DB constraint and not checking
/// for it here either); cycles of length > 1 are rejected by the
/// application-level [`requirement_dependency::check_circular`] check
/// below, matching Node's own `checkCircularDependency` call.
pub async fn add_dependency(
    pool: &PgPool,
    user_id: &str,
    requirement_id: &str,
    depends_on_id: &str,
) -> AppResult<()> {
    if requirement::find_by_id(pool, user_id, requirement_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound("Requirement".into()));
    }
    if requirement::find_by_id(pool, user_id, depends_on_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound("Dependency target".into()));
    }
    if requirement_dependency::check_circular(pool, requirement_id, depends_on_id).await? {
        return Err(AppError::Validation(
            "Adding this dependency would create a circular reference".into(),
        ));
    }
    requirement_dependency::add(pool, requirement_id, depends_on_id).await?;
    Ok(())
}

/// Mirrors Node's `removeDependency` — only `requirement_id`'s ownership is
/// checked, not `depends_on_id`'s (`packages/api/src/requirements/
/// dependency-service.ts:29-35`).
pub async fn remove_dependency(
    pool: &PgPool,
    user_id: &str,
    requirement_id: &str,
    depends_on_id: &str,
) -> AppResult<()> {
    if requirement::find_by_id(pool, user_id, requirement_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound("Requirement".into()));
    }
    requirement_dependency::remove(pool, requirement_id, depends_on_id).await?;
    Ok(())
}

pub async fn get_dependencies(
    pool: &PgPool,
    user_id: &str,
    requirement_id: &str,
) -> AppResult<DependencySides> {
    if requirement::find_by_id(pool, user_id, requirement_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound("Requirement".into()));
    }
    let deps = requirement_dependency::get(pool, requirement_id).await?;
    Ok(DependencySides {
        depends_on: deps.depends_on,
        depended_on_by: deps.depended_on_by,
    })
}

/// Mirrors Node's `getDependencyGraph` (`packages/api/src/requirements/
/// dependency-service.ts:45-68`), including its de-duplication quirk: if
/// the same requirement id appears more than once across `[current,
/// ...ancestors, ...descendants]` (only possible with pre-existing cyclic
/// data, since new cycles are rejected by [`add_dependency`]), the *first*
/// position wins for ordering but the *last* occurrence's fields win for
/// content — matching a JS `Map` built from `[[id, value], ...]` entries,
/// where re-`set`ting an existing key updates its value without moving its
/// iteration position.
pub async fn get_dependency_graph(
    pool: &PgPool,
    user_id: &str,
    requirement_id: &str,
) -> AppResult<DependencyGraph> {
    let req = requirement::find_by_id(pool, user_id, requirement_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Requirement".into()))?;

    let transitive = requirement_dependency::transitive(pool, requirement_id).await?;

    let all_nodes: Vec<DependencyGraphNode> = std::iter::once(DependencyGraphNode {
        id: req.id,
        title: req.title,
        status: req.status,
        priority: req.priority,
        is_current: true,
    })
    .chain(
        transitive
            .ancestors
            .into_iter()
            .map(|r| DependencyGraphNode {
                id: r.id,
                title: r.title,
                status: r.status,
                priority: r.priority,
                is_current: false,
            }),
    )
    .chain(
        transitive
            .descendants
            .into_iter()
            .map(|r| DependencyGraphNode {
                id: r.id,
                title: r.title,
                status: r.status,
                priority: r.priority,
                is_current: false,
            }),
    )
    .collect();

    let mut order: Vec<String> = Vec::new();
    let mut by_id: std::collections::HashMap<String, DependencyGraphNode> =
        std::collections::HashMap::new();
    for node in all_nodes {
        if !by_id.contains_key(&node.id) {
            order.push(node.id.clone());
        }
        by_id.insert(node.id.clone(), node);
    }
    let nodes: Vec<DependencyGraphNode> = order
        .into_iter()
        .map(|id| by_id.remove(&id).expect("just inserted"))
        .collect();

    let edges = transitive
        .edges
        .into_iter()
        .map(|e| DependencyGraphEdge {
            source: e.source,
            target: e.target,
        })
        .collect();

    Ok(DependencyGraph { nodes, edges })
}
