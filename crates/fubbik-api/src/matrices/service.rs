//! Business logic for the matrices domain.
//!
//! The ownership guards live in SQL (`fubbik_db::repo::behavior_matrix`), so
//! this layer's job is validation, `Ok(None) -> 404` mapping, and the two
//! things the repository deliberately does not do: snapshotting a rule before
//! it is edited, and deriving each cell's status for the computed view.

use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::behavior_matrix as repo;
use sqlx::PgPool;
use std::collections::HashMap;

use super::dto::*;

// ---------------------------------------------------------------------------
// Route-schema limits
// ---------------------------------------------------------------------------

const LAYERS: [&str; 2] = ["invariant", "contract"];
const CODE_KINDS: [&str; 3] = ["file", "symbol", "test"];
const TEST_STATUSES: [&str; 2] = ["pass", "fail"];

fn check_len(value: &str, max: usize, field: &str) -> AppResult<()> {
    if value.chars().count() > max {
        return Err(AppError::Validation(format!(
            "{field} must be at most {max} characters"
        )));
    }
    Ok(())
}

fn check_one_of(value: &str, allowed: &[&str], field: &str) -> AppResult<()> {
    if !allowed.contains(&value) {
        return Err(AppError::Validation(format!(
            "{field} must be one of {}",
            allowed.join(", ")
        )));
    }
    Ok(())
}

/// `Ok(None)` from the repository means the ownership guard rejected the
/// call, which is a 404 rather than a 403 — the same posture the rest of this
/// crate takes, so an unauthorized caller cannot distinguish "not yours" from
/// "does not exist".
fn found<T>(row: Option<T>, resource: &str) -> AppResult<T> {
    row.ok_or_else(|| AppError::NotFound(resource.into()))
}

// ---------------------------------------------------------------------------
// Matrix
// ---------------------------------------------------------------------------

pub async fn create(
    pool: &PgPool,
    user_id: &str,
    body: CreateMatrixBody,
) -> AppResult<repo::BehaviorMatrix> {
    check_one_of(&body.layer, &LAYERS, "layer")?;
    check_len(&body.name, 200, "name")?;
    if let Some(d) = body.description.as_deref() {
        check_len(d, 1000, "description")?;
    }
    // A foreign `spaceId` yields `Ok(None)` from the repo — reported as a
    // validation error rather than a 404, because the thing that was not
    // found is the space named in the body, not the matrix being created.
    repo::create(
        pool,
        user_id,
        repo::NewMatrix {
            name: body.name,
            layer: body.layer,
            description: body.description,
            space_id: body.space_id,
        },
    )
    .await?
    .ok_or_else(|| AppError::Validation("Invalid or unknown spaceId".into()))
}

pub async fn detail(pool: &PgPool, id: &str, user_id: &str) -> AppResult<MatrixDetail> {
    let matrix = found(repo::find_by_id(pool, id, user_id).await?, "Matrix")?;
    Ok(MatrixDetail {
        matrix,
        dimensions: repo::dimensions_for_matrix(pool, id, user_id).await?,
        rules: repo::rules_for_matrix(pool, id, user_id).await?,
    })
}

pub async fn list(
    pool: &PgPool,
    user_id: &str,
    query: ListMatricesQuery,
) -> AppResult<Vec<repo::BehaviorMatrix>> {
    repo::list(
        pool,
        user_id,
        query.space_id.as_deref(),
        query.layer.as_deref(),
    )
    .await
}

pub async fn update(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    body: UpdateMatrixBody,
) -> AppResult<repo::BehaviorMatrix> {
    if let Some(n) = body.name.as_deref() {
        check_len(n, 200, "name")?;
    }
    if let Some(Some(d)) = body.description.as_ref() {
        check_len(d, 1000, "description")?;
    }
    found(
        repo::update(
            pool,
            id,
            user_id,
            repo::MatrixPatch {
                name: body.name,
                description: body.description,
            },
        )
        .await?,
        "Matrix",
    )
}

