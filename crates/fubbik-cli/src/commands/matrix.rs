use anyhow::Result;
use fubbik_contracts::matrix::CellStatus;
use serde_json::{Value, json};

use crate::{
    MatrixCommand,
    client::{Client, MatrixView},
    output::{self, OutputMode},
};

pub async fn run(client: &Client, command: MatrixCommand, mode: OutputMode) -> Result<()> {
    match command {
        MatrixCommand::List { layer } => list(client, layer.as_deref(), mode).await,
        MatrixCommand::Create {
            name,
            layer,
            description,
            space,
        } => {
            create(
                client,
                &name,
                &layer,
                description.as_deref(),
                space.as_deref(),
                mode,
            )
            .await
        }
        MatrixCommand::Show { id } => show(client, &id, mode).await,
        MatrixCommand::AddDimension { matrix_id, name } => {
            let value = client.add_matrix_dimension(&matrix_id, &name).await?;
            mutation(mode, &value, &format!("Added dimension \"{name}\""))
        }
        MatrixCommand::AddRule {
            matrix_id,
            title,
            category,
            rationale,
            alternatives,
            consequences,
            counterexample,
        } => {
            let value = client
                .add_matrix_rule(
                    &matrix_id,
                    json!({
                        "title": &title,
                        "category": category.as_deref(),
                        "rationale": rationale.as_deref(),
                        "alternatives": alternatives.as_deref(),
                        "consequences": consequences.as_deref(),
                        "counterexample": counterexample.as_deref(),
                    }),
                )
                .await?;
            mutation(mode, &value, &format!("Added rule \"{title}\""))
        }
        MatrixCommand::Cell {
            matrix_id,
            rule_id,
            dimension_id,
        } => {
            let value = client
                .toggle_matrix_cell(&matrix_id, &rule_id, &dimension_id)
                .await?;
            let action = string(&value, "action");
            mutation(mode, &value, &format!("Cell {action}"))
        }
        MatrixCommand::Gaps { id } => gaps(client, &id, mode).await,
        MatrixCommand::Link {
            cell_id,
            requirement_id,
            matrix,
        } => {
            let value = client
                .link_matrix_requirement(&matrix, &cell_id, &requirement_id)
                .await?;
            mutation(mode, &value, "Requirement linked to cell")
        }
        MatrixCommand::LinkCode {
            cell_id,
            matrix,
            kind,
            code_ref,
        } => {
            let value = client
                .link_matrix_code(&matrix, &cell_id, &kind, &code_ref)
                .await?;
            mutation(
                mode,
                &value,
                &format!("Linked {kind} \"{code_ref}\" to cell"),
            )
        }
        MatrixCommand::ReportTest {
            cell_id,
            matrix,
            test_ref,
            status,
            detail,
        } => {
            let value = client
                .report_matrix_test(&matrix, &cell_id, &test_ref, &status, detail.as_deref())
                .await?;
            mutation(
                mode,
                &value,
                &format!("Reported {status} for \"{test_ref}\""),
            )
        }
        MatrixCommand::History { rule_id, matrix } => {
            history(client, &matrix, &rule_id, mode).await
        }
        MatrixCommand::BehaviorsFor { path } => behaviors_for(client, &path, mode).await,
    }
}

async fn list(client: &Client, layer: Option<&str>, mode: OutputMode) -> Result<()> {
    let matrices = client.list_matrices(layer).await?;
    let mut lines = Vec::new();
    if matrices.is_empty() {
        lines.push("No matrices found.".to_owned());
    }
    for matrix in &matrices {
        lines.push(format!(
            "  {} [{}] ({})",
            matrix.name, matrix.layer, matrix.id
        ));
        if let Some(description) = &matrix.description {
            lines.push(format!("    {description}"));
        }
    }
    output::render(
        mode,
        output::CommandOutput {
            quiet: matrices.iter().map(|matrix| matrix.id.clone()).collect(),
            data: matrices,
            human: lines.join("\n"),
        },
    )
}

async fn create(
    client: &Client,
    name: &str,
    layer: &str,
    description: Option<&str>,
    space: Option<&str>,
    mode: OutputMode,
) -> Result<()> {
    let space_id = client.resolve_space(space).await?;
    let matrix = client
        .create_matrix(name, layer, description, space_id.as_deref())
        .await?;
    output::render(
        mode,
        output::CommandOutput {
            human: format!("Created matrix \"{}\" ({})", matrix.name, matrix.id),
            quiet: vec![matrix.id.clone()],
            data: matrix,
        },
    )
}

