//! The daily digest: what happened across the projects in the last day,
//! per repo, from the change feed (`mcp_events.rs`) and open watch alerts
//! (`tracker.rs`): commits, agents, processes that went down, pull requests,
//! worktrees, and machine-wide news (dependencies that drifted, tools that
//! changed). Markdown, for Today's panel, the `get_digest` MCP tool, and a
//! fubbik note per repo once a day.

use std::collections::BTreeMap;

use crate::mcp_events::Event;
use crate::tracker::Alert;

/// What happened in one repo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoDigest {
    pub commits: Vec<String>,
    pub agents_started: usize,
    pub agents_ended: usize,
    pub agents_waiting: Vec<String>,
    pub processes_down: Vec<String>,
    pub prs: Vec<String>,
    pub worktrees: Vec<String>,
    pub alerts: Vec<String>,
}

impl RepoDigest {
    pub fn is_empty(&self) -> bool {
        *self == RepoDigest::default()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Digest {
    pub hours: u64,
    pub repos: BTreeMap<String, RepoDigest>,
    /// Not about one repo: dependency drift, tool changes, global alerts.
    pub machine: Vec<String>,
}

/// Commit subjects listed per repo (the rest are counted).
const COMMITS_LISTED: usize = 6;

/// Builds the digest of events since `since` (Unix seconds) and the open
/// alerts.
pub fn build(events: &[Event], alerts: &[Alert], since: u64, hours: u64) -> Digest {
    let mut digest = Digest {
        hours,
        ..Default::default()
    };
    for event in events.iter().filter(|e| e.at >= since) {
        let Some(repo) = &event.repo else {
            match event.kind.as_str() {
                "dependency_misaligned" => digest
                    .machine
                    .push(format!("Dependency now differs by a major: {}", event.text)),
                "dependency_aligned" => digest
                    .machine
                    .push(format!("Dependency aligned: {}", event.text)),
                "tool_changed" => digest.machine.push(format!("Tool changed: {}", event.text)),
                _ => {}
            }
            continue;
        };
        let r = digest.repos.entry(repo.clone()).or_default();
        match event.kind.as_str() {
            "commit" => r.commits.push(event.text.clone()),
            "branch_switched" => r.commits.push(format!("switched branch: {}", event.text)),
            "agent_started" => r.agents_started += 1,
            "agent_ended" => r.agents_ended += 1,
            "agent_waiting" => r.agents_waiting.push(event.text.clone()),
            "process_down" => r.processes_down.push(event.text.clone()),
            "pr_opened" => r.prs.push(format!("opened {}", event.text)),
            "pr_merged" => r.prs.push(format!("merged {}", event.text)),
            "pr_needs_you" => r.prs.push(format!("needs you: {}", event.text)),
            "worktree_added" => r.worktrees.push(format!("added {}", event.text)),
            "worktree_removed" => r.worktrees.push(format!("removed {}", event.text)),
            _ => {}
        }
    }
    for alert in alerts.iter().filter(|a| !a.dismissed) {
        match &alert.repo {
            Some(repo) => digest
                .repos
                .entry(repo.clone())
                .or_default()
                .alerts
                .push(alert.text.clone()),
            None => digest.machine.push(format!("Alert: {}", alert.text)),
        }
    }
    digest.repos.retain(|_, r| !r.is_empty());
    digest
}

/// `main: fix it (3 seconds ago)` → `main: fix it`: the age was right when
/// the feed recorded it, not when the digest is read.
fn without_age(text: &str) -> &str {
    match text.rfind(" (") {
        Some(i) if text.ends_with(" ago)") => &text[..i],
        _ => text,
    }
}

/// Each distinct line once, with `×N` when it repeated.
fn counted(lines: &[String]) -> Vec<String> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for line in lines {
        match out.iter_mut().find(|(l, _)| l == line) {
            Some((_, n)) => *n += 1,
            None => out.push((line.clone(), 1)),
        }
    }
    out.into_iter()
        .map(|(l, n)| if n > 1 { format!("{l} (×{n})") } else { l })
        .collect()
}

fn repo_lines(r: &RepoDigest) -> Vec<String> {
    let mut lines = Vec::new();
    for alert in &r.alerts {
        lines.push(format!("- ⚠ {alert}"));
    }
    if !r.commits.is_empty() {
        lines.push(format!("- {} commit(s):", r.commits.len()));
        for c in r.commits.iter().rev().take(COMMITS_LISTED) {
            lines.push(format!("  - {}", without_age(c)));
        }
        if r.commits.len() > COMMITS_LISTED {
            lines.push(format!(
                "  - … and {} more",
                r.commits.len() - COMMITS_LISTED
            ));
        }
    }
    for pr in &r.prs {
        lines.push(format!("- PR {pr}"));
    }
    if r.agents_started + r.agents_ended > 0 {
        lines.push(format!(
            "- Agents: {} started, {} ended",
            r.agents_started, r.agents_ended
        ));
    }
    for w in counted(&r.agents_waiting) {
        lines.push(format!("- {w}"));
    }
    for p in counted(&r.processes_down) {
        lines.push(format!("- Process: {p}"));
    }
    for w in &r.worktrees {
        lines.push(format!("- Worktree {w}"));
    }
    lines
}

/// The whole digest as Markdown.
pub fn markdown(digest: &Digest, title: &str) -> String {
    let mut out = format!(
        "# {title}\n\nThe last {} hours across the projects.\n",
        digest.hours
    );
    if digest.repos.is_empty() && digest.machine.is_empty() {
        out.push_str("\nA quiet day: nothing in the change feed.\n");
        return out;
    }
    for (repo, r) in &digest.repos {
        out.push_str(&format!("\n## {repo}\n\n{}\n", repo_lines(r).join("\n")));
    }
    if !digest.machine.is_empty() {
        out.push_str("\n## This machine\n\n");
        for line in &digest.machine {
            out.push_str(&format!("- {line}\n"));
        }
    }
    out
}

/// One repo's part (plus the machine-wide news), for a fubbik note in that
/// repo's space. None when nothing happened there.
pub fn repo_markdown(digest: &Digest, repo: &str, title: &str) -> Option<String> {
    let r = digest.repos.get(repo)?;
    let mut out = format!("{}\n", repo_lines(r).join("\n"));
    if !digest.machine.is_empty() {
        out.push_str("\nAcross the machine:\n");
        for line in &digest.machine {
            out.push_str(&format!("- {line}\n"));
        }
    }
    Some(format!("# {title}\n\n{out}"))
}

/// The digest of the last `hours`, from the app's change feed and alerts.
pub fn current(hours: u64) -> Digest {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (events, _, _) = crate::mcp_events::since(None, None, 5000);
    build(
        &events,
        &crate::tracker::snapshot().alerts,
        now.saturating_sub(hours * 3600),
        hours,
    )
}

/// Today's local date and hour.
pub fn today() -> (String, u32) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    local_date_hour(now, utc_offset())
}

