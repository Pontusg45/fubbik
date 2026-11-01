//! Pull request and CI status from GitHub, through the `gh` CLI (its own
//! login; nothing here holds a token). One `gh pr list` per repo gives every
//! branch's latest PR.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;

/// The fields asked of `gh pr list --json`.
const FIELDS: &str =
    "number,state,isDraft,url,headRefName,reviewDecision,statusCheckRollup,mergeable";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Checks {
    pub passed: usize,
    pub failed: usize,
    pub pending: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrStatus {
    pub number: u64,
    /// `OPEN`, `MERGED` or `CLOSED`.
    pub state: String,
    pub draft: bool,
    pub url: String,
    /// `APPROVED`, `CHANGES_REQUESTED`, `REVIEW_REQUIRED`; None when the repo
    /// requires no review.
    pub review: Option<String>,
    pub checks: Checks,
    /// `CONFLICTING` when it cannot merge cleanly.
    pub conflicting: bool,
}

impl PrStatus {
    pub fn open(&self) -> bool {
        self.state == "OPEN"
    }

    /// Open, checks green, approved (or no review needed), no conflicts.
    pub fn ready(&self) -> bool {
        self.open()
            && !self.draft
            && !self.conflicting
            && self.checks.failed == 0
            && self.checks.pending == 0
            && self.review.as_deref().is_none_or(|r| r == "APPROVED")
    }

    /// Something for you to act on: failing checks, changes requested, or
    /// conflicts (open PRs only).
    pub fn needs_you(&self) -> Option<&'static str> {
        if !self.open() {
            None
        } else if self.checks.failed > 0 {
            Some("checks failing")
        } else if self.review.as_deref() == Some("CHANGES_REQUESTED") {
            Some("changes requested")
        } else if self.conflicting {
            Some("merge conflicts")
        } else {
            None
        }
    }

    /// A short pill text: `#12 draft · 2 ✗ · changes requested`.
    pub fn summary(&self) -> String {
        let mut parts = vec![format!("PR #{}", self.number)];
        match self.state.as_str() {
            "MERGED" => parts.push("merged".into()),
            "CLOSED" => parts.push("closed".into()),
            _ => {
                if self.draft {
                    parts.push("draft".into());
                }
                let c = &self.checks;
                if c.failed > 0 {
                    parts.push(format!("{} ✗", c.failed));
                } else if c.pending > 0 {
                    parts.push(format!("{} running", c.pending));
                } else if c.passed > 0 {
                    parts.push("checks ✓".into());
                }
                match self.review.as_deref() {
                    Some("APPROVED") => parts.push("approved".into()),
                    Some("CHANGES_REQUESTED") => parts.push("changes requested".into()),
                    Some("REVIEW_REQUIRED") => parts.push("review needed".into()),
                    _ => {}
                }
                if self.conflicting {
                    parts.push("conflicts".into());
                }
            }
        }
        parts.join(" · ")
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPr {
    number: u64,
    state: String,
    #[serde(default)]
    is_draft: bool,
    url: String,
    head_ref_name: String,
    #[serde(default)]
    review_decision: Option<String>,
    #[serde(default)]
    status_check_rollup: Option<Vec<RawCheck>>,
    #[serde(default)]
    mergeable: Option<String>,
}

/// A CheckRun (`status`, `conclusion`) or a StatusContext (`state`).
#[derive(Deserialize)]
struct RawCheck {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    conclusion: Option<String>,
    #[serde(default)]
    state: Option<String>,
}

fn count(checks: &[RawCheck]) -> Checks {
    let mut out = Checks::default();
    for check in checks {
        let outcome = check
            .conclusion
            .as_deref()
            .filter(|c| !c.is_empty())
            .or(check.state.as_deref())
            .unwrap_or_default();
        match outcome {
            "SUCCESS" | "NEUTRAL" | "SKIPPED" => out.passed += 1,
            "FAILURE" | "ERROR" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED"
            | "STARTUP_FAILURE" => out.failed += 1,
            _ if check.status.as_deref() == Some("COMPLETED") => out.passed += 1,
            _ => out.pending += 1,
        }
    }
    out
}

/// Parses `gh pr list --json …` into each branch's newest PR (gh lists
/// newest first).
pub fn parse(json: &str) -> Result<HashMap<String, PrStatus>, String> {
    let raw: Vec<RawPr> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut out = HashMap::new();
    for pr in raw {
        out.entry(pr.head_ref_name.clone())
            .or_insert_with(|| PrStatus {
                number: pr.number,
                state: pr.state,
                draft: pr.is_draft,
                url: pr.url,
                review: pr.review_decision.filter(|r| !r.is_empty()),
                checks: count(pr.status_check_rollup.as_deref().unwrap_or_default()),
                conflicting: pr.mergeable.as_deref() == Some("CONFLICTING"),
            });
    }
    Ok(out)
}

/// Every branch's latest PR in the repo at `dir` (open, merged or closed).
pub fn pull_requests(dir: &Path) -> Result<HashMap<String, PrStatus>, String> {
    let output = Command::new("gh")
        .args([
            "pr", "list", "--state", "all", "--limit", "60", "--json", FIELDS,
        ])
        .current_dir(dir)
        .output()
        .map_err(|e| format!("gh: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    parse(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_checks_and_keeps_the_newest_pr_per_branch() {
        // Given: two PRs for one branch (newest first) and one for another
        let json = r#"[
          {"number": 9, "state": "OPEN", "isDraft": false, "url": "u9", "headRefName": "feat",
           "reviewDecision": "CHANGES_REQUESTED", "mergeable": "MERGEABLE",
           "statusCheckRollup": [
             {"__typename": "CheckRun", "status": "COMPLETED", "conclusion": "SUCCESS"},
             {"__typename": "CheckRun", "status": "COMPLETED", "conclusion": "FAILURE"},
             {"__typename": "CheckRun", "status": "IN_PROGRESS", "conclusion": ""},
             {"__typename": "StatusContext", "state": "PENDING"}
           ]},
          {"number": 3, "state": "CLOSED", "isDraft": false, "url": "u3", "headRefName": "feat",
           "reviewDecision": "", "statusCheckRollup": [], "mergeable": "UNKNOWN"},
          {"number": 5, "state": "OPEN", "isDraft": false, "url": "u5", "headRefName": "fix",
           "reviewDecision": "", "statusCheckRollup": [
             {"__typename": "StatusContext", "state": "SUCCESS"}], "mergeable": "MERGEABLE"}
        ]"#;

        // When
        let prs = parse(json).unwrap();

        // Then
        let feat = &prs["feat"];
        assert_eq!(feat.number, 9);
        assert_eq!(
            feat.checks,
            Checks {
                passed: 1,
                failed: 1,
                pending: 2
            }
        );
        assert_eq!(feat.needs_you(), Some("checks failing"));
        assert_eq!(feat.summary(), "PR #9 · 1 ✗ · changes requested");
        let fix = &prs["fix"];
        assert!(fix.ready(), "no review required, checks green");
        assert_eq!(fix.review, None);
        assert_eq!(fix.summary(), "PR #5 · checks ✓");
    }

    #[test]
    fn closed_and_merged_prs_never_need_you() {
        // Given
        let json = r#"[{"number": 1, "state": "MERGED", "isDraft": false, "url": "u",
          "headRefName": "old", "reviewDecision": "CHANGES_REQUESTED",
          "statusCheckRollup": [{"status": "COMPLETED", "conclusion": "FAILURE"}],
          "mergeable": "CONFLICTING"}]"#;

        // When
        let pr = parse(json).unwrap().remove("old").unwrap();

        // Then
        assert_eq!(pr.needs_you(), None);
        assert!(!pr.ready());
        assert_eq!(pr.summary(), "PR #1 · merged");
    }
}