async fn show(client: &Client, id: &str, mode: OutputMode) -> Result<()> {
    let view = client.matrix_view(id).await?;
    output::render(
        mode,
        output::CommandOutput {
            human: render_grid(&view),
            quiet: vec![view.matrix.id.clone()],
            data: view,
        },
    )
}

async fn gaps(client: &Client, id: &str, mode: OutputMode) -> Result<()> {
    let view = client.matrix_view(id).await?;
    let gaps = collect_gaps(&view);
    let mut lines = Vec::new();
    if gaps.is_empty() {
        lines.push("No gaps or violations found.".to_owned());
    } else {
        for gap in &gaps {
            let status = if gap["status"] == "violated" {
                "VIOLATED"
            } else {
                "GAP"
            };
            lines.push(format!(
                "[{status}] \"{}\" × \"{}\"",
                string(gap, "rule"),
                string(gap, "dimension")
            ));
        }
        lines.push(String::new());
        lines.push(format!("{} issue(s) found.", gaps.len()));
    }
    let quiet = ids(&gaps, "id");
    output::render(
        mode,
        output::CommandOutput {
            data: gaps,
            human: lines.join("\n"),
            quiet,
        },
    )
}

async fn history(client: &Client, matrix_id: &str, rule_id: &str, mode: OutputMode) -> Result<()> {
    let versions = client.matrix_rule_history(matrix_id, rule_id).await?;
    let mut lines = Vec::new();
    if versions.is_empty() {
        lines.push("No history found.".to_owned());
    }
    for version in &versions {
        let snapshot = &version["snapshot"];
        lines.push(format!(
            "  {} ({})",
            string(snapshot, "title"),
            string(version, "id")
        ));
        let author = version["changedBy"]
            .as_str()
            .map(|value| format!(" by {value}"))
            .unwrap_or_default();
        lines.push(format!("    {}{author}", string(version, "createdAt")));
        for field in [
            "category",
            "rationale",
            "alternatives",
            "consequences",
            "counterexample",
        ] {
            if let Some(value) = snapshot[field].as_str() {
                lines.push(format!("    {field}: {value}"));
            }
        }
        lines.push(String::new());
    }
    let quiet = ids(&versions, "id");
    output::render(
        mode,
        output::CommandOutput {
            data: versions,
            human: lines.join("\n"),
            quiet,
        },
    )
}

async fn behaviors_for(client: &Client, path: &str, mode: OutputMode) -> Result<()> {
    let behaviors = client.matrix_behaviors_for_file(path).await?;
    let mut lines = Vec::new();
    if behaviors.is_empty() {
        lines.push(format!("No behavioral rules govern \"{path}\"."));
    }
    for behavior in &behaviors {
        lines.push(format!(
            "  {} [{}] {} × {}",
            string(behavior, "ruleTitle"),
            string(behavior, "layer"),
            string(behavior, "matrixName"),
            string(behavior, "dimensionName")
        ));
        if let Some(description) = behavior["description"].as_str() {
            lines.push(format!("    {description}"));
        }
        if let Some(counterexample) = behavior["counterexample"].as_str() {
            lines.push(format!("    counterexample: {counterexample}"));
        }
    }
    let quiet = ids(&behaviors, "ruleId");
    output::render(
        mode,
        output::CommandOutput {
            data: behaviors,
            human: lines.join("\n"),
            quiet,
        },
    )
}

fn mutation(mode: OutputMode, value: &Value, message: &str) -> Result<()> {
    let quiet = value["id"]
        .as_str()
        .or_else(|| value["cell"]["id"].as_str())
        .map(str::to_owned)
        .into_iter()
        .collect();
    output::render(
        mode,
        output::CommandOutput {
            data: value,
            human: message.into(),
            quiet,
        },
    )
}