pub async fn delete(pool: &PgPool, id: &str, user_id: &str) -> AppResult<()> {
    found(repo::delete(pool, id, user_id).await?, "Matrix")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Dimensions
// ---------------------------------------------------------------------------

pub async fn add_dimension(
    pool: &PgPool,
    matrix_id: &str,
    user_id: &str,
    body: DimensionBody,
) -> AppResult<repo::BehaviorDimension> {
    check_len(&body.name, 100, "name")?;
    found(
        repo::create_dimension(pool, matrix_id, user_id, &body.name).await?,
        "Matrix",
    )
}

pub async fn rename_dimension(
    pool: &PgPool,
    matrix_id: &str,
    dim_id: &str,
    user_id: &str,
    body: DimensionBody,
) -> AppResult<repo::BehaviorDimension> {
    check_len(&body.name, 100, "name")?;
    found(
        repo::update_dimension(pool, dim_id, matrix_id, user_id, &body.name).await?,
        "Dimension",
    )
}

pub async fn remove_dimension(
    pool: &PgPool,
    matrix_id: &str,
    dim_id: &str,
    user_id: &str,
) -> AppResult<()> {
    found(
        repo::delete_dimension(pool, dim_id, matrix_id, user_id).await?,
        "Dimension",
    )?;
    Ok(())
}

/// Reorders, then 404s if the matrix is not the caller's.
///
/// The repository skips ids that belong elsewhere, so a zero affected-row
/// count is ambiguous on its own — it means either "not your matrix" or "none
/// of those ids live here". The explicit ownership read disambiguates, so a
/// caller reordering with a stale id list gets a 200 rather than a confusing
/// 404.
pub async fn reorder_dimensions(
    pool: &PgPool,
    matrix_id: &str,
    user_id: &str,
    dimension_ids: Vec<String>,
) -> AppResult<()> {
    found(repo::find_by_id(pool, matrix_id, user_id).await?, "Matrix")?;
    repo::reorder_dimensions(pool, matrix_id, user_id, &dimension_ids).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

/// The five `maxLength`s Node puts on a rule body, in one place so create and
/// update cannot drift apart.
fn check_rule_fields(
    title: Option<&str>,
    description: Option<&str>,
    category: Option<&str>,
    long_fields: [(&str, Option<&str>); 4],
) -> AppResult<()> {
    if let Some(t) = title {
        check_len(t, 200, "title")?;
    }
    if let Some(d) = description {
        check_len(d, 1000, "description")?;
    }
    if let Some(c) = category {
        check_len(c, 100, "category")?;
    }
    for (field, value) in long_fields {
        if let Some(v) = value {
            check_len(v, 2000, field)?;
        }
    }
    Ok(())
}

pub async fn add_rule(
    pool: &PgPool,
    matrix_id: &str,
    user_id: &str,
    body: CreateRuleBody,
) -> AppResult<repo::BehaviorRule> {
    check_rule_fields(
        Some(&body.title),
        body.description.as_deref(),
        body.category.as_deref(),
        [
            ("rationale", body.rationale.as_deref()),
            ("alternatives", body.alternatives.as_deref()),
            ("consequences", body.consequences.as_deref()),
            ("counterexample", body.counterexample.as_deref()),
        ],
    )?;
    found(
        repo::create_rule(
            pool,
            matrix_id,
            user_id,
            repo::NewRule {
                title: body.title,
                description: body.description,
                category: body.category,
                rationale: body.rationale,
                alternatives: body.alternatives,
                consequences: body.consequences,
                counterexample: body.counterexample,
            },
        )
        .await?,
        "Matrix",
    )
}

/// Snapshots the pre-edit rule into append-only history **before** mutating,
/// matching Node (`matrices/service.ts:193-208`).
///
/// Validation runs before the snapshot, so a rejected PATCH leaves no history
/// entry for an edit that never happened — the same ordering `chunks::update`
/// documents.
pub async fn update_rule(
    pool: &PgPool,
    matrix_id: &str,
    rule_id: &str,
    user_id: &str,
    body: UpdateRuleBody,
) -> AppResult<repo::BehaviorRule> {
    check_rule_fields(
        body.title.as_deref(),
        body.description.as_ref().and_then(|d| d.as_deref()),
        body.category.as_ref().and_then(|c| c.as_deref()),
        [
            (
                "rationale",
                body.rationale.as_ref().and_then(|v| v.as_deref()),
            ),
            (
                "alternatives",
                body.alternatives.as_ref().and_then(|v| v.as_deref()),
            ),
            (
                "consequences",
                body.consequences.as_ref().and_then(|v| v.as_deref()),
            ),
            (
                "counterexample",
                body.counterexample.as_ref().and_then(|v| v.as_deref()),
            ),
        ],
    )?;

    let existing = found(
        repo::find_rule(pool, rule_id, matrix_id, user_id).await?,
        "Rule",
    )?;
    repo::insert_rule_version(
        pool,
        rule_id,
        matrix_id,
        user_id,
        &repo::BehaviorRuleSnapshot {
            title: existing.title,
            description: existing.description,
            category: existing.category,
            rationale: existing.rationale,
            alternatives: existing.alternatives,
            consequences: existing.consequences,
            counterexample: existing.counterexample,
        },
    )
    .await?;

    found(
        repo::update_rule(
            pool,
            rule_id,
            matrix_id,
            user_id,
            repo::RulePatch {
                title: body.title,
                description: body.description,
                category: body.category,
                rationale: body.rationale,
                alternatives: body.alternatives,
                consequences: body.consequences,
                counterexample: body.counterexample,
            },
        )
        .await?,
        "Rule",
    )
}

pub async fn rule_history(
    pool: &PgPool,
    matrix_id: &str,
    rule_id: &str,
    user_id: &str,
) -> AppResult<Vec<repo::BehaviorRuleVersion>> {
    found(repo::find_by_id(pool, matrix_id, user_id).await?, "Matrix")?;
    repo::rule_versions(pool, rule_id, matrix_id, user_id).await
}

pub async fn remove_rule(
    pool: &PgPool,
    matrix_id: &str,
    rule_id: &str,
    user_id: &str,
) -> AppResult<()> {
    found(
        repo::delete_rule(pool, rule_id, matrix_id, user_id).await?,
        "Rule",
    )?;
    Ok(())
}

/// See [`reorder_dimensions`] for why ownership is read explicitly.
pub async fn reorder_rules(
    pool: &PgPool,
    matrix_id: &str,
    user_id: &str,
    rule_ids: Vec<String>,
) -> AppResult<()> {
    found(repo::find_by_id(pool, matrix_id, user_id).await?, "Matrix")?;
    repo::reorder_rules(pool, matrix_id, user_id, &rule_ids).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Cells
// ---------------------------------------------------------------------------

/// Creates the cell if absent, deletes it if present.
///
/// Refuses to delete a cell that still has requirements linked, matching
/// Node — the message names the count so the UI can tell the user what to
/// unlink.
pub async fn toggle_cell(
    pool: &PgPool,
    matrix_id: &str,
    user_id: &str,
    body: ToggleCellBody,
) -> AppResult<ToggleCellResponse> {
    let existing =
        repo::find_cell(pool, &body.rule_id, &body.dimension_id, matrix_id, user_id).await?;

    match existing {
        None => {
            // `Ok(None)` here means the rule or the dimension does not belong
            // to this matrix (or the matrix is not the caller's) — the repo
            // checks all three in one INSERT.
            let cell = found(
                repo::create_cell(pool, &body.rule_id, &body.dimension_id, matrix_id, user_id)
                    .await?,
                "Rule or dimension",
            )?;
            Ok(ToggleCellResponse {
                action: "created".into(),
                cell,
            })
        }
        Some(cell) => {
            let count = repo::cell_requirement_count(pool, &cell.id, matrix_id, user_id).await?;
            if count > 0 {
                return Err(AppError::Validation(format!(
                    "Cell has {count} linked requirement(s). Unlink them first."
                )));
            }
            let deleted = found(
                repo::delete_cell(pool, &cell.id, matrix_id, user_id).await?,
                "Cell",
            )?;
            Ok(ToggleCellResponse {
                action: "deleted".into(),
                cell: deleted,
            })
        }
    }
}

pub async fn link_requirement(
    pool: &PgPool,
    matrix_id: &str,
    cell_id: &str,
    user_id: &str,
    body: LinkRequirementBody,
) -> AppResult<repo::CellRequirementLink> {
    // `Ok(None)` covers three cases the repo checks in one statement: the
    // cell is not in this matrix, the matrix is not the caller's, or the
    // requirement is not the caller's. It also covers a duplicate link
    // (`ON CONFLICT DO NOTHING`), so a re-link of an existing pair is
    // reported as a conflict rather than a 404.
    match repo::link_cell_requirement(pool, cell_id, &body.requirement_id, matrix_id, user_id)
        .await?
    {
        Some(link) => Ok(link),
        None => {
            if repo::find_cell_by_id(pool, cell_id, matrix_id, user_id)
                .await?
                .is_some()
            {
                Err(AppError::Conflict(
                    "requirement is already linked to this cell, or is not yours".into(),
                ))
            } else {
                Err(AppError::NotFound("Cell".into()))
            }
        }
    }
}

pub async fn unlink_requirement(
    pool: &PgPool,
    matrix_id: &str,
    cell_id: &str,
    user_id: &str,
    requirement_id: &str,
) -> AppResult<()> {
    found(
        repo::unlink_cell_requirement(pool, cell_id, requirement_id, matrix_id, user_id).await?,
        "Cell-Requirement link",
    )?;
    Ok(())
}

pub async fn requirements_for_cell(
    pool: &PgPool,
    matrix_id: &str,
    cell_id: &str,
    user_id: &str,
) -> AppResult<Vec<repo::CellRequirement>> {
    found(
        repo::find_cell_by_id(pool, cell_id, matrix_id, user_id).await?,
        "Cell",
    )?;
    repo::requirements_for_cell(pool, cell_id, matrix_id, user_id).await
}

pub async fn link_code(
    pool: &PgPool,
    matrix_id: &str,
    cell_id: &str,
    user_id: &str,
    body: LinkCodeBody,
) -> AppResult<repo::BehaviorCellCode> {
    check_one_of(&body.kind, &CODE_KINDS, "kind")?;
    let code_ref = body.code_ref.trim();
    if code_ref.is_empty() {
        return Err(AppError::Validation("Code link ref is required".into()));
    }
    check_len(code_ref, 500, "ref")?;

    match repo::link_cell_code(pool, cell_id, &body.kind, code_ref, matrix_id, user_id).await? {
        Some(row) => Ok(row),
        None => {
            if repo::find_cell_by_id(pool, cell_id, matrix_id, user_id)
                .await?
                .is_some()
            {
                Err(AppError::Conflict(
                    "this code link already exists on the cell".into(),
                ))
            } else {
                Err(AppError::NotFound("Cell".into()))
            }
        }
    }
}

pub async fn unlink_code(
    pool: &PgPool,
    matrix_id: &str,
    cell_id: &str,
    user_id: &str,
    code_id: &str,
) -> AppResult<()> {
    found(
        repo::delete_cell_code(pool, code_id, cell_id, matrix_id, user_id).await?,
        "Code link",
    )?;
    Ok(())
}

pub async fn code_for_cell(
    pool: &PgPool,
    matrix_id: &str,
    cell_id: &str,
    user_id: &str,
) -> AppResult<Vec<repo::BehaviorCellCode>> {
    found(
        repo::find_cell_by_id(pool, cell_id, matrix_id, user_id).await?,
        "Cell",
    )?;
    repo::code_for_cell(pool, cell_id, matrix_id, user_id).await
}

pub async fn record_test_result(
    pool: &PgPool,
    matrix_id: &str,
    cell_id: &str,
    user_id: &str,
    body: TestResultBody,
) -> AppResult<repo::BehaviorTestResult> {
    check_one_of(&body.status, &TEST_STATUSES, "status")?;
    found(
        repo::record_test_result(
            pool,
            cell_id,
            &body.test_ref,
            &body.status,
            body.detail.as_deref(),
            matrix_id,
            user_id,
        )
        .await?,
        "Cell",
    )
}

pub async fn test_results_for_cell(
    pool: &PgPool,
    matrix_id: &str,
    cell_id: &str,
    user_id: &str,
) -> AppResult<Vec<repo::BehaviorTestResult>> {
    found(
        repo::find_cell_by_id(pool, cell_id, matrix_id, user_id).await?,
        "Cell",
    )?;
    repo::test_results_for_cell(pool, cell_id, matrix_id, user_id).await
}

pub async fn behaviors_for_path(
    pool: &PgPool,
    user_id: &str,
    path: &str,
) -> AppResult<Vec<repo::BehaviorForFile>> {
    repo::behaviors_for_path(pool, user_id, path).await
}

// ---------------------------------------------------------------------------
// The computed view
// ---------------------------------------------------------------------------

/// Derives a cell's status from its evidence.
///
/// The precedence is Node's exactly (`matrices/service.ts:365-380`) and the
/// order is the whole point:
///
/// 1. **violated** — a failing requirement OR a failing test. Checked first
///    so a cell with both a passing and a failing test reads as violated;
///    evidence of breakage outranks evidence of working.
/// 2. **verified** — passing test evidence. Real runs outrank a written spec.
/// 3. **specified** — requirements linked, but nothing has run.
/// 4. **unspecified** — the cell exists but carries no evidence at all.
///
/// Note `code_count` never affects status: linking code says where a
/// behaviour lives, not whether it holds.
fn cell_status(cell: &fubbik_db::repo::behavior_matrix::MatrixViewCell) -> CellStatus {
    if cell.failing_count > 0 || cell.failing_test_count > 0 {
        CellStatus::Violated
    } else if cell.passing_test_count > 0 {
        CellStatus::Verified
    } else if cell.requirement_count > 0 {
        CellStatus::Specified
    } else {
        CellStatus::Unspecified
    }
}

pub async fn view(pool: &PgPool, matrix_id: &str, user_id: &str) -> AppResult<MatrixView> {
    let matrix = found(repo::find_by_id(pool, matrix_id, user_id).await?, "Matrix")?;
    let dimensions = repo::dimensions_for_matrix(pool, matrix_id, user_id).await?;
    let rules = repo::rules_for_matrix(pool, matrix_id, user_id).await?;
    let rows = repo::matrix_view_cells(pool, matrix_id, user_id).await?;

    let mut cells = HashMap::with_capacity(rows.len());
    let mut summary = ViewSummary {
        specified: 0,
        unspecified: 0,
        violated: 0,
        verified: 0,
        total: 0,
    };

    for row in &rows {
        let status = cell_status(row);
        match status {
            CellStatus::Specified => summary.specified += 1,
            CellStatus::Unspecified => summary.unspecified += 1,
            CellStatus::Violated => summary.violated += 1,
            CellStatus::Verified => summary.verified += 1,
        }
        cells.insert(
            format!("{}:{}", row.rule_id, row.dimension_id),
            ViewCell {
                id: row.id.clone(),
                status,
                requirement_count: row.requirement_count,
                code_count: row.code_count,
                passing_test_count: row.passing_test_count,
                failing_test_count: row.failing_test_count,
            },
        );
    }
    summary.total = summary.specified + summary.unspecified + summary.violated + summary.verified;

    Ok(MatrixView {
        matrix,
        dimensions,
        rules,
        cells,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fubbik_db::repo::behavior_matrix::MatrixViewCell;

    fn cell(
        requirement_count: i64,
        failing_count: i64,
        passing_test_count: i64,
        failing_test_count: i64,
    ) -> MatrixViewCell {
        MatrixViewCell {
            id: "c".into(),
            rule_id: "r".into(),
            dimension_id: "d".into(),
            requirement_count,
            failing_count,
            code_count: 0,
            passing_test_count,
            failing_test_count,
        }
    }

    /// The precedence, enumerated. Each case is chosen so that a rule
    /// evaluated in the wrong order produces a different answer — a cell with
    /// both passing and failing tests is the one that catches an
    /// "if passing then verified" check placed first.
    #[test]
    fn cell_status_precedence() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(cell_status(&cell(0, 0, 0, 0)), CellStatus::Unspecified);
        assert_eq!(cell_status(&cell(2, 0, 0, 0)), CellStatus::Specified);
        assert_eq!(cell_status(&cell(2, 0, 3, 0)), CellStatus::Verified);
        assert_eq!(
            cell_status(&cell(2, 1, 0, 0)),
            CellStatus::Violated,
            "a failing requirement violates, even with no test evidence"
        );
        assert_eq!(
            cell_status(&cell(2, 0, 0, 1)),
            CellStatus::Violated,
            "a failing test violates, even with requirements satisfied"
        );
        assert_eq!(
            cell_status(&cell(2, 0, 5, 1)),
            CellStatus::Violated,
            "one failing test outranks five passing ones — breakage beats working"
        );
        assert_eq!(
            cell_status(&cell(0, 0, 1, 0)),
            CellStatus::Verified,
            "a passing test verifies even with no requirement written down"
        );
    }
}