/// The local date (`2026-11-02`) and hour for a Unix time, given the UTC
/// offset in seconds (from `date +%z`).
pub fn local_date_hour(unix: i64, offset: i64) -> (String, u32) {
    let secs = unix + offset;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (
        format!("{year:04}-{month:02}-{day:02}"),
        (rem / 3600) as u32,
    )
}

/// This machine's UTC offset in seconds (`date +%z`, read once).
pub fn utc_offset() -> i64 {
    static OFFSET: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
    *OFFSET.get_or_init(|| {
        let out = std::process::Command::new("date").arg("+%z").output().ok();
        let text = out
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        parse_offset(&text)
    })
}

/// `+0100` → 3600, `-0530` → −19800; anything else → 0.
pub fn parse_offset(text: &str) -> i64 {
    let (sign, digits) = match text.split_at_checked(1) {
        Some(("+", d)) => (1, d),
        Some(("-", d)) => (-1, d),
        _ => return 0,
    };
    if digits.len() != 4 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return 0;
    }
    let (h, m): (i64, i64) = (
        digits[..2].parse().unwrap_or(0),
        digits[2..].parse().unwrap_or(0),
    );
    sign * (h * 3600 + m * 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(at: u64, repo: Option<&str>, kind: &str, text: &str) -> Event {
        Event {
            seq: at,
            at,
            repo: repo.map(str::to_string),
            kind: kind.into(),
            text: text.into(),
        }
    }

    #[test]
    fn groups_the_day_per_repo_and_drops_older_events() {
        // Given
        let events = vec![
            event(10, Some("app"), "commit", "main: too old"),
            event(100, Some("app"), "commit", "main: one (3 seconds ago)"),
            event(
                115,
                Some("app"),
                "agent_waiting",
                "Claude Code waits for you: t",
            ),
            event(
                116,
                Some("app"),
                "agent_waiting",
                "Claude Code waits for you: t",
            ),
            event(110, Some("app"), "commit", "main: two"),
            event(120, Some("app"), "agent_started", "Claude Code: x"),
            event(130, Some("app"), "pr_merged", "feat: PR #3"),
            event(140, Some("web"), "process_down", "web went down (Error)"),
            event(150, None, "tool_changed", "tmux 3.7 → 3.8"),
            event(160, Some("app"), "bell", "ignored kinds stay out"),
        ];
        let alerts = vec![Alert {
            id: "a1".into(),
            watch: "w1".into(),
            repo: Some("web".into()),
            text: "api down 5+ min".into(),
            at: 0,
            dismissed: false,
            key: String::new(),
        }];

        // When
        let digest = build(&events, &alerts, 50, 24);
        let text = markdown(&digest, "Digest 2026-11-02");
        let web = repo_markdown(&digest, "web", "web — 2026-11-02").unwrap();

        // Then
        let app = &digest.repos["app"];
        assert_eq!(app.commits, ["main: one (3 seconds ago)", "main: two"]);
        assert!(
            text.contains("  - main: one\n"),
            "the frozen age goes:\n{text}"
        );
        assert!(text.contains("Claude Code waits for you: t (×2)"), "{text}");
        assert_eq!((app.agents_started, app.prs.len()), (1, 1));
        assert!(
            text.contains("## app") && text.contains("## web") && text.contains("## This machine")
        );
        assert!(
            text.contains("  - main: two\n  - main: one"),
            "newest first:\n{text}"
        );
        assert!(text.contains("Tool changed: tmux 3.7 → 3.8"));
        assert!(web.contains("⚠ api down 5+ min") && web.contains("Across the machine"));
        assert!(repo_markdown(&digest, "quiet", "x").is_none());
    }

    #[test]
    fn a_quiet_day_says_so() {
        // Given / When
        let text = markdown(&build(&[], &[], 0, 24), "Digest");

        // Then
        assert!(text.contains("A quiet day"));
    }

    #[test]
    fn local_dates_and_offsets() {
        // Given: 2026-11-01 23:30 UTC
        let unix = 1_793_575_800;

        // When / Then
        assert_eq!(local_date_hour(unix, 0), ("2026-11-01".to_string(), 23));
        assert_eq!(local_date_hour(unix, 3600), ("2026-11-02".to_string(), 0));
        assert_eq!(parse_offset("+0100"), 3600);
        assert_eq!(parse_offset("-0530"), -19_800);
        assert_eq!(parse_offset("garbage"), 0);
    }
}