fn render_grid(view: &MatrixView) -> String {
    let summary = &view.summary;
    let mut lines = vec![
        String::new(),
        format!("{} [{}]", view.matrix.name, view.matrix.layer),
        format!(
            "Coverage: {} specified, {} verified, {} unspecified, {} violated / {} total",
            summary.specified,
            summary.verified,
            summary.unspecified,
            summary.violated,
            summary.total
        ),
        String::new(),
    ];
    if view.dimensions.is_empty() || view.rules.is_empty() {
        lines.push("Matrix is empty. Add dimensions and rules first.".into());
        return lines.join("\n");
    }

    let rule_width = view
        .rules
        .iter()
        .map(|rule| rule.title.chars().count())
        .max()
        .unwrap_or(10)
        .max(10);
    let column_width = view
        .dimensions
        .iter()
        .map(|dimension| dimension.name.chars().count())
        .max()
        .unwrap_or(5)
        .max(5);
    let header = format!(
        "{}{}",
        " ".repeat(rule_width + 2),
        view.dimensions
            .iter()
            .map(|dimension| format!("{:>column_width$}", dimension.name))
            .collect::<Vec<_>>()
            .join(" ")
    );
    lines.push(header.clone());
    lines.push("-".repeat(header.chars().count()));
    for rule in &view.rules {
        let cells = view
            .dimensions
            .iter()
            .map(|dimension| {
                let key = format!("{}:{}", rule.id, dimension.id);
                let symbol = match view.cells.get(&key).map(|cell| cell.status) {
                    Some(CellStatus::Specified | CellStatus::Verified) => "✓",
                    Some(CellStatus::Violated) => "✗",
                    Some(CellStatus::Unspecified) => "?",
                    None => ".",
                };
                format!("{symbol:>column_width$}")
            })
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(format!("{:<rule_width$}  {cells}", rule.title));
    }
    lines.join("\n")
}

fn collect_gaps(view: &MatrixView) -> Vec<Value> {
    let dimensions = view
        .dimensions
        .iter()
        .map(|dimension| (dimension.id.as_str(), dimension.name.as_str()))
        .collect::<std::collections::HashMap<_, _>>();
    let rules = view
        .rules
        .iter()
        .map(|rule| (rule.id.as_str(), rule.title.as_str()))
        .collect::<std::collections::HashMap<_, _>>();
    view.cells
        .iter()
        .filter(|(_, cell)| matches!(cell.status, CellStatus::Unspecified | CellStatus::Violated))
        .map(|(key, cell)| {
            let (rule_id, dimension_id) = key.split_once(':').unwrap_or(("", ""));
            json!({
                "id": cell.id,
                "key": key,
                "rule": rules.get(rule_id).copied().unwrap_or(rule_id),
                "dimension": dimensions.get(dimension_id).copied().unwrap_or(dimension_id),
                "status": cell.status,
                "requirementCount": cell.requirement_count,
            })
        })
        .collect()
}

fn ids(values: &[Value], field: &str) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| value[field].as_str().map(str::to_owned))
        .collect()
}

fn string<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field].as_str().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::client::{Matrix, MatrixDimension, MatrixRule, MatrixView};
    use fubbik_contracts::matrix::{CellStatus, ViewCell, ViewSummary};

    use super::{collect_gaps, render_grid};

    fn view() -> MatrixView {
        MatrixView {
            matrix: Matrix {
                id: "matrix-1".into(),
                name: "API behavior".into(),
                layer: "contract".into(),
                description: None,
            },
            dimensions: vec![MatrixDimension {
                id: "dimension-1".into(),
                name: "HTTP".into(),
                order: 0,
            }],
            rules: vec![MatrixRule {
                id: "rule-1".into(),
                title: "Reject invalid input".into(),
                category: None,
                order: 0,
            }],
            cells: HashMap::from([(
                "rule-1:dimension-1".into(),
                ViewCell {
                    id: "cell-1".into(),
                    status: CellStatus::Violated,
                    requirement_count: 1,
                    code_count: 0,
                    passing_test_count: 0,
                    failing_test_count: 1,
                },
            )]),
            summary: ViewSummary {
                specified: 0,
                unspecified: 0,
                violated: 1,
                verified: 0,
                total: 1,
            },
        }
    }

    #[test]
    fn grid_and_gap_rendering_name_the_rule_and_dimension() {
        // Given a matrix with one violated cell
        let view = view();

        // When its grid and gaps are rendered
        let grid = render_grid(&view);
        let gaps = collect_gaps(&view);

        // Then both representations retain the behavioral coordinates and status
        assert!(grid.contains("Reject invalid input"));
        assert!(grid.contains('✗'));
        assert_eq!(gaps[0]["rule"], "Reject invalid input");
        assert_eq!(gaps[0]["dimension"], "HTTP");
        assert_eq!(gaps[0]["status"], "violated");
    }
}
