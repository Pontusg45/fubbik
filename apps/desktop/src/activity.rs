//! What happened in a repo and how healthy it is: commit history, a
//! per-day heatmap, stale branches, a standup summary, and a health score
//! with the reasons behind it. The builders are pure; git and the audit
//! tools are called by the thin functions at the top.

use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const DAY: i64 = 86_400;

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub hash: String,
    pub time: i64,
    pub subject: String,
}

pub fn parse_log(text: &str) -> Vec<Commit> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            Some(Commit {
                hash: parts.next()?.to_string(),
                time: parts.next()?.parse().ok()?,
                subject: parts.next().unwrap_or_default().to_string(),
            })
        })
        .collect()
}

/// Commits on any branch in the last `days` days, newest first.
pub fn commits_since(repo: &Path, days: i64) -> Vec<Commit> {
    let since = format!("--since={days}.days");
    git(
        repo,
        &[
            "log",
            "--all",
            "--no-merges",
            &since,
            "--format=%h%x09%at%x09%s",
        ],
    )
    .map(|t| parse_log(&t))
    .unwrap_or_default()
}

/// Counts per day for the last `days` days, oldest first.
pub fn heatmap(times: &[i64], days: usize, now: i64) -> Vec<u32> {
    let mut counts = vec![0u32; days];
    let today = now.div_euclid(DAY);
    for t in times {
        let age = today - t.div_euclid(DAY);
        if (0..days as i64).contains(&age) {
            counts[days - 1 - age as usize] += 1;
        }
    }
    counts
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    pub age_days: i64,
    pub merged: bool,
}

pub fn parse_branches(refs: &str, merged: &str, now: i64) -> Vec<Branch> {
    let merged: Vec<&str> = merged
        .lines()
        .map(|l| l.trim_start_matches(['*', '+', ' ']))
        .collect();
    refs.lines()
        .filter_map(|line| {
            let (name, time) = line.split_once('\t')?;
            Some(Branch {
                name: name.to_string(),
                age_days: (now - time.trim().parse::<i64>().ok()?) / DAY,
                merged: merged.contains(&name),
            })
        })
        .collect()
}

/// Branches (other than main) with no commit for `days` days.
pub fn stale_branches(repo: &Path, main: &str, days: i64) -> Vec<Branch> {
    let refs = git(
        repo,
        &[
            "for-each-ref",
            "refs/heads",
            "--format=%(refname:short)%09%(committerdate:unix)",
        ],
    )
    .unwrap_or_default();
    let merged = git(repo, &["branch", "--merged", main]).unwrap_or_default();
    parse_branches(&refs, &merged, now())
        .into_iter()
        .filter(|b| b.name != main && b.age_days >= days)
        .collect()
}

/// Vulnerability findings from the audit tools that are installed:
/// `cargo audit` for Cargo.lock, `pnpm audit` for pnpm-lock.yaml. None when
/// neither applies or is installed.
pub fn audit(repo: &Path) -> Option<Result<usize, String>> {
    let run = |program: &str, args: &[&str]| -> Option<std::process::Output> {
        Command::new(program)
            .args(args)
            .current_dir(repo)
            .output()
            .ok()
    };
    let mut findings = None;
    if repo.join("Cargo.lock").is_file()
        && let Some(out) = run("cargo", &["audit", "--json", "-q"])
    {
        let text = String::from_utf8_lossy(&out.stdout);
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
            let n = json["vulnerabilities"]["count"].as_u64().unwrap_or(0) as usize;
            findings = Some(Ok(findings.and_then(Result::ok).unwrap_or(0) + n));
        }
    }
    if repo.join("pnpm-lock.yaml").is_file()
        && let Some(out) = run("pnpm", &["audit", "--json"])
    {
        let text = String::from_utf8_lossy(&out.stdout);
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
            let counts = &json["metadata"]["vulnerabilities"];
            let n: u64 = ["low", "moderate", "high", "critical"]
                .iter()
                .filter_map(|k| counts[*k].as_u64())
                .sum();
            findings = Some(Ok(findings.and_then(Result::ok).unwrap_or(0) + n as usize));
        }
    }
    findings
}

/// Inputs for the health score; every field is a count of something wrong
/// (or a coverage percentage).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HealthInputs {
    pub failing_processes: usize,
    pub failing_requirements: i64,
    pub violated_cells: i64,
    pub stale_branches: usize,
    pub idle_worktrees: usize,
    pub stale_knowledge: i64,
    pub uncommitted_files: usize,
    pub vulnerabilities: Option<usize>,
    /// Knowledge coverage of source files, when a gaps scan ran.
    pub coverage: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Health {
    pub score: u32,
    /// What cost points, worst first.
    pub reasons: Vec<(u32, String)>,
}

