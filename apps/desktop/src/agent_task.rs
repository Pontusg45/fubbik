//! Starting a Claude Code session on a plan task: an optional new worktree,
//! a board run keyed to the session id (decided here, passed to
//! `claude --session-id`), a claim on the task, and a first prompt built from
//! the task and its plan's fubbik context.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::fubbik::{FubbikClient, PlanDetail, Task};
use crate::worktree;

/// A random (v4) UUID, from `/dev/urandom`.
pub fn uuid_v4() -> String {
    let mut bytes = [0u8; 16];
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .is_err()
    {
        // Fallback: time and pid; unique enough for a client mutation id.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        bytes = (nanos ^ ((std::process::id() as u128) << 64)).to_le_bytes();
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// `task/<slug of the title>`, at most 48 characters.
pub fn branch_for(task: &Task) -> String {
    let mut slug = String::new();
    for c in task.title.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug: String = slug.trim_end_matches('-').chars().take(40).collect();
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        format!("task/{}", &task.id.chars().take(8).collect::<String>())
    } else {
        format!("task/{slug}")
    }
}

pub struct Assignment<'a> {
    pub plan: &'a PlanDetail,
    pub task: &'a Task,
    /// None when the coordination board is off.
    pub run_id: Option<&'a str>,
    pub context: &'a str,
}

pub fn build_prompt(a: &Assignment) -> String {
    let mut prompt = format!(
        "You are working on task \"{}\" of the fubbik plan \"{}\".\n\nPlan id: {}\nTask id: {}\n",
        a.task.title, a.plan.title, a.plan.id, a.task.id
    );
    if let Some(run) = a.run_id {
        prompt.push_str(&format!(
            "Board run id: {run} (this session is already joined to the plan's coordination board and the task is claimed for it).\n\
             Use the fubbik MCP tools: `read_board` to see the board, `write_board_entry` for progress notes, questions and handoffs, \
             and `update_board_task` to mark the task done when every acceptance criterion holds.\n"
        ));
    }
    if let Some(description) = &a.task.description {
        prompt.push_str(&format!("\n## Task\n\n{description}\n"));
    }
    if !a.task.acceptance.is_empty() {
        prompt.push_str("\n## Acceptance criteria\n\n");
        for (text, done) in &a.task.acceptance {
            prompt.push_str(&format!("- [{}] {text}\n", if *done { "x" } else { " " }));
        }
    }
    if !a.context.trim().is_empty() {
        prompt.push_str(&format!(
            "\n## Knowledge from fubbik for this plan\n\n{}\n",
            a.context.trim()
        ));
    }
    prompt.push_str("\nStart by reading the relevant code, then propose a short plan before changing anything.\n");
    prompt
}

/// Where a task's session should run, and the command that starts it.
pub struct Started {
    pub cwd: PathBuf,
    pub command: String,
}

/// Everything after the click but the launch, blocking: the worktree, the
/// board run and claim, and the prompt file. The caller runs `command`.
pub fn start(
    client: &FubbikClient,
    repo: &Path,
    plan: &PlanDetail,
    task: &Task,
    new_worktree: bool,
) -> Result<Started, String> {
    let cwd = if new_worktree {
        worktree::create(repo, &branch_for(task))?
    } else {
        repo.to_path_buf()
    };
    let session_id = uuid_v4();
    let handle = format!("claude-{}", &session_id[..8]);
    let metadata = json!({
        "source": "fubbik-desktop",
        "sessionId": session_id,
        "worktree": cwd.to_string_lossy(),
    });
    // The board is optional: with `coordination` off the session still starts.
    let run_id = client
        .join_run(&plan.id, &handle, &session_id, metadata)
        .ok();
    if let Some(run) = &run_id {
        client.claim_task(&plan.id, &task.id, run)?;
        let where_ = cwd
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        client.transition_task(
            &plan.id,
            &task.id,
            run,
            "in_progress",
            &format!("Started from fubbik desktop in {where_}"),
        )?;
    }
    let context = client.context_for_plan(&plan.id).unwrap_or_default();
    let prompt = build_prompt(&Assignment {
        plan,
        task,
        run_id: run_id.as_deref(),
        context: &context,
    });

    let prompt_file = std::env::temp_dir().join(format!("fubbik-task-{session_id}.md"));
    std::fs::write(&prompt_file, prompt).map_err(|e| e.to_string())?;
    let command = format!(
        "claude --session-id {session_id} \"$(cat '{}')\"",
        prompt_file.display()
    );
    Ok(Started { cwd, command })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(title: &str) -> Task {
        Task {
            id: "t-123456789".into(),
            title: title.into(),
            description: Some("Make it prune.".into()),
            acceptance: vec![("Stale entries vanish".into(), false)],
            status: "pending".into(),
        }
    }

    #[test]
    fn uuid_is_v4_shaped() {
        // Given
        let ids = [uuid_v4(), uuid_v4()];

        // When
        let parts: Vec<usize> = ids[0].split('-').map(str::len).collect();

        // Then
        assert_eq!(parts, [8, 4, 4, 4, 12]);
        assert_eq!(&ids[0][14..15], "4");
        assert_ne!(ids[0], ids[1]);
    }

    #[test]
    fn branch_slugs_the_title() {
        // Given
        let tasks = [task("Add worktree prune — & tests!"), task("???")];

        // When
        let branches: Vec<String> = tasks.iter().map(branch_for).collect();

        // Then
        assert_eq!(branches, ["task/add-worktree-prune-tests", "task/t-123456"]);
    }

    #[test]
    fn prompt_carries_ids_criteria_and_context() {
        // Given
        let plan = PlanDetail {
            id: "p1".into(),
            title: "Desktop".into(),
            status: "in_progress".into(),
            tasks: vec![],
        };
        let task = task("Prune");

        // When
        let prompt = build_prompt(&Assignment {
            plan: &plan,
            task: &task,
            run_id: Some("r9"),
            context: "## Conventions\nUse AppError",
        });

        // Then
        assert!(prompt.contains("Plan id: p1"));
        assert!(prompt.contains("Task id: t-123456789"));
        assert!(prompt.contains("Board run id: r9"));
        assert!(prompt.contains("- [ ] Stale entries vanish"));
        assert!(prompt.contains("Use AppError"));
    }

    #[test]
    fn prompt_without_board_skips_board_tools() {
        // Given
        let plan = PlanDetail::default();
        let task = task("Prune");

        // When
        let prompt = build_prompt(&Assignment {
            plan: &plan,
            task: &task,
            run_id: None,
            context: "",
        });

        // Then
        assert!(!prompt.contains("update_board_task"));
    }
}
