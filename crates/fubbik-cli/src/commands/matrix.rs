use anyhow::Result;
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
    match mode {
        OutputMode::Json => output::json(&matrices),
        OutputMode::Quiet => {
            for matrix in matrices {
                println!("{}", matrix.id);
            }
            Ok(())
        }
        OutputMode::Human => {
            if matrices.is_empty() {
                println!("No matrices found.");
            }
            for matrix in matrices {
                println!("  {} [{}] ({})", matrix.name, matrix.layer, matrix.id);
                if let Some(description) = matrix.description {
                    println!("    {description}");
                }
            }
            Ok(())
        }
    }
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
    match mode {
        OutputMode::Json => output::json(&matrix),
        OutputMode::Quiet => {
            println!("{}", matrix.id);
            Ok(())
        }
        OutputMode::Human => {
            println!("Created matrix \"{}\" ({})", matrix.name, matrix.id);
            Ok(())
        }
    }
}

async fn show(client: &Client, id: &str, mode: OutputMode) -> Result<()> {
    let view = client.matrix_view(id).await?;
    match mode {
        OutputMode::Json => output::json(&view),
        OutputMode::Quiet => {
            println!("{}", view.matrix.id);
            Ok(())
        }
        OutputMode::Human => {
            println!("{}", render_grid(&view));
            Ok(())
        }
    }
}

async fn gaps(client: &Client, id: &str, mode: OutputMode) -> Result<()> {
    let view = client.matrix_view(id).await?;
    let gaps = collect_gaps(&view);
    match mode {
        OutputMode::Json => output::json(&gaps),
        OutputMode::Quiet => {
            for gap in &gaps {
                println!("{}", gap["id"].as_str().unwrap_or_default());
            }
            Ok(())
        }
        OutputMode::Human => {
            if gaps.is_empty() {
                println!("No gaps or violations found.");
            } else {
                for gap in &gaps {
                    let status = if gap["status"] == "violated" {
                        "VIOLATED"
                    } else {
                        "GAP"
                    };
                    println!(
                        "[{status}] \"{}\" × \"{}\"",
                        string(gap, "rule"),
                        string(gap, "dimension")
                    );
                }
                println!("\n{} issue(s) found.", gaps.len());
            }
            Ok(())
        }
    }
}

async fn history(client: &Client, matrix_id: &str, rule_id: &str, mode: OutputMode) -> Result<()> {
    let versions = client.matrix_rule_history(matrix_id, rule_id).await?;
    match mode {
        OutputMode::Json => output::json(&versions),
        OutputMode::Quiet => {
            print_ids(&versions, "id");
            Ok(())
        }
        OutputMode::Human => {
            if versions.is_empty() {
                println!("No history found.");
            }
            for version in versions {
                let snapshot = &version["snapshot"];
                println!(
                    "  {} ({})",
                    string(snapshot, "title"),
                    string(&version, "id")
                );
                let author = version["changedBy"]
                    .as_str()
                    .map(|value| format!(" by {value}"))
                    .unwrap_or_default();
                println!("    {}{author}", string(&version, "createdAt"));
                for field in [
                    "category",
                    "rationale",
                    "alternatives",
                    "consequences",
                    "counterexample",
                ] {
                    if let Some(value) = snapshot[field].as_str() {
                        println!("    {field}: {value}");
                    }
                }
                println!();
            }
            Ok(())
        }
    }
}

async fn behaviors_for(client: &Client, path: &str, mode: OutputMode) -> Result<()> {
    let behaviors = client.matrix_behaviors_for_file(path).await?;
    match mode {
        OutputMode::Json => output::json(&behaviors),
        OutputMode::Quiet => {
            print_ids(&behaviors, "ruleId");
            Ok(())
        }
        OutputMode::Human => {
            if behaviors.is_empty() {
                println!("No behavioral rules govern \"{path}\".");
            }
            for behavior in behaviors {
                println!(
                    "  {} [{}] {} × {}",
                    string(&behavior, "ruleTitle"),
                    string(&behavior, "layer"),
                    string(&behavior, "matrixName"),
                    string(&behavior, "dimensionName")
                );
                if let Some(description) = behavior["description"].as_str() {
                    println!("    {description}");
                }
                if let Some(counterexample) = behavior["counterexample"].as_str() {
                    println!("    counterexample: {counterexample}");
                }
            }
            Ok(())
        }
    }
}

fn mutation(mode: OutputMode, value: &Value, message: &str) -> Result<()> {
    match mode {
        OutputMode::Json => output::json(value),
        OutputMode::Quiet => {
            if let Some(id) = value["id"]
                .as_str()
                .or_else(|| value["cell"]["id"].as_str())
            {
                println!("{id}");
            }
            Ok(())
        }
        OutputMode::Human => {
            println!("{message}");
            Ok(())
        }
    }
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
                let symbol = match view.cells.get(&key).map(|cell| cell.status.as_str()) {
                    Some("specified" | "verified") => "✓",
                    Some("violated") => "✗",
                    Some(_) => "?",
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
        .filter(|(_, cell)| matches!(cell.status.as_str(), "unspecified" | "violated"))
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

fn print_ids(values: &[Value], field: &str) {
    for value in values {
        if let Some(id) = value[field].as_str() {
            println!("{id}");
        }
    }
}

fn string<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field].as_str().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::client::{
        Matrix, MatrixCell, MatrixDimension, MatrixRule, MatrixSummary, MatrixView,
    };

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
                MatrixCell {
                    id: "cell-1".into(),
                    status: "violated".into(),
                    requirement_count: 1,
                },
            )]),
            summary: MatrixSummary {
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