/// 100 minus capped penalties, so one bad area cannot hide the others.
pub fn health(i: &HealthInputs) -> Health {
    let mut reasons: Vec<(u32, String)> = Vec::new();
    let mut penalty = |points: u32, cap: u32, n: usize, what: &str| {
        if n > 0 {
            reasons.push(((points * n as u32).min(cap), format!("{n} {what}")));
        }
    };
    penalty(
        10,
        30,
        i.failing_processes,
        "failing or degraded process(es)",
    );
    penalty(
        8,
        24,
        i.failing_requirements as usize,
        "failing requirement(s)",
    );
    penalty(
        6,
        18,
        i.violated_cells as usize,
        "violated behaviour cell(s)",
    );
    penalty(
        10,
        30,
        i.vulnerabilities.unwrap_or(0),
        "known vulnerabilit(ies) in dependencies",
    );
    penalty(2, 10, i.stale_branches, "stale branch(es)");
    penalty(3, 9, i.idle_worktrees, "idle worktree(s)");
    penalty(1, 10, i.stale_knowledge as usize, "stale knowledge flag(s)");
    if i.uncommitted_files > 50 {
        reasons.push((5, format!("{} uncommitted files", i.uncommitted_files)));
    }
    if let Some(coverage) = i.coverage.filter(|c| *c < 50) {
        reasons.push((
            ((50 - coverage) / 5) as u32,
            format!("{coverage}% of source files covered by knowledge"),
        ));
    }
    reasons.sort_by_key(|(points, _)| std::cmp::Reverse(*points));
    let lost: u32 = reasons.iter().map(|(p, _)| p).sum();
    Health {
        score: 100u32.saturating_sub(lost),
        reasons,
    }
}

/// What a standup or weekly summary covers.
#[derive(Debug, Clone, Default)]
pub struct Period<'a> {
    pub repo: &'a str,
    pub days: i64,
    pub commits: &'a [Commit],
    pub tasks_done: &'a [String],
    pub tasks_in_progress: &'a [String],
    pub tasks_blocked: &'a [String],
    pub sessions: &'a [String],
    pub requirements: Option<(i64, i64, i64)>,
}

pub fn summary(p: &Period) -> String {
    let span = if p.days == 1 {
        "the last day".to_string()
    } else {
        format!("the last {} days", p.days)
    };
    let mut out = format!("## {} — {span}\n", p.repo);
    let mut list = |title: &str, items: &[String], limit: usize| {
        if items.is_empty() {
            return;
        }
        out.push_str(&format!("\n**{title}** ({})\n", items.len()));
        for item in items.iter().take(limit) {
            out.push_str(&format!("- {item}\n"));
        }
        if items.len() > limit {
            out.push_str(&format!("- … and {} more\n", items.len() - limit));
        }
    };
    let commits: Vec<String> = p
        .commits
        .iter()
        .map(|c| format!("{} `{}`", c.subject, c.hash))
        .collect();
    list("Done", p.tasks_done, 20);
    list("In progress", p.tasks_in_progress, 20);
    list("Blocked", p.tasks_blocked, 20);
    list("Commits", &commits, 15);
    list("Agent sessions", p.sessions, 10);
    if let Some((passing, failing, untested)) = p.requirements {
        out.push_str(&format!(
            "\n**Requirements:** {passing} passing · {failing} failing · {untested} untested\n"
        ));
    }
    if p.commits.is_empty() && p.tasks_done.is_empty() && p.sessions.is_empty() {
        out.push_str("\nNo recorded activity.\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_log_lines() {
        // Given
        let text = "abc1234\t1793400000\tfix: tabs\tstill subject\nbad line\n";

        // When
        let commits = parse_log(text);

        // Then
        assert_eq!(
            commits,
            [Commit {
                hash: "abc1234".into(),
                time: 1793400000,
                subject: "fix: tabs\tstill subject".into()
            }]
        );
    }

    #[test]
    fn heatmap_buckets_by_day_oldest_first() {
        // Given
        let now = 100 * DAY + 3600;
        let times = [now, now - 10, now - DAY, now - 6 * DAY, now - 30 * DAY];

        // When
        let map = heatmap(&times, 7, now);

        // Then
        assert_eq!(map, [1, 0, 0, 0, 0, 1, 2]);
    }

    #[test]
    fn parses_branches_with_age_and_merged() {
        // Given
        let now = 100 * DAY;
        let refs = format!(
            "main\t{}\ntask/a\t{}\nold\t{}\n",
            now,
            now - 2 * DAY,
            now - 40 * DAY
        );
        let merged = "* main\n  task/a\n";

        // When
        let branches = parse_branches(&refs, merged, now);

        // Then
        assert_eq!(
            branches[1],
            Branch {
                name: "task/a".into(),
                age_days: 2,
                merged: true
            }
        );
        assert_eq!(
            branches[2],
            Branch {
                name: "old".into(),
                age_days: 40,
                merged: false
            }
        );
    }

    #[test]
    fn health_is_full_when_nothing_is_wrong_and_caps_each_area() {
        // Given
        let clean = HealthInputs::default();
        let bad = HealthInputs {
            failing_processes: 9,
            stale_branches: 2,
            ..Default::default()
        };

        // When
        let (a, b) = (health(&clean), health(&bad));

        // Then
        assert_eq!(a.score, 100);
        assert!(a.reasons.is_empty());
        assert_eq!(b.score, 100 - 30 - 4, "9 processes cap at 30");
        assert_eq!(b.reasons[0].1, "9 failing or degraded process(es)");
    }

    #[test]
    fn summary_lists_what_happened() {
        // Given
        let commits = [Commit {
            hash: "abc".into(),
            time: 0,
            subject: "feat: board".into(),
        }];
        let done = ["Add board".to_string()];
        let period = Period {
            repo: "fubbik",
            days: 7,
            commits: &commits,
            tasks_done: &done,
            requirements: Some((3, 1, 2)),
            ..Default::default()
        };

        // When
        let text = summary(&period);

        // Then
        assert!(text.starts_with("## fubbik — the last 7 days"));
        assert!(text.contains("**Done** (1)\n- Add board"));
        assert!(text.contains("- feat: board `abc`"));
        assert!(text.contains("3 passing · 1 failing · 2 untested"));
    }
}
